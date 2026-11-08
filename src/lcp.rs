use aes::Aes256;
use anyhow::{Context, Result, anyhow};
use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use cbc::Decryptor;
use cipher::{BlockDecryptMut, KeyIvInit, block_padding::NoPadding};
use flate2::read::DeflateDecoder;
use percent_encoding::percent_decode_str;
use roxmltree::Document;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs::File;
use std::io::{Cursor, Read, Write};
use std::path::{Path, PathBuf};
use xmltree::{Element, XMLNode};
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipArchive, ZipWriter};

const PROFILE10_MASTER_KEY: [u8; 64] = [
    0xb3, 0xa0, 0x7c, 0x4d, 0x42, 0x88, 0x0e, 0x69, 0x39, 0x8e, 0x05, 0x39, 0x24, 0x05, 0x05, 0x0e,
    0xfe, 0xea, 0x06, 0x64, 0xc0, 0xb6, 0x38, 0xb7, 0xc9, 0x86, 0x55, 0x6f, 0xa9, 0xb5, 0x8d, 0x77,
    0xb3, 0x1a, 0x40, 0xeb, 0x6a, 0x4f, 0xdb, 0xa1, 0xe4, 0x53, 0x72, 0x29, 0xd9, 0xf7, 0x79, 0xda,
    0xad, 0x1c, 0xc4, 0x1e, 0xe9, 0x68, 0x15, 0x3c, 0xb7, 0x1f, 0x27, 0xdc, 0x96, 0x96, 0xd4, 0x0f,
];

#[derive(Debug)]
struct EncryptedResource {
    uri: String,
    algorithm: Option<String>,
    compression: Option<String>,
    original_length: Option<usize>,
}

pub fn decrypt_epub(input: &Path, output: &Path, passphrases: &[String]) -> Result<PathBuf> {
    let file = File::open(input).with_context(|| format!("open {}", input.display()))?;
    let mut archive = ZipArchive::new(file).context("read EPUB ZIP archive")?;
    let names: Vec<String> = (0..archive.len())
        .map(|index| {
            archive
                .by_index(index)
                .map(|entry| entry.name().to_string())
        })
        .collect::<std::result::Result<_, _>>()?;
    let mut files = HashMap::new();
    for name in &names {
        let mut entry = archive
            .by_name(name)
            .with_context(|| format!("read EPUB member {name}"))?;
        let mut data = Vec::with_capacity(entry.size() as usize);
        entry
            .read_to_end(&mut data)
            .with_context(|| format!("read EPUB member {name}"))?;
        files.insert(name.clone(), data);
    }
    drop(archive);

    let license_bytes = files
        .get("META-INF/license.lcpl")
        .ok_or_else(|| anyhow!("EPUB does not contain META-INF/license.lcpl"))?;
    let license: Value = serde_json::from_slice(license_bytes).context("parse LCP license")?;
    let license_id = license
        .get("id")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let enc_resources = files
        .get("META-INF/encryption.xml")
        .map(|xml| parse_encryption_xml(xml))
        .transpose()?
        .unwrap_or_default();
    if enc_resources.is_empty() {
        return Err(anyhow!(
            "EPUB has no encrypted resources in META-INF/encryption.xml"
        ));
    }
    let user_key = find_user_key(&license, license_id, passphrases)?;
    let content_value = license
        .pointer("/encryption/content_key/encrypted_value")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("LCP license has no encrypted content key"))?;
    let content_key = unwrap_value(content_value, &user_key).context("unwrap LCP content key")?;
    if content_key.len() != 32 {
        return Err(anyhow!(
            "Unexpected LCP content key size: {}",
            content_key.len()
        ));
    }

    for resource in enc_resources {
        if resource
            .algorithm
            .as_deref()
            .is_some_and(|algorithm| !algorithm.ends_with("aes256-cbc"))
        {
            return Err(anyhow!(
                "Unsupported encryption algorithm for {}",
                resource.uri
            ));
        }
        let data = files.get_mut(&resource.uri).ok_or_else(|| {
            anyhow!(
                "encryption.xml references missing resource {}",
                resource.uri
            )
        })?;
        let decrypted = decrypt_resource(&content_key, data)
            .with_context(|| format!("decrypt EPUB resource {}", resource.uri))?;
        let plain = if resource.compression.as_deref() == Some("8") {
            let mut decoder = DeflateDecoder::new(decrypted.as_slice());
            let mut output = Vec::new();
            decoder
                .read_to_end(&mut output)
                .context("inflate EPUB resource")?;
            output
        } else {
            decrypted
        };
        if let Some(expected) = resource.original_length
            && plain.len() != expected
        {
            return Err(anyhow!(
                "{} expanded to {} bytes, expected {expected}",
                resource.uri,
                plain.len()
            ));
        }
        *data = plain;
    }

    if passphrases.is_empty() {
        return Err(anyhow!("No account passphrase was provided."));
    }
    write_sanitized_epub(output, &names, &files, passphrases, license_id)?;
    Ok(output.to_path_buf())
}

