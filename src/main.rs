mod api;
mod downloader;
mod filenames;
mod i18n;
mod lcp;
mod network;
mod settings;
mod ui;

fn main() {
    let arguments: Vec<_> = std::env::args().skip(1).collect();
    let demo_page = arguments
        .iter()
        .position(|arg| arg == "--demo")
        .map(|index| {
            arguments
                .get(index + 1)
                .cloned()
                .unwrap_or_else(|| "library".to_string())
        });
    if demo_page.is_none() && dotenvy::dotenv().is_err() {
        let _ = dotenvy::from_path("../.env");
    }
    ui::run(demo_page);
}

#[cfg(test)]
mod live_tests {
    use crate::{api, downloader, lcp};
    use std::fs::File;
    use std::io::Read;
    use zip::ZipArchive;

    #[test]
    #[ignore = "makes a real Endao login and downloads a purchased EPUB"]
    fn live_old_path_downloads_and_exports_a_clean_epub() {
        if std::env::var_os("ED_EMAIL").is_none() {
            let _ = dotenvy::from_path("../.env");
        }
        let email = std::env::var("ED_EMAIL").expect("ED_EMAIL is required");
        let password = std::env::var("ED_PASSWORD").expect("ED_PASSWORD is required");
        let proxy = crate::settings::ProxySettings::default();
        let session = api::login(&email, &password, api::latest_version(&proxy), &proxy).unwrap();
        let books = api::purchased_books(&session, &proxy).unwrap();
        let book = books
            .first()
            .expect("purchased bookshelf must contain at least one book");
        let directory = tempfile::tempdir().unwrap();
        let stems = crate::filenames::export_stems(&books);
        let stem = &stems[&book.id()];
        let encrypted = directory.path().join(format!("{stem}.books.epub"));
        downloader::download_legacy(
            &session,
            book,
            &encrypted,
            4,
            &proxy,
            std::sync::Arc::new(downloader::DownloadControl::default()),
            |_| {},
        )
        .unwrap();
        let plain = directory.path().join(format!("{stem}.epub"));
        lcp::decrypt_epub(&encrypted, &plain, &[session.email]).unwrap();

        let mut archive = ZipArchive::new(File::open(plain).unwrap()).unwrap();
        assert!(archive.by_name("META-INF/license.lcpl").is_err());
        assert!(archive.by_name("META-INF/encryption.xml").is_err());
        for index in 0..archive.len() {
            let mut entry = archive.by_index(index).unwrap();
            let mut contents = Vec::new();
            entry.read_to_end(&mut contents).unwrap();
        }
    }
}
