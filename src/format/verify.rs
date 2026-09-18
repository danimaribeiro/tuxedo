//! AST-equivalence check: the guarantee that formatting never changes what
//! a program means.
//!
//! `ruby -c` on the output only proves it is *some* valid program.
//! `defined?(Foo)` -> `Foo` and `undef foo` -> `foo` both still parse, and
//! both are different programs. So the formatter parses its own output and
//! compares the two trees, ignoring everything that is layout: where a
//! token sits, and which optional tokens (`then`, `do`, parentheses, the
//! `=`/`end` of an endless vs. regular `def`) spell the same node.
//!
//! The comparison works on a fingerprint string built from `ruby_prism`'s
//! `Debug` output. That output happens to be close to what is needed: it
//! prints child nodes structurally, constant-pool ids for every name, the
//! unescaped bytes of string content, and locations as the source text
//! they cover. Three things are adjusted on top of it:
//!
//! - An optional location (`Some("then")`) is rewritten to `None`, so a
//!   dropped or added optional token does not count as a difference. `&.`
//!   is kept: it is the only optional token that changes semantics.
//! - Numeric nodes print a pointer (`IntegerNode(0x5ccc...)`), which is
//!   neither stable nor meaningful. Pointers are blanked here and the
//!   literal's source text is appended by `NumericLiterals` instead.
//! - Required locations (`".."` vs `"..."`, `"/i"`) are kept as printed.
//!
//! Constant-pool ids are numbered in the order the parser first met each
//! name, and the fingerprint prints them, so a rewrite that reorders two
//! statements (a modifier `if` written as a block) renumbers names without
//! changing the program. `same_modulo_name_order` is the second chance for
//! output the strict comparison refused.

use ruby_prism::Visit;
use std::collections::BTreeSet;

/// A location-insensitive fingerprint of `source`'s AST, or `None` if
/// `source` does not parse.
pub(super) fn fingerprint(source: &str) -> Option<String> {
    let parsed = ruby_prism::parse(source.as_bytes());
    if parsed.errors().next().is_some() {
        return None;
    }
    let node = parsed.node();
    let mut out = normalize(&format!("{node:?}"));

    let mut numerics = NumericLiterals {
        source: source.as_bytes(),
        out: String::new(),
    };
    numerics.visit(&node);
    out.push('|');
    out.push_str(&numerics.out);

    // Comments are not in the tree, and losing or duplicating one is as
    // silent as dropping a node. Their text must survive (modulo the
    // `#foo` -> `# foo` normalization); *where* they end up is layout, and
    // is measured by scripts/comment-anchors.rb instead.
    let mut comments: Vec<String> = parsed
        .comments()
        .map(|c| {
            let text = String::from_utf8_lossy(c.text()).into_owned();
            super::Formatter::format_comment(text.trim_end()).into_owned()
        })
        .collect();
    comments.sort_unstable();
    for comment in comments {
        out.push('|');
        out.push_str(&comment);
    }

    // Everything after `__END__` is program data (`DATA.read`), not part
    // of the tree at all.
    if let Some(data) = parsed.data_loc() {
        out.push('|');
        out.push_str(&String::from_utf8_lossy(data.as_slice()));
    }
    Some(out)
}

/// Whether `source` and `output` are the same program when the only
/// difference is the order in which names first appear.
///
/// Prism numbers the constant pool in that order, and the fingerprint
/// prints the numbers, so moving `a(b)` past `c(d)` -- `a(b) if c(d)`
/// written as `if c(d)` / `a(b)` -- renumbers every name in it without
/// changing the program. Both texts are parsed again behind one prelude
/// that mentions every name in either of them, sorted, so the pool is
/// numbered the same way for both and only a real difference is left.
///
/// A name the prelude does not know keeps the order-sensitive numbering,
/// which can only make two equal programs differ, never the reverse: this
/// is a second chance for output the strict comparison refused.
pub(super) fn same_modulo_name_order(source: &str, output: &str) -> bool {
    let mut names = BTreeSet::new();
    collect_names(source, &mut names);
    collect_names(output, &mut names);
    let mut prelude = String::new();
    for name in &names {
        if name.starts_with(['@', '$']) {
            prelude.push_str(name);
        } else {
            prelude.push_str("nil.");
            prelude.push_str(name);
            if !name.ends_with(['?', '!']) {
                prelude.push_str(" = nil;nil.");
                prelude.push_str(name);
            }
        }
        prelude.push('\n');
    }
    // Operator methods are names too: `a + b` interns `+`.
    prelude.push_str(
        "nil + 1; nil - 1; nil * 1; nil / 1; nil % 1; nil ** 1; nil == 1; nil != 1; nil < 1;\n\
         nil > 1; nil <= 1; nil >= 1; nil <=> 1; nil === 1; nil =~ 1; nil !~ 1; nil << 1;\n\
         nil >> 1; nil & 1; nil | 1; nil ^ 1; -nil; +nil; !nil; ~nil; nil[1]; nil[1] = 2; nil.()\n",
    );
    let pooled = |text: &str| fingerprint(&format!("{prelude}{text}"));
    match (pooled(source), pooled(output)) {
        (Some(before), Some(after)) => before == after,
        _ => false,
    }
}

