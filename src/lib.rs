//! Tuxedo, a Ruby formatter.
//!
//! The `tuxedo` binary (`src/main.rs`) is a thin command line over this
//! crate. [`format_with_config`] turns one source string into formatted
//! source, or into a [`FormatError`] when it cannot do so faithfully.
//! [`walk_and_process`] finds the Ruby files under the paths a run
//! names and hands each to a callback, on several threads.
//! [`write_atomic`] replaces a file so that no reader ever sees it half
//! written. [`Config`] is `.tuxedo.toml`.
//!
//! How formatting works, and the invariants every formatting method relies
//! on, are in the [`format`](mod@format) module.

pub mod config;
pub mod format;
pub mod walk;
pub mod write;

pub use config::Config;
pub use format::{FormatError, format, format_unverified, format_with_config, verify_same_program};
pub use walk::{Exclude, WalkResult, walk_and_process};
pub use write::write_atomic;
