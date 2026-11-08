use crate::network;
use crate::settings::ProxySettings;
use anyhow::{Context, Result, anyhow};
use reqwest::blocking::{Client, Response};
use reqwest::header::{ACCEPT, CONTENT_TYPE, HeaderMap, HeaderValue, USER_AGENT};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::time::Duration;
use uuid::Uuid;

pub const DEFAULT_API_BASE: &str = "https://vbookuk.click/api/v4/";
const DEFAULT_VERSION: &str = "2.5.0-2-google";
const DEFAULT_DEVICE_NAME: &str = "Android Device";
const DEFAULT_DEVICE_TYPE: &str = "Android SDK_35";
const API_FALLBACKS: &[&str] = &[
    "https://a.edapi.top/api/v4/",
    "https://ebook.endao.co/api/v4/",
    "https://a.ebookrealm.uk/api/v4/",
    "https://ebookstore.endao.co/api/v4/",
];

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Session {
    pub email: String,
    pub access_token: String,
    pub token: String,
    pub device_id: String,
    pub device_name: String,
    pub device_type: String,
    pub version: String,
    pub api_base: String,
}

#[derive(Clone, Debug)]
pub struct Book {
    pub raw: Value,
}

impl Book {
    pub fn id(&self) -> String {
        value_string(&self.raw, &["bookId", "id"]).unwrap_or_else(|| "?".to_string())
    }

    pub fn title(&self) -> String {
        value_string(&self.raw, &["name", "title", "bookName"])
            .unwrap_or_else(|| "Untitled book".to_string())
    }

    pub fn author(&self) -> String {
        value_string(&self.raw, &["author", "authorName"]).unwrap_or_default()
    }

    pub fn filename(&self) -> Option<String> {
        value_string(&self.raw, &["fileName", "filename"])
    }
}

fn value_string(value: &Value, keys: &[&str]) -> Option<String> {
    keys.iter().find_map(|key| match &value[*key] {
        Value::String(text) if !text.is_empty() => Some(text.clone()),
        Value::Number(number) => Some(number.to_string()),
        _ => None,
    })
}

pub fn latest_version(proxy: &ProxySettings) -> String {
    let client = match network::http_client(Duration::from_secs(8), "okhttp/4.12.0", proxy) {
        Ok(client) => client,
        Err(_) => return DEFAULT_VERSION.to_string(),
    };

    if let Ok(response) = client
        .get("https://20.189.93.112/android/newest.json")
        .send()
        && let Ok(payload) = response.error_for_status()
        && let Ok(value) = payload.json::<Value>()
        && let Some(version) = value_string(&value, &["versionName", "version"])
    {
        return version;
    }

    let play_url = "https://play.google.com/store/apps/details?id=life.tti.readerui&hl=en&gl=US";
    if let Ok(response) = client
        .get(play_url)
        .header(USER_AGENT, "Mozilla/5.0")
        .send()
        && let Ok(response) = response.error_for_status()
        && let Ok(html) = response.text()
    {
        for pattern in [
            r#""softwareVersion"\s*:\s*"([^"]+)""#,
            r#"Current Version.{0,300}?>([0-9][^<]+)<"#,
        ] {
            if let Ok(regex) = regex::Regex::new(pattern)
                && let Some(captures) = regex.captures(&html)
                && let Some(version) = captures.get(1)
            {
                return version.as_str().trim().to_string();
            }
        }
    }
    DEFAULT_VERSION.to_string()
}

fn headers(session: &Session) -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert("currency", HeaderValue::from_static("CNY"));
    headers.insert("lang", HeaderValue::from_static("zh_CN"));
    headers.insert("df", HeaderValue::from_static("a"));
    headers.insert(
        "v",
        HeaderValue::from_str(&session.version)
            .unwrap_or(HeaderValue::from_static(DEFAULT_VERSION)),
    );
    headers.insert(ACCEPT, HeaderValue::from_static("application/json"));
    headers.insert(
        CONTENT_TYPE,
        HeaderValue::from_static("application/json; charset=UTF-8"),
    );
    headers.insert(
        "access_token",
        HeaderValue::from_str(&session.access_token)
            .unwrap_or_else(|_| HeaderValue::from_static("")),
    );
    headers.insert(
        "token",
        HeaderValue::from_str(&session.token).unwrap_or_else(|_| HeaderValue::from_static("")),
    );
    headers.insert(
        "email",
        HeaderValue::from_str(&session.email).unwrap_or_else(|_| HeaderValue::from_static("")),
    );
    headers
}

