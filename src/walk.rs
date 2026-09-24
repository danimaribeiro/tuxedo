//! Which files a run formats, and running the formatter over them.
//!
//! [`walk_and_process`] walks the named paths with the `ignore` crate (the
//! walker ripgrep uses) on its thread pool: hidden files are skipped,
//! `.gitignore` is honoured unless `--no-ignore` turns it off, and the
//! config's `exclude` globs ([`Exclude`]) are applied on top. Only files
//! that look like Ruby are handed on, by extension (`.rb`, `.rake`,
//! `.gemspec`, `.ru`) or by name (`Rakefile`, `Gemfile`). A path named on
//! the command line is never excluded, but it still has to look like Ruby.
//!
//! What the walk could not hand on -- a directory it could not descend, a
//! file it could not read or that is not UTF-8 -- is counted in
//! [`WalkResult::errors`] and fails the run, the same as a file the
//! formatter refused: a run that skipped part of the tree must not look
//! clean.

use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};

use ignore::WalkBuilder;
use ignore::gitignore::{Gitignore, GitignoreBuilder};

/// The `exclude` globs of a config, matched the way `.gitignore` lines are
/// against a path relative to the config file's directory.
///
/// Applied as a filter on the walk rather than through the `ignore`
/// crate's override builder: an override matches a path only after
/// stripping a root that has to be spelled the way the walk spells its
/// paths, and a walk started from `.` or `lib` spells them relative while
/// the config's directory is absolute. Here the path is made absolute and
/// cut back to the root first, so the match does not depend on how the
/// command line named the directory.
pub struct Exclude {
    root: std::path::PathBuf,
    globs: Gitignore,
}

impl Exclude {
    /// An error names the first glob that is not valid. A leading `!` is
    /// refused: it would un-exclude, and `exclude` only ever excludes.
    pub fn new(root: &Path, patterns: &[String]) -> Result<Self, String> {
        let mut builder = GitignoreBuilder::new(".");
        for pattern in patterns {
            if pattern.starts_with('!') {
                return Err(format!(
                    "exclude pattern `{pattern}`: a leading `!` is not supported"
                ));
            }
            builder
                .add_line(None, pattern)
                .map_err(|e| format!("exclude pattern `{pattern}`: {e}"))?;
        }
        let globs = builder.build().map_err(|e| format!("exclude: {e}"))?;
        Ok(Self {
            root: root.to_path_buf(),
            globs,
        })
    }

    fn excludes(&self, path: &Path, is_dir: bool) -> bool {
        if self.globs.is_empty() {
            return false;
        }
        let Ok(absolute) = std::path::absolute(path) else {
            return false;
        };
        lexically_clean(&absolute)
            .strip_prefix(&self.root)
            .is_ok_and(|relative| self.globs.matched(relative, is_dir).is_ignore())
    }
}

pub struct WalkResult {
    /// Files handed to `process`. A file that could not be read never
    /// reaches it and so is not counted here: it was not checked, and a
    /// summary saying it was would hide the failure below.
    pub total: usize,
    pub changed: usize,
    /// What the walk could not hand to `process` at all: a directory it
    /// could not descend, a file it could not read, or bytes that are not
    /// valid UTF-8. Kept apart from the failures `process` reports for
    /// itself, and fatal to the exit status just the same -- a run that
    /// skipped half a tree must not look like a clean one.
    pub errors: usize,
}

