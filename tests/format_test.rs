//! Formatter tests.
//!
//! Each `.rb` file under `tests/fixtures/format/` must already be
//! formatted. Two checks per fixture:
//!   1. Stability:   format(input) == input
//!   2. Idempotency: format(format(input)) == format(input)

use std::fs;
use std::path::{Path, PathBuf};

fn fixture_paths() -> Vec<PathBuf> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/format");
    let mut paths = Vec::new();
    collect_rb_files(&dir, &mut paths);
    paths.sort();
    paths
}

fn collect_rb_files(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(dir).expect("failed to read fixtures/format directory") {
        let entry = entry.expect("failed to read directory entry");
        let path = entry.path();
        if path.is_dir() {
            collect_rb_files(&path, out);
        } else if path.extension().is_some_and(|ext| ext == "rb") {
            out.push(path);
        }
    }
}

#[test]
fn format_fixtures() {
    for path in fixture_paths() {
        let input = fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("failed to read {}: {e}", path.display()));

        let output = tuxedo::format(&input);

        assert_eq!(
            input,
            output,
            "fixture is not already formatted: {}",
            path.display()
        );

        let reformatted = tuxedo::format(&output);
        assert_eq!(
            output,
            reformatted,
            "formatting is not idempotent for fixture: {}",
            path.display()
        );
    }
}
