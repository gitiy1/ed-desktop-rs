use anyhow::{Context, Result, ensure};
use std::fs;
use std::path::Path;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeleteOutcome {
    Trashed,
    PermanentlyDeleted,
    AlreadyMissing,
}

pub trait FileActions: Send + Sync {
    fn reveal(&self, path: &Path, cx: &gpui::App) -> Result<()>;
    fn delete(&self, path: &Path) -> Result<DeleteOutcome>;
}

pub struct NativeFileActions;

impl FileActions for NativeFileActions {
    fn reveal(&self, path: &Path, cx: &gpui::App) -> Result<()> {
        let path = std::path::absolute(path).context("resolve downloaded file path")?;
        check_file(&path).context("locate downloaded file")?;
        cx.reveal_path(&path);
        Ok(())
    }

    fn delete(&self, path: &Path) -> Result<DeleteOutcome> {
        delete_with(path, recycle)
    }
}

fn recycle(path: &Path) -> Result<()> {
    // Windows shell recycling needs its own COM apartment. A fresh thread
    // avoids conflicting with a GPUI worker's existing COM initialization.
    #[cfg(windows)]
    {
        let path = path.to_path_buf();
        std::thread::spawn(move || trash::delete(path))
            .join()
            .map_err(|_| anyhow::anyhow!("Recycle bin worker failed"))?
            .map_err(Into::into)
    }
    #[cfg(target_os = "macos")]
    {
        use trash::macos::{DeleteMethod, TrashContextExtMacos};
        let mut context = trash::TrashContext::default();
        context.set_delete_method(DeleteMethod::NsFileManager);
        context.delete(path).map_err(Into::into)
    }
    #[cfg(all(not(windows), not(target_os = "macos")))]
    trash::delete(path).map_err(Into::into)
}

fn check_file(path: &Path) -> Result<()> {
    let metadata = fs::symlink_metadata(path)?;
    ensure!(
        metadata.is_file() || metadata.file_type().is_symlink(),
        "The download path is not a file"
    );
    Ok(())
}

fn delete_with(path: &Path, recycle: impl FnOnce(&Path) -> Result<()>) -> Result<DeleteOutcome> {
    match check_file(path) {
        Ok(()) => {}
        Err(error) if is_missing(&error) => return Ok(DeleteOutcome::AlreadyMissing),
        Err(error) => return Err(error.context("inspect downloaded file")),
    }
    match recycle(path) {
        Ok(()) => Ok(DeleteOutcome::Trashed),
        Err(trash_error) => match fs::remove_file(path) {
            Ok(()) => Ok(DeleteOutcome::PermanentlyDeleted),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                Ok(DeleteOutcome::AlreadyMissing)
            }
            Err(error) => Err(error).with_context(|| {
                format!("Recycle bin failed ({trash_error:#}); direct deletion also failed")
            }),
        },
    }
}

