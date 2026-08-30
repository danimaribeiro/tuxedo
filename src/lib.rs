pub mod config;
pub mod format;
pub mod lint;
pub mod walk;

pub use config::Config;
pub use format::format;
pub use lint::{Diagnostic, check, fix};
pub use walk::{WalkResult, walk_and_process};
