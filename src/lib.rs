pub mod config;
pub mod format;
pub mod lint;
pub mod walk;
pub mod write;

pub use config::Config;
pub use format::{FormatError, format, format_unverified, format_with_config};
pub use lint::{Diagnostic, check, fix};
pub use walk::{Exclude, WalkResult, walk_and_process};
pub use write::write_atomic;
