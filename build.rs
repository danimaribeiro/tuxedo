//! Writes one `#[test]` per fixture file for `tests/format_test.rs` and
//! `tests/format_snapshot_test.rs`, so a change that breaks eight fixtures
//! reports eight, and `cargo test <fixture name>` runs one.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

fn main() {
    println!("cargo:rerun-if-changed=tests/fixtures");
    let out = PathBuf::from(std::env::var_os("OUT_DIR").expect("OUT_DIR is set by cargo"));
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    for (dir, check, file) in [
        ("format", "check_fixture", "format_fixtures.rs"),
        ("format_messy", "check_messy_fixture", "messy_fixtures.rs"),
    ] {
        let base = root.join(dir);
        let mut paths = Vec::new();
        collect(&base, &mut paths);
        paths.sort();

        let mut generated = String::new();
        for path in paths {
            let relative = path.strip_prefix(&base).unwrap().with_extension("");
            let name: String = relative
                .to_string_lossy()
                .chars()
                .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
                .collect();
            let relative = relative.to_string_lossy().replace('\\', "/");
            writeln!(
                generated,
                "#[test]\nfn {name}() {{\n    {check}(\"{relative}\");\n}}\n"
            )
            .unwrap();
        }
        std::fs::write(out.join(file), generated).unwrap();
    }
}

fn collect(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries {
        let path = entry.unwrap().path();
        if path.is_dir() {
            collect(&path, out);
        } else if path.extension().is_some_and(|ext| ext == "rb") {
            out.push(path);
        }
    }
}
