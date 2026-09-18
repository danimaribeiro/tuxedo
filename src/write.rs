use std::io::Write;
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};

/// Replaces `path`'s contents with `contents` in one step.
///
/// The formatter rewrites a file whose only copy is the one on disk, so a
/// write interrupted halfway -- a full disk, a killed process -- would
/// leave source code truncated with nothing to restore it from. Writing a
/// sibling file and renaming it over the target means a reader sees
/// either the whole old file or the whole new one. `rename` is atomic
/// only within a filesystem, which is why the temporary cannot live in
/// the system temp directory: it goes next to the file it will replace.
pub fn write_atomic(path: &Path, contents: &str) -> std::io::Result<()> {
    let dir = path.parent().unwrap_or_else(|| Path::new(""));
    let dir = if dir.as_os_str().is_empty() {
        Path::new(".")
    } else {
        dir
    };
    let temp = dir.join(temp_name(path));

    let result = replace(&temp, path, contents);
    if result.is_err() {
        // A failed write must not leave a dotfile beside the user's
        // source that nothing later cleans up.
        let _ = std::fs::remove_file(&temp);
    }
    result
}

fn replace(temp: &Path, path: &Path, contents: &str) -> std::io::Result<()> {
    let mut file = std::fs::File::create(temp)?;
    file.write_all(contents.as_bytes())?;
    drop(file);

    // A file created from scratch takes the process umask, so the mode on
    // the source -- an executable script under `bin/`, a file the user
    // made read-only -- would be dropped without carrying it across.
    if let Ok(metadata) = std::fs::metadata(path) {
        std::fs::set_permissions(temp, metadata.permissions())?;
    }

    std::fs::rename(temp, path)
}

/// A name no other write can be using. The walk formats files on several
/// threads at once, and two of them in one directory would otherwise race
/// for the same temporary -- one rename would land, the other would find
/// its file already gone.
fn temp_name(path: &Path) -> String {
    static COUNTER: AtomicUsize = AtomicUsize::new(0);

    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("source");
    let id = std::process::id();
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    format!(".{name}.tuxedo-{id}-{n}.tmp")
}

#[cfg(test)]
mod tests {
    use super::write_atomic;

    /// A directory of this session's making, removed when the test ends.
    struct TempDir(std::path::PathBuf);

    impl TempDir {
        fn new(name: &str) -> Self {
            let dir =
                std::env::temp_dir().join(format!("tuxedo-write-{}-{name}", std::process::id()));
            std::fs::create_dir_all(&dir).unwrap();
            Self(dir)
        }

        fn join(&self, name: &str) -> std::path::PathBuf {
            self.0.join(name)
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn replaces_the_file_and_leaves_no_temporary_behind() {
        let dir = TempDir::new("replace");
        let path = dir.join("a.rb");
        std::fs::write(&path, "x = 1\n").unwrap();

        write_atomic(&path, "x = 2\n").unwrap();

        assert_eq!(std::fs::read_to_string(&path).unwrap(), "x = 2\n");
        let left_over: Vec<_> = std::fs::read_dir(&dir.0)
            .unwrap()
            .map(|entry| entry.unwrap().file_name())
            .collect();
        assert_eq!(left_over, ["a.rb"]);
    }

    #[cfg(unix)]
    #[test]
    fn keeps_the_mode_the_file_had() {
        use std::os::unix::fs::PermissionsExt;

        let dir = TempDir::new("mode");
        let path = dir.join("script.rb");
        std::fs::write(&path, "puts 1\n").unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();

        write_atomic(&path, "puts 2\n").unwrap();

        let mode = std::fs::metadata(&path).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o755);
    }

    #[test]
    fn a_write_that_cannot_land_reports_it() {
        let dir = TempDir::new("missing");
        let path = dir.join("no-such-dir").join("a.rb");

        assert!(write_atomic(&path, "x = 1\n").is_err());
    }
}