fn parse_payload(response: Response, endpoint: &str) -> Result<Value> {
    let status = response.status();
    let body = response.text().context("read Endao response")?;
    if !status.is_success() {
        return Err(anyhow!(
            "{endpoint}: HTTP {status}: {}",
            body.chars().take(500).collect::<String>()
        ));
    }
    let payload: Value = serde_json::from_str(&body)
        .map_err(|error| anyhow!("{endpoint}: invalid JSON response ({error})"))?;
    if payload.get("success") == Some(&Value::Bool(false)) {
        let message = payload.get("errorMessages").or(payload.get("message"));
        return Err(anyhow!("{endpoint}: {}", message.unwrap_or(&payload)));
    }
    Ok(payload)
}

pub fn login(
    email: &str,
    password: &str,
    version: String,
    proxy: &ProxySettings,
) -> Result<Session> {
    let email = email.trim().to_lowercase();
    if email.is_empty() || password.is_empty() {
        return Err(anyhow!("Enter both email and password."));
    }
    let device_id = Uuid::new_v4().simple().to_string();
    let client = network::http_client(Duration::from_secs(90), "okhttp/4.12.0", proxy)?;
    let mut bases = vec![DEFAULT_API_BASE.to_string()];
    bases.extend(API_FALLBACKS.iter().map(|base| base.to_string()));
    let mut errors = Vec::new();

    for base in bases {
        let url = format!("{}/user/login", base.trim_end_matches('/'));
        let response = client
            .post(&url)
            .header("currency", "CNY")
            .header("lang", "zh_CN")
            .header("df", "a")
            .header("v", &version)
            .header(ACCEPT, "application/json")
            .json(&json!({
                "email": email,
                "passwd": password,
                "deviceID": device_id,
                "deviceName": DEFAULT_DEVICE_NAME,
                "deviceType": DEFAULT_DEVICE_TYPE,
            }))
            .send();

        let payload = match response {
            Ok(response) if response.status().is_client_error() => {
                let status = response.status();
                if status == reqwest::StatusCode::NOT_FOUND {
                    errors.push(format!("{base}: HTTP {status}"));
                    continue;
                }
                return Err(anyhow!("Login was rejected by Endao (HTTP {status})."));
            }
            Ok(response) if response.status().is_server_error() => {
                let status = response.status();
                errors.push(format!("{base}: HTTP {status}"));
                continue;
            }
            Ok(response) => parse_payload(response, "user/login")?,
            Err(error) => {
                errors.push(format!("{base}: {error}"));
                continue;
            }
        };

        let result = payload.get("result").cloned().unwrap_or(Value::Null);
        let access_token = value_string(&result, &["access_token", "accessToken"]);
        let token = value_string(&result, &["token"]).or_else(|| access_token.clone());
        match (access_token, token) {
            (Some(access_token), Some(token)) => {
                return Ok(Session {
                    email,
                    access_token,
                    token,
                    device_id,
                    device_name: DEFAULT_DEVICE_NAME.to_string(),
                    device_type: DEFAULT_DEVICE_TYPE.to_string(),
                    version,
                    api_base: base,
                });
            }
            _ => return Err(anyhow!("Login response did not contain an access token.")),
        }
    }
    Err(anyhow!(
        "All Endao API hosts failed: {}",
        errors.join(" | ")
    ))
}

fn post_json(client: &Client, session: &Session, endpoint: &str, body: Value) -> Result<Value> {
    let base = format!("{}/", session.api_base.trim_end_matches('/'));
    let response = client
        .post(format!("{base}{endpoint}"))
        .headers(headers(session))
        .json(&body)
        .send()
        .with_context(|| format!("request {endpoint}"))?;
    parse_payload(response, endpoint)
}

pub fn purchased_books(session: &Session, proxy: &ProxySettings) -> Result<Vec<Book>> {
    let client = network::http_client(Duration::from_secs(90), "okhttp/4.12.0", proxy)?;
    let mut books = Vec::new();
    let mut page = 1;
    loop {
        let payload = post_json(
            &client,
            session,
            "book/booksPage",
            json!({ "email": session.email, "page": page }),
        )?;
        let rows = payload
            .get("result")
            .and_then(Value::as_array)
            .ok_or_else(|| anyhow!("book/booksPage returned an invalid result list"))?;
        books.extend(
            rows.iter()
                .filter(|row| row.is_object())
                .cloned()
                .map(|raw| Book { raw }),
        );
        let total_page = payload
            .get("totalPage")
            .and_then(Value::as_u64)
            .unwrap_or(page as u64) as usize;
        if page >= total_page || rows.is_empty() {
            break;
        }
        page += 1;
    }
    Ok(books)
}

pub fn old_download_request(
    client: &Client,
    session: &Session,
    book: &Book,
) -> reqwest::blocking::RequestBuilder {
    let base = format!("{}/", session.api_base.trim_end_matches('/'));
    client
        .post(format!("{base}book/download"))
        .headers(headers(session))
        .json(&json!({
            "bookId": book.id(),
            "email": session.email,
            "deviceID": session.device_id,
        }))
}