/// Every identifier-shaped run in `text`, with its `@`/`@@`/`$` sigil:
/// a superset of the names the parser interns (it also finds words in
/// strings and comments, which costs nothing).
fn collect_names(text: &str, out: &mut BTreeSet<String>) {
    let is_word = |c: char| c.is_alphanumeric() || c == '_';
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let start = i;
        while i < chars.len() && matches!(chars[i], '@' | '$') {
            i += 1;
        }
        if i < chars.len() && (chars[i].is_alphabetic() || chars[i] == '_') {
            while i < chars.len() && is_word(chars[i]) {
                i += 1;
            }
            if i < chars.len() && matches!(chars[i], '?' | '!') {
                i += 1;
            }
            out.insert(chars[start..i].iter().collect());
        } else if i == start {
            i += 1;
        }
    }
}

/// Returns a description of the first point where two fingerprints
/// diverge, for error messages. Both are single-line strings, so a window
/// around the first differing byte is the most useful thing to show.
pub(super) fn first_difference(before: &str, after: &str) -> String {
    let at = before
        .bytes()
        .zip(after.bytes())
        .position(|(a, b)| a != b)
        .unwrap_or_else(|| before.len().min(after.len()));
    let window = |s: &str| {
        let mut start = at.saturating_sub(60);
        while !s.is_char_boundary(start) {
            start -= 1;
        }
        let mut end = (at + 60).min(s.len());
        while !s.is_char_boundary(end) {
            end += 1;
        }
        s[start..end].to_string()
    };
    format!(
        "source AST: ...{}...\n  output AST: ...{}...",
        window(before),
        window(after)
    )
}

fn normalize(debug: &str) -> String {
    const OPTIONAL_LOCATION: &str = "Some(\"";
    let bytes = debug.as_bytes();
    let mut out = String::with_capacity(debug.len());
    let mut i = 0;
    while i < bytes.len() {
        if debug[i..].starts_with(OPTIONAL_LOCATION) {
            let text_start = i + OPTIONAL_LOCATION.len();
            let text_end = string_literal_end(bytes, text_start);
            // `Some("` + text + `")`
            let end = (text_end + 2).min(bytes.len());
            if &debug[text_start..text_end] == "&." {
                out.push_str(&debug[i..end]);
            } else {
                out.push_str("None");
            }
            i = end;
        } else if bytes[i] == b'"' {
            // A required location: copy it whole so its contents are never
            // mistaken for the patterns above.
            let text_end = string_literal_end(bytes, i + 1);
            let end = (text_end + 1).min(bytes.len());
            out.push_str(&debug[i..end]);
            i = end;
        } else if debug[i..].starts_with("0x") {
            i += 2;
            while i < bytes.len() && bytes[i].is_ascii_hexdigit() {
                i += 1;
            }
            out.push('_');
        } else {
            let ch = debug[i..].chars().next().unwrap_or('\0');
            out.push(ch);
            i += ch.len_utf8().max(1);
        }
    }
    out
}

/// Index of the closing `"` of a `Debug`-escaped string literal whose
/// contents start at `start`.
fn string_literal_end(bytes: &[u8], start: usize) -> usize {
    let mut i = start;
    while i < bytes.len() {
        match bytes[i] {
            b'\\' => i += 2,
            b'"' => return i,
            _ => i += 1,
        }
    }
    bytes.len()
}

