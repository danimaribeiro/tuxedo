//! `.tuxedo.toml`: found by walking up from the current directory unless
//! `--config` names a path, and [`Config::default`] when there is none.
//!
//! Every key here is one the formatter or linter reads, and
//! `deny_unknown_fields` makes anything else a hard error rather than a
//! silently ignored setting: a knob nothing consults is a promise to the
//! user that nothing keeps.

use std::path::{Path, PathBuf};

use serde::Deserialize;

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    #[serde(default)]
    pub format: FormatConfig,

    #[serde(default)]
    pub lint: LintConfig,

    /// Gitignore-style globs, relative to [`Config::root`], that a walk
    /// skips on top of `.gitignore`. Top level rather than under `[format]`
    /// because `check` honours it as well.
    #[serde(default)]
    pub exclude: Vec<String>,

    /// The directory the config file sits in, which `exclude` is relative
    /// to. Empty when there is no file, and then `exclude` is empty too.
    #[serde(skip)]
    pub root: PathBuf,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct FormatConfig {
    pub line_length: usize,
    pub indent_size: usize,
    /// Whether a hash literal has a space inside its braces.
    pub hash_brace_space: HashBraceSpace,
    /// Where `private`, `protected` and `public` sit in a class body.
    pub access_modifier_style: AccessModifierStyle,
    /// Which quote a plain string or quoted symbol is written with.
    pub quote_style: QuoteStyle,
}

impl Default for FormatConfig {
    fn default() -> Self {
        Self {
            line_length: 120,
            indent_size: 2,
            hash_brace_space: HashBraceSpace::default(),
            access_modifier_style: AccessModifierStyle::default(),
            quote_style: QuoteStyle::default(),
        }
    }
}

/// `{ key: value }` (`space`) or `{key: value}` (`no_space`). Block braces
/// are a separate construct and keep their spaces either way.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HashBraceSpace {
    #[default]
    Space,
    NoSpace,
}

/// `indent` puts an access modifier at the body's indent, `outdent` one
/// level out, at the indent of the `class` keyword.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AccessModifierStyle {
    #[default]
    Indent,
    Outdent,
}

/// The quote a literal is rewritten to, when that loses nothing: `double`
/// or `single`, or `preserve` to leave every literal as written. A literal
/// the target quote cannot hold without changing its value is left alone.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QuoteStyle {
    #[default]
    Double,
    Single,
    Preserve,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LintConfig {
    #[serde(default)]
    pub enabled: Vec<String>,

    #[serde(default)]
    pub disabled: Vec<String>,
}

impl Config {
    pub fn load(explicit_path: Option<&Path>) -> Config {
        if let Some(path) = explicit_path {
            let contents = std::fs::read_to_string(path).unwrap_or_else(|e| {
                eprintln!("error: could not read config {}: {e}", path.display());
                std::process::exit(1);
            });
            return parse_config(&contents, path);
        }

        if let Some((path, contents)) = discover() {
            return parse_config(&contents, &path);
        }

        Config::default()
    }
}

fn discover() -> Option<(PathBuf, String)> {
    let mut dir = std::env::current_dir().ok()?;
    loop {
        let candidate = dir.join(".tuxedo.toml");
        if let Ok(contents) = std::fs::read_to_string(&candidate) {
            return Some((candidate, contents));
        }
        if !dir.pop() {
            return None;
        }
    }
}

fn parse_config(contents: &str, path: &Path) -> Config {
    let mut config: Config = toml::from_str(contents).unwrap_or_else(|e| {
        eprintln!("error: invalid config {}: {e}", path.display());
        std::process::exit(1);
    });
    let root = path.parent().filter(|p| !p.as_os_str().is_empty());
    config.root = std::path::absolute(root.unwrap_or(Path::new("."))).unwrap_or_default();
    config
}