fn parse_encryption_xml(bytes: &[u8]) -> Result<Vec<EncryptedResource>> {
    let text = std::str::from_utf8(bytes).context("encryption.xml is not UTF-8")?;
    let document = Document::parse(text).context("parse encryption.xml")?;
    let mut resources = Vec::new();
    for encrypted_data in document
        .descendants()
        .filter(|node| node.has_tag_name("EncryptedData"))
    {
        let algorithm = encrypted_data
            .descendants()
            .find(|node| node.has_tag_name("EncryptionMethod"))
            .and_then(|node| node.attribute("Algorithm"))
            .map(str::to_string);
        let reference = encrypted_data
            .descendants()
            .find(|node| node.has_tag_name("CipherReference"));
        let Some(uri) = reference.and_then(|node| node.attribute("URI")) else {
            continue;
        };
        let compression = encrypted_data
            .descendants()
            .find(|node| node.has_tag_name("Compression"));
        let original_length = compression
            .and_then(|node| node.attribute("OriginalLength"))
            .and_then(|value| value.parse().ok());
        resources.push(EncryptedResource {
            uri: percent_decode_str(uri)
                .decode_utf8_lossy()
                .trim_start_matches('/')
                .to_string(),
            algorithm,
            compression: compression
                .and_then(|node| node.attribute("Method"))
                .map(str::to_string),
            original_length,
        });
    }
    Ok(resources)
}

fn find_user_key(license: &Value, license_id: &str, passphrases: &[String]) -> Result<Vec<u8>> {
    let key_check = license
        .pointer("/encryption/user_key/key_check")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("LCP license has no user_key.key_check"))?;
    let encrypted = decode_base64(key_check)?;
    let (iv, ciphertext) = split_iv(&encrypted)?;
    for passphrase in passphrases {
        let digest = Sha256::digest(passphrase.as_bytes()).to_vec();
        for candidate in [digest.clone(), profile10_transform(digest)] {
            if let Ok(plain) = decrypt_aes_cbc(&candidate, iv, ciphertext)
                && plain.starts_with(license_id.as_bytes())
            {
                return Ok(candidate);
            }
        }
    }
    Err(anyhow!("The account email did not match this LCP license."))
}

fn profile10_transform(mut current: Vec<u8>) -> Vec<u8> {
    for byte in PROFILE10_MASTER_KEY {
        current.push(byte);
        current = Sha256::digest(&current).to_vec();
    }
    current
}

fn unwrap_value(value: &str, key: &[u8]) -> Result<Vec<u8>> {
    let encrypted = decode_base64(value)?;
    let (iv, ciphertext) = split_iv(&encrypted)?;
    let plain = decrypt_aes_cbc(key, iv, ciphertext)?;
    Ok(strip_padding(plain))
}

fn decrypt_resource(key: &[u8], encrypted: &[u8]) -> Result<Vec<u8>> {
    let (iv, ciphertext) = split_iv(encrypted)?;
    let plain = decrypt_aes_cbc(key, iv, ciphertext)?;
    Ok(strip_padding(plain))
}

fn decrypt_aes_cbc(key: &[u8], iv: &[u8], ciphertext: &[u8]) -> Result<Vec<u8>> {
    if key.len() != 32
        || iv.len() != 16
        || ciphertext.is_empty()
        || !ciphertext.len().is_multiple_of(16)
    {
        return Err(anyhow!("Invalid AES-256-CBC input length."));
    }
    let mut buffer = ciphertext.to_vec();
    let decrypted = Decryptor::<Aes256>::new_from_slices(key, iv)
        .map_err(|_| anyhow!("Invalid AES key or IV"))?
        .decrypt_padded_mut::<NoPadding>(&mut buffer)
        .map_err(|_| anyhow!("AES-CBC decryption failed"))?;
    Ok(decrypted.to_vec())
}

fn strip_padding(mut data: Vec<u8>) -> Vec<u8> {
    if let Some(&count) = data.last() {
        let count = count as usize;
        if (1..=16).contains(&count) && data.len() >= count {
            data.truncate(data.len() - count);
        }
    }
    data
}

