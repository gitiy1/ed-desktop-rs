use crate::lcp;
use anyhow::{Context, Result, ensure};
use std::fs;
use std::path::{Path, PathBuf};

pub struct ExportedBook {
    pub path: PathBuf,
    pub decrypted: bool,
    pub cleanup_warning: Option<String>,
}

pub fn finish_download(
    encrypted: &Path,
    plain: &Path,
    auto_decrypt: bool,
    keep_encrypted: bool,
    passphrases: &[String],
) -> Result<ExportedBook> {
    finish_with(encrypted, plain, auto_decrypt, keep_encrypted, |staged| {
        lcp::decrypt_epub(encrypted, staged, passphrases).map(|_| ())
    })
}

fn finish_with(
    encrypted: &Path,
    plain: &Path,
    auto_decrypt: bool,
    keep_encrypted: bool,
    decrypt: impl FnOnce(&Path) -> Result<()>,
) -> Result<ExportedBook> {
    if !auto_decrypt {
        return Ok(ExportedBook {
            path: encrypted.to_path_buf(),
            decrypted: false,
            cleanup_warning: None,
        });
    }
    ensure!(
        encrypted != plain,
        "Export must use a different path from its encrypted source"
    );
    let parent = plain.parent().unwrap_or_else(|| Path::new("."));
    let staged = tempfile::Builder::new()
        .prefix(".endao-export-")
        .suffix(".epub")
        .tempfile_in(parent)
        .context("create temporary EPUB export")?;
    decrypt(staged.path()).context("export decrypted EPUB; encrypted source retained")?;
    staged.as_file().sync_all().context("sync exported EPUB")?;
    staged
        .persist(plain)
        .context("save exported EPUB; encrypted source retained")?;
    // Only a committed, successful export permits source deletion. A cleanup
    // failure is a warning: the usable export must remain a completed task.
    let cleanup_warning = if keep_encrypted {
        None
    } else {
        fs::remove_file(encrypted)
            .err()
            .map(|error| error.to_string())
    };
    Ok(ExportedBook {
        path: plain.to_path_buf(),
        decrypted: true,
        cleanup_warning,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn successful_export_cleans_source_only_when_enabled() {
        for keep in [false, true] {
            let directory = tempfile::tempdir().unwrap();
            let encrypted = directory.path().join("book.books.epub");
            let plain = directory.path().join("book.epub");
            fs::write(&encrypted, b"encrypted").unwrap();
            fs::write(&plain, b"old export").unwrap();
            let result = finish_with(&encrypted, &plain, true, keep, |path| {
                fs::write(path, b"exported")?;
                Ok(())
            })
            .unwrap();
            assert!(result.decrypted);
            assert!(result.cleanup_warning.is_none());
            assert_eq!(fs::read(&plain).unwrap(), b"exported");
            assert_eq!(encrypted.exists(), keep);
            assert_eq!(
                fs::read_dir(directory.path()).unwrap().count(),
                if keep { 2 } else { 1 }
            );
        }
    }

    #[test]
    fn failed_export_preserves_source_and_previous_export_and_removes_staging_file() {
        let directory = tempfile::tempdir().unwrap();
        let encrypted = directory.path().join("book.books.epub");
        let plain = directory.path().join("book.epub");
        fs::write(&encrypted, b"encrypted").unwrap();
        fs::write(&plain, b"previous export").unwrap();
        let result = finish_with(&encrypted, &plain, true, false, |path| {
            fs::write(path, b"incomplete")?;
            anyhow::bail!("export failed")
        });
        assert!(result.is_err());
        assert_eq!(fs::read(&encrypted).unwrap(), b"encrypted");
        assert_eq!(fs::read(&plain).unwrap(), b"previous export");
        assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 2);
    }

    #[test]
    fn download_without_export_always_keeps_source() {
        let directory = tempfile::tempdir().unwrap();
        let encrypted = directory.path().join("book.books.epub");
        let plain = directory.path().join("book.epub");
        fs::write(&encrypted, b"encrypted").unwrap();
        let result = finish_with(&encrypted, &plain, false, false, |_| {
            panic!("must not decrypt")
        })
        .unwrap();
        assert!(!result.decrypted);
        assert!(encrypted.exists());
        assert!(!plain.exists());
    }

    #[test]
    fn cleanup_failure_reports_a_warning_and_keeps_the_successful_export() {
        let directory = tempfile::tempdir().unwrap();
        let encrypted = directory.path().join("source-cannot-be-removed-as-a-file");
        fs::create_dir(&encrypted).unwrap();
        let plain = directory.path().join("book.epub");
        let result = finish_with(&encrypted, &plain, true, false, |path| {
            fs::write(path, b"exported")?;
            Ok(())
        })
        .unwrap();
        assert!(result.decrypted);
        assert!(result.cleanup_warning.is_some());
        assert_eq!(fs::read(plain).unwrap(), b"exported");
    }
}