struct NumericLiterals<'s> {
    source: &'s [u8],
    out: String,
}

impl NumericLiterals<'_> {
    fn push(&mut self, location: &ruby_prism::Location<'_>) {
        let text = &self.source[location.start_offset()..location.end_offset()];
        self.out.push_str(&String::from_utf8_lossy(text));
        self.out.push(',');
    }
}

impl<'pr> Visit<'pr> for NumericLiterals<'_> {
    fn visit_integer_node(&mut self, node: &ruby_prism::IntegerNode<'pr>) {
        self.push(&node.location());
    }

    fn visit_rational_node(&mut self, node: &ruby_prism::RationalNode<'pr>) {
        self.push(&node.location());
    }

    fn visit_imaginary_node(&mut self, node: &ruby_prism::ImaginaryNode<'pr>) {
        self.push(&node.location());
    }
}

#[cfg(test)]
mod tests {
    use super::fingerprint;

    fn same(a: &str, b: &str) -> bool {
        fingerprint(a).expect("a parses") == fingerprint(b).expect("b parses")
    }

    #[test]
    fn layout_is_ignored() {
        assert!(same("x = [1,\n     2]\n", "x = [1, 2]\n"));
        assert!(same("if a then b end\n", "if a\n  b\nend\n"));
        assert!(same("while x do y end\n", "while x\n  y\nend\n"));
        assert!(same("def a = 1\n", "def a\n  1\nend\n"));
        assert!(same("foo(1, # note\n    2)\n", "foo(1, 2)\n# note\n"));
        assert!(same("#note\nfoo\n", "# note\nfoo\n"));
        assert!(same("x = 'a'\n", "x = \"a\"\n"));
    }

    #[test]
    fn a_statement_moved_past_another_is_layout() {
        let pooled = super::same_modulo_name_order;
        assert!(pooled("a(b) if c(d)\n", "if c(d)\n  a(b)\nend\n"));
        assert!(pooled("a(b) unless c(d)\n", "unless c(d)\n  a(b)\nend\n"));
        assert!(pooled("@x = y if z?\n", "if z?\n  @x = y\nend\n"));
        assert!(pooled("f(x + 1) if g(x)\n", "if g(x)\n  f(x + 1)\nend\n"));
    }

    #[test]
    fn a_changed_name_is_not_layout() {
        let pooled = super::same_modulo_name_order;
        assert!(!pooled("a(b) if c(d)\n", "if c(d)\n  a(e)\nend\n"));
        assert!(!pooled("a(b) if c(d)\n", "if c(d)\n  b(a)\nend\n"));
        assert!(!pooled("a + b if c\n", "if c\n  a - b\nend\n"));
        assert!(!pooled("@x = y if z\n", "if z\n  @w = y\nend\n"));
        assert!(!pooled("a(b) if c(d)\n", "unless c(d)\n  a(b)\nend\n"));
    }

    #[test]
    fn meaning_is_not() {
        assert!(!same("defined?(Foo)\n", "Foo\n"));
        assert!(!same("undef foo\n", "foo\n"));
        assert!(!same("a&.b\n", "a.b\n"));
        assert!(!same("a&.+(1)\n", "a + 1\n"));
        assert!(!same("1..2\n", "1...2\n"));
        assert!(!same("x = 1\n", "x = 2\n"));
        assert!(!same("x = 0x10\n", "x = 10\n"));
        assert!(!same("x = 1r\n", "x = 2r\n"));
        assert!(!same("/a/i\n", "/a/\n"));
        assert!(!same("x = 'a'\n", "x = 'b'\n"));
        assert!(!same("v = + 1.second\n", "v = +1.second\n"));
        assert!(!same("foo do |a; b|\nend\n", "foo do |a|\nend\n"));
        assert!(!same("puts 1\n__END__\ndata\n", "puts 1\n"));
        assert!(!same("foo # note\n", "foo\n"));
        assert!(!same("foo # note\n", "foo # note\n# note\n"));
        assert!(!same(
            "x = <<~A\n  #{a}\n    b\nA\n",
            "x = <<~A\n#{a}\n    b\nA\n"
        ));
    }

    #[test]
    fn unparseable_source_has_no_fingerprint() {
        assert_eq!(fingerprint("def foo(\n  x = "), None);
    }
}