fn decode_base64(value: &str) -> Result<Vec<u8>> {
    let mut padded = value.trim().to_string();
    while !padded.len().is_multiple_of(4) {
        padded.push('=');
    }
    STANDARD.decode(padded).context("decode LCP base64 value")
}

fn split_iv(bytes: &[u8]) -> Result<(&[u8], &[u8])> {
    if bytes.len() < 32 || !(bytes.len() - 16).is_multiple_of(16) {
        return Err(anyhow!("Encrypted LCP value has an invalid length."));
    }
    Ok((&bytes[..16], &bytes[16..]))
}

fn write_sanitized_epub(
    output: &Path,
    names: &[String],
    files: &HashMap<String, Vec<u8>>,
    passphrases: &[String],
    license_id: &str,
) -> Result<()> {
    if let Some(parent) = output.parent() {
        std::fs::create_dir_all(parent).with_context(|| format!("create {}", parent.display()))?;
    }
    let file = File::create(output).with_context(|| format!("create {}", output.display()))?;
    let mut archive = ZipWriter::new(file);
    let account_markers: Vec<String> = passphrases
        .iter()
        .filter(|value| value.contains('@'))
        .map(|value| value.to_lowercase())
        .collect();
    let options = |stored: bool| {
        SimpleFileOptions::default().compression_method(if stored {
            CompressionMethod::Stored
        } else {
            CompressionMethod::Deflated
        })
    };
    if let Some(mimetype) = files.get("mimetype") {
        archive.start_file("mimetype", options(true))?;
        archive.write_all(mimetype)?;
    }
    for name in names {
        if name == "mimetype"
            || name == "META-INF/license.lcpl"
            || name == "META-INF/encryption.xml"
            || name == "META-INF/calibre_bookmarks.txt"
            || name == "META-INF/rights.xml"
            || name == "META-INF/signatures.xml"
        {
            continue;
        }
        let Some(mut bytes) = files.get(name).cloned() else {
            continue;
        };
        if name.to_ascii_lowercase().ends_with(".opf") {
            bytes = sanitize_opf(&bytes, &account_markers, license_id).unwrap_or(bytes);
        }
        archive.start_file(name, options(false))?;
        archive.write_all(&bytes)?;
    }
    archive
        .finish()?
        .sync_all()
        .context("sync decrypted EPUB")?;
    Ok(())
}

fn sanitize_opf(bytes: &[u8], account_markers: &[String], license_id: &str) -> Result<Vec<u8>> {
    let mut root = Element::parse(Cursor::new(bytes)).context("parse OPF metadata")?;
    sanitize_element(&mut root, account_markers, license_id);
    let mut output = Vec::new();
    root.write(&mut output)
        .context("write cleaned OPF metadata")?;
    Ok(output)
}