pub fn walk_and_process(
    paths: &[impl AsRef<Path>],
    jobs: Option<usize>,
    no_ignore: bool,
    exclude: Exclude,
    process: impl FnMut(&Path, &str) -> Option<String> + Send + Clone,
) -> WalkResult {
    let changed = AtomicUsize::new(0);
    let total = AtomicUsize::new(0);
    let errors = AtomicUsize::new(0);

    let first = paths[0].as_ref();
    let mut builder = WalkBuilder::new(first);
    for path in &paths[1..] {
        builder.add(path);
    }
    builder.hidden(true);
    if no_ignore {
        builder.git_ignore(false);
    }
    if let Some(n) = jobs {
        builder.threads(n);
    }
    // A path named on the command line (depth 0) is never excluded: asking
    // for a file by name is the override, as it is for `.gitignore`.
    builder.filter_entry(move |entry| {
        entry.depth() == 0
            || !exclude.excludes(entry.path(), entry.file_type().is_some_and(|t| t.is_dir()))
    });

    let changed_ref = &changed;
    let total_ref = &total;
    let errors_ref = &errors;

    builder.build_parallel().run(|| {
        let mut process = process.clone();
        Box::new(move |entry| {
            let entry = match entry {
                Ok(entry) => entry,
                Err(e) => {
                    eprintln!("error: {e}");
                    errors_ref.fetch_add(1, Ordering::Relaxed);
                    return ignore::WalkState::Continue;
                }
            };

            let path = entry.path();
            if !path.is_file() || !is_ruby_file(path) {
                return ignore::WalkState::Continue;
            }

            let source = match std::fs::read_to_string(path) {
                Ok(source) => source,
                Err(e) => {
                    // Covers "stream did not contain valid UTF-8". The
                    // formatter works on `&str` and has no way to carry
                    // arbitrary bytes through, so such a file is refused
                    // out loud rather than passed through lossily.
                    eprintln!("error: could not read {}: {e}", path.display());
                    errors_ref.fetch_add(1, Ordering::Relaxed);
                    return ignore::WalkState::Continue;
                }
            };

            total_ref.fetch_add(1, Ordering::Relaxed);

            if let Some(_output) = process(path, &source) {
                changed_ref.fetch_add(1, Ordering::Relaxed);
            }

            ignore::WalkState::Continue
        })
    });

    WalkResult {
        total: total.load(Ordering::Relaxed),
        changed: changed.load(Ordering::Relaxed),
        errors: errors.load(Ordering::Relaxed),
    }
}

/// `path` with its `.` and `..` components folded away, without touching the
/// filesystem. `absolute` leaves them in, and `app/../db/x.rb` would
/// otherwise not match an anchored `/db/x.rb`.
fn lexically_clean(path: &Path) -> std::path::PathBuf {
    use std::path::Component;
    let mut clean = std::path::PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                clean.pop();
            }
            other => clean.push(other),
        }
    }
    clean
}

/// Extensions that mean Ruby.
const RUBY_EXTENSIONS: [&str; 4] = ["rb", "rake", "gemspec", "ru"];

/// Files that are Ruby but carry no extension to say so. Ruby's own
/// tooling recognises these by name, so matching on the extension alone
/// skips most of a project's build and dependency code -- the files most
/// likely to be edited by hand and least likely to be covered by tests.
const RUBY_FILENAMES: [&str; 2] = ["Rakefile", "Gemfile"];

fn is_ruby_file(path: &Path) -> bool {
    if let Some(ext) = path.extension().and_then(|ext| ext.to_str())
        && RUBY_EXTENSIONS.contains(&ext)
    {
        return true;
    }
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| RUBY_FILENAMES.contains(&name))
}

#[cfg(test)]
mod tests {
    use super::{is_ruby_file, lexically_clean};
    use std::path::Path;

    #[test]
    fn dot_components_fold_away() {
        assert_eq!(
            lexically_clean(Path::new("/a/b/../c/./d.rb")),
            Path::new("/a/c/d.rb")
        );
    }

    #[test]
    fn recognises_ruby_by_extension_and_by_name() {
        for path in [
            "lib/tuxedo.rb",
            "lib/tasks/db.rake",
            "tuxedo.gemspec",
            "config.ru",
            "Rakefile",
            "Gemfile",
            "sub/dir/Rakefile",
        ] {
            assert!(is_ruby_file(Path::new(path)), "{path} should be Ruby");
        }

        for path in [
            "README.md",
            "Gemfile.lock",
            "rakefile.txt",
            "Makefile",
            "src/main.rs",
            "norbextension",
        ] {
            assert!(!is_ruby_file(Path::new(path)), "{path} should not be Ruby");
        }
    }
}
