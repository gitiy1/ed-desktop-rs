use crate::settings::{ProxyMode, ProxyProtocol, ProxySettings};
use anyhow::{Context, Result, anyhow};
use reqwest::blocking::{Client, ClientBuilder};
use reqwest::{NoProxy, Proxy};
#[cfg(not(target_os = "windows"))]
use std::net::IpAddr;
use std::time::Duration;

pub fn http_client(
    timeout: Duration,
    user_agent: &str,
    settings: &ProxySettings,
) -> Result<Client> {
    let builder = Client::builder()
        .timeout(timeout)
        .connect_timeout(Duration::from_secs(20))
        .user_agent(user_agent);
    apply_proxy(builder, settings)?
        .build()
        .context("create HTTP client")
}

fn apply_proxy(mut builder: ClientBuilder, settings: &ProxySettings) -> Result<ClientBuilder> {
    match settings.mode {
        ProxyMode::System => {
            if let Some((url, bypass)) = system_proxy_url() {
                let proxy = Proxy::all(url)?.no_proxy(NoProxy::from_string(&bypass));
                builder = builder.no_proxy().proxy(proxy);
            }
        }
        ProxyMode::Manual => {
            let url = manual_proxy_url(settings.protocol, &settings.address)?;
            builder = builder.no_proxy().proxy(Proxy::all(url)?);
        }
    }
    Ok(builder)
}

fn manual_proxy_url(protocol: ProxyProtocol, address: &str) -> Result<String> {
    let address = address.trim();
    if address.is_empty() {
        return Err(anyhow!("Enter a proxy host and port."));
    }
    let scheme = match protocol {
        ProxyProtocol::Http => "http",
        ProxyProtocol::Socks5 => "socks5h",
    };
    let endpoint = if address.contains("://") {
        address.to_string()
    } else {
        format!("{scheme}://{address}")
    };
    let parsed = reqwest::Url::parse(&endpoint).context("invalid proxy address")?;
    let valid_scheme = match protocol {
        ProxyProtocol::Http => matches!(parsed.scheme(), "http" | "https"),
        ProxyProtocol::Socks5 => matches!(parsed.scheme(), "socks5" | "socks5h"),
    };
    if !valid_scheme || parsed.host_str().is_none() || parsed.port_or_known_default().is_none() {
        return Err(anyhow!(
            "Enter a valid {} proxy host and port.",
            match protocol {
                ProxyProtocol::Http => "HTTP",
                ProxyProtocol::Socks5 => "SOCKS5",
            }
        ));
    }
    if !parsed.username().is_empty() || parsed.password().is_some() {
        return Err(anyhow!("Proxy credentials in the URL are not supported."));
    }
    Ok(endpoint)
}

fn system_proxy_url() -> Option<(String, String)> {
    #[cfg(target_os = "windows")]
    let (url, bypass) = windows_system_proxy()?;

    #[cfg(not(target_os = "windows"))]
    let (url, bypass) = {
        let proxy = sysproxy::Sysproxy::get_system_proxy().ok()?;
        if !proxy.enable || proxy.host.trim().is_empty() || proxy.port == 0 {
            return None;
        }
        let explicit_socks = sysproxy::Sysproxy::get_socks().ok();
        let socks_active = explicit_socks.is_some_and(|socks| {
            !socks.host.trim().is_empty() && socks.host == proxy.host && socks.port == proxy.port
        });
        let scheme = if socks_active { "socks5h" } else { "http" };
        let host = proxy.host.trim();
        let host = if host.parse::<IpAddr>().is_ok_and(|ip| ip.is_ipv6()) {
            format!("[{host}]")
        } else {
            host.to_string()
        };
        (format!("{scheme}://{host}:{}", proxy.port), proxy.bypass)
    };
    Some((url, bypass))
}

