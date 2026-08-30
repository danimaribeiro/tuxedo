use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};

use ignore::WalkBuilder;

pub struct WalkResult {
    pub total: usize,
    pub changed: usize,
}

pub fn walk_and_process(
    paths: &[impl AsRef<Path>],
    jobs: Option<usize>,
    no_ignore: bool,
    process: impl FnMut(&Path, &str) -> Option<String> + Send + Clone,
) -> WalkResult {
    let changed = AtomicUsize::new(0);
    let total = AtomicUsize::new(0);

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

    builder.build_parallel().run(|| {
        let mut process = process.clone();
        Box::new(move |entry| {
            let Ok(entry) = entry else {
                return ignore::WalkState::Continue;
            };

            let path = entry.path();
            if !path.is_file() || !path.extension().is_some_and(|ext| ext == "rb") {
                return ignore::WalkState::Continue;
            }

            total_ref.fetch_add(1, Ordering::Relaxed);

            let Ok(source) = std::fs::read_to_string(path) else {
                eprintln!("error: could not read {}", path.display());
                return ignore::WalkState::Continue;
            };

            if let Some(_output) = process(path, &source) {
                changed_ref.fetch_add(1, Ordering::Relaxed);
            }

            ignore::WalkState::Continue
        })
    });

    WalkResult {
        total: total.load(Ordering::Relaxed),
        changed: changed.load(Ordering::Relaxed),
    }
}