fn sanitize_element(element: &mut Element, account_markers: &[String], license_id: &str) {
    element.children.retain_mut(|node| {
        let XMLNode::Element(child) = node else {
            return true;
        };
        let name = child.name.to_ascii_lowercase();
        let attrs = child
            .attributes
            .iter()
            .map(|(key, value)| {
                format!(
                    "{} {}",
                    key.to_ascii_lowercase(),
                    value.to_ascii_lowercase()
                )
            })
            .collect::<String>();
        let text = child
            .get_text()
            .map(|value| value.to_ascii_lowercase())
            .unwrap_or_default();
        let has_account = account_markers.iter().any(|marker| text.contains(marker))
            || (!license_id.is_empty() && text.contains(&license_id.to_ascii_lowercase()));
        let account_property = name == "meta"
            && ["license", "lcp", "account", "email", "user"]
                .iter()
                .any(|marker| attrs.contains(marker));
        if has_account || account_property {
            false
        } else {
            sanitize_element(child, account_markers, license_id);
            true
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use cbc::Encryptor;
    use cipher::{BlockEncryptMut, block_padding::Pkcs7};
    use flate2::Compression;
    use flate2::write::DeflateEncoder;
    use serde_json::json;
    use zip::write::SimpleFileOptions;

    const PASSPHRASE: &str = "reader@example.com";
    const LICENSE_ID: &str = "license-test-001";
    const CONTENT_KEY: [u8; 32] = [0x42; 32];

    fn encrypt(key: &[u8], iv: &[u8], value: &[u8]) -> Vec<u8> {
        let mut buffer = value.to_vec();
        let length = buffer.len();
        buffer.resize(length + 16, 0);
        Encryptor::<Aes256>::new_from_slices(key, iv)
            .unwrap()
            .encrypt_padded_mut::<Pkcs7>(&mut buffer, length)
            .unwrap()
            .to_vec()
    }

    fn encoded_value(key: &[u8], iv: &[u8], value: &[u8]) -> String {
        let encrypted = encrypt(key, iv, value);
        let mut payload = iv.to_vec();
        payload.extend(encrypted);
        STANDARD.encode(payload)
    }

    #[test]
    fn decrypts_and_sanitizes_a_profile_10_epub() {
        let directory = tempfile::tempdir().unwrap();
        let encrypted_epub = directory.path().join("encrypted.epub");
        let plain_epub = directory.path().join("plain.epub");
        let user_key = profile10_transform(Sha256::digest(PASSPHRASE.as_bytes()).to_vec());
        let iv_check = [0x11; 16];
        let iv_content = [0x22; 16];
        let iv_resource = [0x33; 16];
        let key_check = encoded_value(&user_key, &iv_check, LICENSE_ID.as_bytes());
        let encrypted_content_key = encoded_value(&user_key, &iv_content, &CONTENT_KEY);

        let plaintext = b"<html><body>decoded chapter</body></html>";
        let mut deflater = DeflateEncoder::new(Vec::new(), Compression::default());
        deflater.write_all(plaintext).unwrap();
        let compressed = deflater.finish().unwrap();
        let encrypted_resource = encrypt(&CONTENT_KEY, &iv_resource, &compressed);
        let mut resource_value = iv_resource.to_vec();
        resource_value.extend(encrypted_resource);

        let license = json!({
            "id": LICENSE_ID,
            "encryption": {
                "user_key": { "key_check": key_check },
                "content_key": { "encrypted_value": encrypted_content_key }
            }
        });
        let encryption_xml = format!(
            r#"<encryption xmlns="urn:oasis:names:tc:opendocument:xmlns:container"><EncryptedData><EncryptionMethod Algorithm="http://www.w3.org/2001/04/xmlenc#aes256-cbc"/><CipherData><CipherReference URI="OEBPS/chapter.xhtml"/></CipherData><Compression Method="8" OriginalLength="{}"/></EncryptedData></encryption>"#,
            plaintext.len()
        );
        let opf = format!(
            r#"<package xmlns:dc="http://purl.org/dc/elements/1.1/"><metadata><dc:title>Sample</dc:title><dc:creator>{PASSPHRASE}</dc:creator><meta property="lcp:license">account-only</meta></metadata></package>"#
        );

        let file = File::create(&encrypted_epub).unwrap();
        let mut archive = ZipWriter::new(file);
        let deflated = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
        archive
            .start_file(
                "mimetype",
                SimpleFileOptions::default().compression_method(CompressionMethod::Stored),
            )
            .unwrap();
        archive.write_all(b"application/epub+zip").unwrap();
        archive
            .start_file("META-INF/license.lcpl", deflated)
            .unwrap();
        archive
            .write_all(serde_json::to_string(&license).unwrap().as_bytes())
            .unwrap();
        archive
            .start_file("META-INF/encryption.xml", deflated)
            .unwrap();
        archive.write_all(encryption_xml.as_bytes()).unwrap();
        archive.start_file("OEBPS/content.opf", deflated).unwrap();
        archive.write_all(opf.as_bytes()).unwrap();
        archive.start_file("OEBPS/chapter.xhtml", deflated).unwrap();
        archive.write_all(&resource_value).unwrap();
        archive.finish().unwrap();

        decrypt_epub(&encrypted_epub, &plain_epub, &[PASSPHRASE.to_string()]).unwrap();

        let mut archive = ZipArchive::new(File::open(plain_epub).unwrap()).unwrap();
        assert!(archive.by_name("META-INF/license.lcpl").is_err());
        assert!(archive.by_name("META-INF/encryption.xml").is_err());
        let mut chapter = String::new();
        archive
            .by_name("OEBPS/chapter.xhtml")
            .unwrap()
            .read_to_string(&mut chapter)
            .unwrap();
        assert_eq!(chapter.as_bytes(), plaintext);
        let mut opf = String::new();
        archive
            .by_name("OEBPS/content.opf")
            .unwrap()
            .read_to_string(&mut opf)
            .unwrap();
        assert!(!opf.contains(PASSPHRASE));
        assert!(!opf.contains("lcp:license"));
    }
}