fn is_missing(error: &anyhow::Error) -> bool {
    error
        .downcast_ref::<std::io::Error>()
        .is_some_and(|error| error.kind() == std::io::ErrorKind::NotFound)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(target_os = "linux")]
    #[test]
    #[ignore = "uses the native recycle bin; requires an isolated temporary XDG_DATA_HOME"]
    fn native_recycle_bin_keeps_a_synthetic_export_recoverable() {
        let data_home = std::env::var_os("XDG_DATA_HOME")
            .map(std::path::PathBuf::from)
            .expect("set XDG_DATA_HOME to an empty temporary directory");
        let temporary = std::env::temp_dir().canonicalize().unwrap();
        let data_home = data_home.canonicalize().unwrap();
        assert!(data_home.starts_with(&temporary) && data_home != temporary);
        assert_eq!(fs::read_dir(&data_home).unwrap().count(), 0);
        let source_dir = tempfile::tempdir_in(data_home.parent().unwrap()).unwrap();
        let source = source_dir
            .path()
            .join("Synthetic 书名 - Author's Book.epub");
        fs::write(&source, b"synthetic exported EPUB").unwrap();
        assert_eq!(
            NativeFileActions.delete(&source).unwrap(),
            DeleteOutcome::Trashed
        );
        assert!(!source.exists());
        let files = fs::read_dir(data_home.join("Trash/files"))
            .unwrap()
            .collect::<std::io::Result<Vec<_>>>()
            .unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!(
            fs::read(files[0].path()).unwrap(),
            b"synthetic exported EPUB"
        );
        let info = fs::read_dir(data_home.join("Trash/info"))
            .unwrap()
            .collect::<std::io::Result<Vec<_>>>()
            .unwrap();
        assert_eq!(info.len(), 1);
        let metadata = fs::read_to_string(info[0].path()).unwrap();
        assert!(metadata.contains("[Trash Info]"));
        assert!(metadata.contains("Path="));
        assert!(metadata.contains("DeletionDate="));
    }

    #[test]
    fn recycling_preserves_file_contents_and_only_removes_the_selected_export() {
        let directory = tempfile::tempdir().unwrap();
        let output = directory.path().join("书名 - Author's Book.epub");
        let source = directory.path().join("书名 - Author's Book.books.epub");
        let recycled = directory.path().join("recycled.epub");
        fs::write(&output, b"export").unwrap();
        fs::write(&source, b"encrypted source").unwrap();
        let result = delete_with(&output, |path| {
            fs::rename(path, &recycled)?;
            Ok(())
        })
        .unwrap();
        assert_eq!(result, DeleteOutcome::Trashed);
        assert!(!output.exists());
        assert_eq!(fs::read(recycled).unwrap(), b"export");
        assert_eq!(fs::read(source).unwrap(), b"encrypted source");
    }

    #[test]
    fn unavailable_recycle_bin_falls_back_to_removing_the_file() {
        let directory = tempfile::tempdir().unwrap();
        let output = directory.path().join("book.epub");
        fs::write(&output, b"export").unwrap();
        let result = delete_with(&output, |_| anyhow::bail!("no recycle bin")).unwrap();
        assert_eq!(result, DeleteOutcome::PermanentlyDeleted);
        assert!(!output.exists());
        assert_eq!(
            delete_with(&output, |_| panic!("missing file must not be recycled")).unwrap(),
            DeleteOutcome::AlreadyMissing
        );
    }

    #[test]
    fn directory_paths_are_never_recycled_or_recursively_deleted() {
        let directory = tempfile::tempdir().unwrap();
        let child = directory.path().join("unrelated.epub");
        fs::write(&child, b"unrelated").unwrap();
        assert!(delete_with(directory.path(), |_| panic!("must reject a directory")).is_err());
        assert_eq!(fs::read(child).unwrap(), b"unrelated");
    }

    #[test]
    fn failure_of_both_methods_preserves_the_error() {
        let directory = tempfile::tempdir().unwrap();
        let output = directory.path().join("book.epub");
        fs::write(&output, b"export").unwrap();
        // A directory substituted during the recycle attempt must not be
        // recursively removed by the fallback.
        let error = delete_with(&output, |path| {
            fs::remove_file(path)?;
            fs::create_dir(path)?;
            anyhow::bail!("recycle failure")
        })
        .unwrap_err();
        let message = format!("{error:#}");
        assert!(message.contains("recycle failure"));
        assert!(message.contains("direct deletion also failed"));
        assert!(output.is_dir());
    }

    #[cfg(unix)]
    #[test]
    fn fallback_deletes_a_symlink_without_deleting_its_target() {
        let directory = tempfile::tempdir().unwrap();
        let target = directory.path().join("original.epub");
        let link = directory.path().join("link.epub");
        fs::write(&target, b"original").unwrap();
        std::os::unix::fs::symlink(&target, &link).unwrap();
        assert_eq!(
            delete_with(&link, |_| anyhow::bail!("no trash")).unwrap(),
            DeleteOutcome::PermanentlyDeleted
        );
        assert!(fs::symlink_metadata(link).is_err());
        assert_eq!(fs::read(target).unwrap(), b"original");
    }
}
