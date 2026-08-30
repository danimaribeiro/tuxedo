#[derive(Debug, Clone)]
pub struct Diagnostic {
    pub line: usize,
    pub col: usize,
    pub message: String,
    pub rule: String,
}

pub fn check(_source: &str) -> Vec<Diagnostic> {
    vec![]
}

pub fn fix(_source: &str) -> Option<String> {
    None
}
