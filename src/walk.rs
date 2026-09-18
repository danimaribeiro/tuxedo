use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};

use ignore::WalkBuilder;

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
    use super::is_ruby_file;
    use std::path::Path;

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
