use std::path::{Path, PathBuf};

use serde::Deserialize;

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    #[serde(default)]
    pub format: FormatConfig,

    #[serde(default)]
    pub lint: LintConfig,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FormatConfig {
    pub line_length: usize,
    pub indent_size: usize,
    pub quote_style: QuoteStyle,
}

impl Default for FormatConfig {
    fn default() -> Self {
        Self {
            line_length: 120,
            indent_size: 2,
            quote_style: QuoteStyle::Single,
        }
    }
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LintConfig {
    #[serde(default)]
    pub enabled: Vec<String>,

    #[serde(default)]
    pub disabled: Vec<String>,
}

#[derive(Debug, Default, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum QuoteStyle {
    #[default]
    Single,
    Double,
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
    toml::from_str(contents).unwrap_or_else(|e| {
        eprintln!("error: invalid config {}: {e}", path.display());
        std::process::exit(1);
    })
}