#[cfg(target_os = "windows")]
fn windows_system_proxy() -> Option<(String, String)> {
    use winreg::RegKey;
    use winreg::enums::HKEY_CURRENT_USER;

    let settings = RegKey::predef(HKEY_CURRENT_USER)
        .open_subkey("SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Internet Settings")
        .ok()?;
    if settings.get_value::<u32, _>("ProxyEnable").ok()? != 1 {
        return None;
    }
    let server: String = settings.get_value("ProxyServer").ok()?;
    let bypass = settings
        .get_value::<String, _>("ProxyOverride")
        .unwrap_or_default();
    parse_windows_proxy(&server, &bypass)
}

#[cfg(any(target_os = "windows", test))]
fn parse_windows_proxy(server: &str, bypass: &str) -> Option<(String, String)> {
    let mut http = None;
    let mut socks = None;
    for entry in server
        .split(';')
        .map(str::trim)
        .filter(|entry| !entry.is_empty())
    {
        if let Some((protocol, endpoint)) = entry.split_once('=') {
            match protocol.trim().to_ascii_lowercase().as_str() {
                "socks" | "socks5" => socks = Some(endpoint.trim()),
                "http" | "https" => http = Some(endpoint.trim()),
                _ => {}
            }
        } else if http.is_none() {
            http = Some(entry);
        }
    }
    if let Some(endpoint) = socks {
        let url = if endpoint.contains("://") {
            endpoint.to_string()
        } else {
            format!("socks5h://{endpoint}")
        };
        return Some((url, bypass.replace(';', ",")));
    }
    let endpoint = http?;
    let url = if endpoint.contains("://") {
        endpoint.to_string()
    } else {
        format!("http://{endpoint}")
    };
    Some((url, bypass.replace(';', ",")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manual_http_and_socks5_endpoints_are_normalized() {
        assert_eq!(
            manual_proxy_url(ProxyProtocol::Http, "127.0.0.1:8080").unwrap(),
            "http://127.0.0.1:8080"
        );
        assert_eq!(
            manual_proxy_url(ProxyProtocol::Socks5, "127.0.0.1:1080").unwrap(),
            "socks5h://127.0.0.1:1080"
        );
        assert_eq!(
            manual_proxy_url(ProxyProtocol::Socks5, "socks5://[::1]:1080").unwrap(),
            "socks5://[::1]:1080"
        );
    }

    #[test]
    fn manual_proxy_rejects_empty_invalid_and_credential_bearing_endpoints() {
        assert!(manual_proxy_url(ProxyProtocol::Http, " ").is_err());
        assert!(manual_proxy_url(ProxyProtocol::Http, "socks5://127.0.0.1:1080").is_err());
        assert!(manual_proxy_url(ProxyProtocol::Socks5, "http://proxy:8080").is_err());
        assert!(manual_proxy_url(ProxyProtocol::Http, "http://user@proxy:8080").is_err());
    }

    #[test]
    fn reqwest_clients_accept_both_manual_proxy_protocols() {
        for (protocol, address) in [
            (ProxyProtocol::Http, "127.0.0.1:8080"),
            (ProxyProtocol::Socks5, "127.0.0.1:1080"),
        ] {
            let settings = ProxySettings {
                mode: ProxyMode::Manual,
                protocol,
                address: address.to_string(),
            };
            assert!(http_client(Duration::from_secs(1), "test", &settings).is_ok());
        }
    }

    #[test]
    fn windows_system_proxy_parser_supports_hostnames_and_per_protocol_values() {
        assert_eq!(
            parse_windows_proxy("proxy.corp:8080", "localhost;*.internal"),
            Some((
                "http://proxy.corp:8080".to_string(),
                "localhost,*.internal".to_string()
            ))
        );
        assert_eq!(
            parse_windows_proxy(
                "http=proxy.corp:8080;https=secure.corp:8443;socks=socks.corp:1080",
                ""
            ),
            Some(("socks5h://socks.corp:1080".to_string(), String::new()))
        );
    }
}
