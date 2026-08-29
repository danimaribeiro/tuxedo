//! Golden/snapshot tests for the formatter.
//!
//! Each `.rb` file under `tests/fixtures/format/` is formatted, and the
//! output is checked against a committed `insta` snapshot. Run
//! `cargo insta review` to accept new/changed snapshots.
//!
//! Every fixture is also checked for idempotency: formatting already
//! formatted code should be a no-op.

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
    let fixtures_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/format");

    for path in fixture_paths() {
        let input = fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("failed to read {}: {e}", path.display()));

        let snapshot_name = path
            .strip_prefix(&fixtures_dir)
            .expect("fixture path should be under fixtures_dir")
            .with_extension("")
            .to_string_lossy()
            .replace(std::path::MAIN_SEPARATOR, "__");

        let output = tuxedo::format(&input);

        insta::assert_snapshot!(snapshot_name, output);

        let reformatted = tuxedo::format(&output);
        assert_eq!(
            output,
            reformatted,
            "formatting is not idempotent for fixture: {}",
            path.display()
        );
    }
}
