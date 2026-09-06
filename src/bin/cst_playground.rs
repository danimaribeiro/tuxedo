//! Playground: build a lossless (green/red) CST on top of ruby-prism's AST.
//!
//! Prism's AST is lossy - it exposes precise byte locations for every node and
//! for many of the keyword/operator/delimiter tokens within a node, but it does
//! not model whitespace, newlines, or comments as tree nodes at all. This tool
//! reconstructs that missing trivia from the *gaps* between known locations and
//! emits the result as JSON for `playground/index.html` to visualize.
//!
//! Node coverage here is intentionally narrow (Program/Statements/If/Unless/
//! Else/Call/Arguments/String) - just enough to cover `playground/conditionals.rb`.
//! Anything else falls back to an opaque leaf.

use ruby_prism::{ArgumentsNode, ElseNode, Location, Node, StatementsNode};
use std::fmt::Write as _;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Category {
    /// A green-tree interior node mirroring an AST construct.
    Node,
    /// A concrete lexical token (keyword, identifier, punctuation, string content).
    Token,
    /// Trivia reconstructed from a gap between known locations: whitespace or a comment.
    Trivia,
}

impl Category {
    fn as_str(self) -> &'static str {
        match self {
            Category::Node => "node",
            Category::Token => "token",
            Category::Trivia => "trivia",
        }
    }
}

struct Elem {
    kind: String,
    category: Category,
    start: usize,
    end: usize,
    children: Vec<Elem>,
}

fn range(loc: &Location<'_>) -> (usize, usize) {
    (loc.start_offset(), loc.end_offset())
}

fn token(kind: &str, (start, end): (usize, usize)) -> Elem {
    Elem { kind: kind.to_string(), category: Category::Token, start, end, children: Vec::new() }
}

/// Sorts `parts` by position and fills every gap between them (and between the
/// enclosing `span` and the first/last part) with reconstructed trivia. This is
/// the whole trick: prism guarantees the parts it hands us don't overlap, so
/// anything left over in `span` must be whitespace or a comment.
fn assemble(kind: &str, span: (usize, usize), mut parts: Vec<Elem>, comments: &[(usize, usize)]) -> Elem {
    parts.sort_by_key(|e| e.start);

    let mut children = Vec::with_capacity(parts.len() * 2);
    let mut cursor = span.0;
    for part in parts {
        if part.start > cursor {
            push_trivia(&mut children, cursor, part.start, comments);
        }
        cursor = cursor.max(part.end);
        children.push(part);
    }
    if span.1 > cursor {
        push_trivia(&mut children, cursor, span.1, comments);
    }

    Elem { kind: kind.to_string(), category: Category::Node, start: span.0, end: span.1, children }
}

fn push_trivia(children: &mut Vec<Elem>, start: usize, end: usize, comments: &[(usize, usize)]) {
    let mut cursor = start;
    for &(cs, ce) in comments {
        if cs < cursor || ce > end {
            continue;
        }
        if cs > cursor {
            children.push(Elem { kind: "whitespace".into(), category: Category::Trivia, start: cursor, end: cs, children: Vec::new() });
        }
        children.push(Elem { kind: "comment".into(), category: Category::Trivia, start: cs, end: ce, children: Vec::new() });
        cursor = ce;
    }
    if end > cursor {
        children.push(Elem { kind: "whitespace".into(), category: Category::Trivia, start: cursor, end, children: Vec::new() });
    }
}

fn build_statements(node: &StatementsNode<'_>, comments: &[(usize, usize)]) -> Elem {
    let parts = node.body().iter().map(|child| build(&child, comments)).collect();
    assemble("Statements", range(&node.location()), parts, comments)
}

fn build_arguments(node: &ArgumentsNode<'_>, comments: &[(usize, usize)]) -> Elem {
    let parts = node.arguments().iter().map(|child| build(&child, comments)).collect();
    assemble("Arguments", range(&node.location()), parts, comments)
}

fn build_else(node: &ElseNode<'_>, comments: &[(usize, usize)]) -> Elem {
    let mut parts = vec![token("else", range(&node.else_keyword_loc()))];
    if let Some(statements) = node.statements() {
        parts.push(build_statements(&statements, comments));
    }
    if let Some(end_loc) = node.end_keyword_loc() {
        parts.push(token("end", range(&end_loc)));
    }
    assemble("Else", range(&node.location()), parts, comments)
}

fn build(node: &Node<'_>, comments: &[(usize, usize)]) -> Elem {
    if let Some(n) = node.as_program_node() {
        let statements = build_statements(&n.statements(), comments);
        return assemble("Program", range(&n.location()), vec![statements], comments);
    }

    if let Some(n) = node.as_if_node() {
        let mut parts = Vec::new();
        if let Some(loc) = n.if_keyword_loc() {
            parts.push(token("if", range(&loc)));
        }
        parts.push(build(&n.predicate(), comments));
        if let Some(loc) = n.then_keyword_loc() {
            parts.push(token("then", range(&loc)));
        }
        if let Some(statements) = n.statements() {
            parts.push(build_statements(&statements, comments));
        }
        let has_subsequent = n.subsequent().is_some();
        if let Some(subsequent) = n.subsequent() {
            parts.push(build(&subsequent, comments));
        }
        // prism mirrors the chain's single terminal `end` onto every level's
        // end_keyword_loc; only take it here when this is that terminal level,
        // or it would be emitted once per elsif/if in the chain.
        if !has_subsequent {
            if let Some(loc) = n.end_keyword_loc() {
                parts.push(token("end", range(&loc)));
            }
        }
        return assemble("If", range(&n.location()), parts, comments);
    }

    if let Some(n) = node.as_else_node() {
        return build_else(&n, comments);
    }

    if let Some(n) = node.as_unless_node() {
        let mut parts = vec![token("unless", range(&n.keyword_loc()))];
        parts.push(build(&n.predicate(), comments));
        if let Some(loc) = n.then_keyword_loc() {
            parts.push(token("then", range(&loc)));
        }
        if let Some(statements) = n.statements() {
            parts.push(build_statements(&statements, comments));
        }
        let has_else = n.else_clause().is_some();
        if let Some(else_clause) = n.else_clause() {
            parts.push(build_else(&else_clause, comments));
        }
        // Same mirroring as IfNode above: defer to the else clause's own
        // end_keyword_loc when present, since it's the same physical token.
        if !has_else {
            if let Some(loc) = n.end_keyword_loc() {
                parts.push(token("end", range(&loc)));
            }
        }
        return assemble("Unless", range(&n.location()), parts, comments);
    }

    if let Some(n) = node.as_call_node() {
        let mut parts = Vec::new();
        if let Some(receiver) = n.receiver() {
            parts.push(build(&receiver, comments));
        }
        if let Some(loc) = n.call_operator_loc() {
            parts.push(token("call_operator", range(&loc)));
        }
        if let Some(loc) = n.message_loc() {
            parts.push(token("message", range(&loc)));
        }
        if let Some(loc) = n.opening_loc() {
            parts.push(token("opening_paren", range(&loc)));
        }
        if let Some(arguments) = n.arguments() {
            parts.push(build_arguments(&arguments, comments));
        }
        if let Some(loc) = n.closing_loc() {
            parts.push(token("closing_paren", range(&loc)));
        }
        return assemble("Call", range(&n.location()), parts, comments);
    }

    if let Some(n) = node.as_string_node() {
        let mut parts = vec![token("content", range(&n.content_loc()))];
        if let Some(loc) = n.opening_loc() {
            parts.push(token("opening_quote", range(&loc)));
        }
        if let Some(loc) = n.closing_loc() {
            parts.push(token("closing_quote", range(&loc)));
        }
        return assemble("String", range(&n.location()), parts, comments);
    }

    // Adjacent string literals with no operator between them (typically joined
    // by a `\`-escaped newline) get merged by prism into one node whose parts
    // are the individual StringNodes - this is the node RuboCop's semantic
    // alignment cops anchor against for multi-line concatenation.
    if let Some(n) = node.as_interpolated_string_node() {
        let mut parts: Vec<Elem> = n.parts().iter().map(|p| build(&p, comments)).collect();
        if let Some(loc) = n.opening_loc() {
            parts.push(token("opening_quote", range(&loc)));
        }
        if let Some(loc) = n.closing_loc() {
            parts.push(token("closing_quote", range(&loc)));
        }
        return assemble("InterpolatedString", range(&n.location()), parts, comments);
    }

    // Opaque leaf fallback for anything not handled above - not exercised by
    // playground/conditionals.rb today, but keeps the builder total.
    let (start, end) = range(&node.location());
    Elem { kind: "Unhandled".into(), category: Category::Token, start, end, children: Vec::new() }
}

fn node_children(elem: &Elem) -> Vec<&Elem> {
    elem.children.iter().filter(|c| c.category == Category::Node).collect()
}

fn tok<'a>(elem: &'a Elem, kind: &str, source: &'a str) -> Option<&'a str> {
    elem.children.iter().find(|c| c.category == Category::Token && c.kind == kind).map(|t| &source[t.start..t.end])
}

fn line_col(source: &str, offset: usize) -> (usize, usize) {
    let before = &source[..offset];
    match before.rfind('\n') {
        Some(nl) => (before.matches('\n').count() + 1, offset - nl - 1),
        None => (1, offset),
    }
}

// --- Formatter: re-derive canonical trivia from the green tree's structure,
// discarding whatever whitespace the source actually had. Node coverage
// mirrors `build()`; the only structural policy kept from the original
// trivia is "collapse 2+ blank lines to at most one", read directly off the
// discarded whitespace text rather than hardcoded.
const INDENT: &str = "  ";

fn indent(depth: usize) -> String {
    INDENT.repeat(depth)
}

fn render(elem: &Elem, source: &str, depth: usize) -> String {
    match elem.kind.as_str() {
        "Program" => render_statements_block(node_children(elem)[0], source, depth),
        "If" | "Unless" => render_conditional(elem, source, depth),
        "Else" => render_else(elem, source, depth),
        "Call" => render_call(elem, source, depth),
        "Arguments" => node_children(elem).iter().map(|a| render(a, source, depth)).collect::<Vec<_>>().join(", "),
        // Leaves and anything not given a canonical rule render verbatim -
        // exactly right for String/InterpolatedString, since only the
        // surrounding trivia (not literal content) is a formatting concern.
        _ => source[elem.start..elem.end].to_string(),
    }
}

fn render_statements_block(stmts: &Elem, source: &str, depth: usize) -> String {
    let mut out = String::new();
    let mut first = true;
    let mut pending_blank = false;
    for child in &stmts.children {
        match child.category {
            Category::Trivia => {
                if source[child.start..child.end].matches('\n').count() >= 2 {
                    pending_blank = true;
                }
            }
            Category::Node => {
                if !first {
                    out.push('\n');
                    if pending_blank {
                        out.push('\n');
                    }
                }
                out.push_str(&indent(depth));
                out.push_str(&render(child, source, depth));
                first = false;
                pending_blank = false;
            }
            Category::Token => {}
        }
    }
    out
}

fn render_conditional(elem: &Elem, source: &str, depth: usize) -> String {
    let keyword = tok(elem, if elem.kind == "If" { "if" } else { "unless" }, source).expect("if/unless keyword");
    let nodes = node_children(elem);
    let predicate = nodes[0];
    let rest = &nodes[1..];

    let mut out = format!("{keyword} {}", render(predicate, source, depth));

    let mut rest = rest;
    if let Some(stmts) = rest.first().filter(|n| n.kind == "Statements") {
        out.push('\n');
        out.push_str(&render_statements_block(stmts, source, depth + 1));
        rest = &rest[1..];
    }

    if let Some(sub) = rest.first() {
        out.push('\n');
        out.push_str(&indent(depth));
        out.push_str(&render(sub, source, depth));
        return out;
    }

    out.push('\n');
    out.push_str(&indent(depth));
    out.push_str("end");
    out
}

fn render_else(elem: &Elem, source: &str, depth: usize) -> String {
    let mut out = String::from("else");
    if let Some(stmts) = node_children(elem).first() {
        out.push('\n');
        out.push_str(&render_statements_block(stmts, source, depth + 1));
    }
    out.push('\n');
    out.push_str(&indent(depth));
    out.push_str("end");
    out
}

fn render_call(elem: &Elem, source: &str, depth: usize) -> String {
    let nodes = node_children(elem);
    let message = tok(elem, "message", source).unwrap_or("");
    if let Some(op) = tok(elem, "call_operator", source) {
        // receiver.method(...) form - not exercised by the formatter fixture.
        let receiver = render(nodes[0], source, depth);
        return format!("{receiver}{op}{message}");
    }
    match nodes.len() {
        0 => message.to_string(),
        1 => format!("{message} {}", render(nodes[0], source, depth)),
        _ => format!("{} {message} {}", render(nodes[0], source, depth), render(nodes[1], source, depth)),
    }
}

// --- Linter: a `Style/IfUnlessModifier`-shaped check. The green tree gives a
// position-independent pattern match (single-statement body, no elsif/else);
// only the reported location needs the red tree's absolute offsets.
struct Diagnostic {
    start: usize,
    end: usize,
    message: String,
    replacement: String,
}

fn check_if_unless_modifier(elem: &Elem, source: &str, out: &mut Vec<Diagnostic>) {
    if elem.kind == "If" || elem.kind == "Unless" {
        let nodes = node_children(elem);
        if nodes.len() == 2 && nodes[1].kind == "Statements" && node_children(nodes[1]).len() == 1 {
            let keyword = tok(elem, if elem.kind == "If" { "if" } else { "unless" }, source).expect("keyword");
            let predicate_text = &source[nodes[0].start..nodes[0].end];
            let body = node_children(nodes[1])[0];
            let body_text = &source[body.start..body.end];
            let replacement = format!("{body_text} {keyword} {predicate_text}");
            if replacement.len() <= 80 {
                out.push(Diagnostic {
                    start: elem.start,
                    end: elem.end,
                    message: format!("Style/IfUnlessModifier: Favor a modifier `{keyword}` unless the body doesn't fit on one line."),
                    replacement,
                });
            }
        }
    }
    for child in &elem.children {
        check_if_unless_modifier(child, source, out);
    }
}

// --- RuboCop-style semantic alignment: a multi-line `InterpolatedString`'s
// later parts are checked against the *column* of the first part - a purely
// red-tree concern, since "column" only exists once you've walked from the
// root accumulating widths into an absolute position.
struct AlignmentDiagnostic {
    start: usize,
    end: usize,
    expected_column: usize,
    actual_column: usize,
}

fn check_alignment(elem: &Elem, source: &str, out: &mut Vec<AlignmentDiagnostic>) {
    if elem.kind == "InterpolatedString" {
        let parts = node_children(elem);
        if let Some((anchor, rest)) = parts.split_first() {
            let (_, anchor_col) = line_col(source, anchor.start);
            for part in rest {
                let (_, col) = line_col(source, part.start);
                if col != anchor_col {
                    out.push(AlignmentDiagnostic { start: part.start, end: part.end, expected_column: anchor_col, actual_column: col });
                }
            }
        }
    }
    for child in &elem.children {
        check_alignment(child, source, out);
    }
}

fn corrected_alignment(source: &str, diagnostics: &[AlignmentDiagnostic]) -> String {
    let mut edits: Vec<(usize, usize, String)> = diagnostics
        .iter()
        .map(|d| {
            let last_nl = source[..d.start].rfind('\n').expect("continuation line has a preceding newline");
            (last_nl + 1, d.start, " ".repeat(d.expected_column))
        })
        .collect();
    edits.sort_by(|a, b| b.0.cmp(&a.0));

    let mut result = source.to_string();
    for (start, end, replacement) in edits {
        result.replace_range(start..end, &replacement);
    }
    result
}

fn escape_json(s: &str, out: &mut String) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => {
                let _ = write!(out, "\\u{:04x}", c as u32);
            }
            c => out.push(c),
        }
    }
    out.push('"');
}

fn write_elem(elem: &Elem, source: &str, out: &mut String) {
    out.push('{');
    out.push_str("\"kind\":");
    escape_json(&elem.kind, out);
    let _ = write!(out, ",\"category\":\"{}\",\"start\":{},\"end\":{},\"text\":", elem.category.as_str(), elem.start, elem.end);
    escape_json(&source[elem.start..elem.end], out);
    out.push_str(",\"children\":[");
    for (i, child) in elem.children.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        write_elem(child, source, out);
    }
    out.push_str("]}");
}

fn parse_file(path: &str) -> (String, Elem) {
    let source = std::fs::read_to_string(path).unwrap_or_else(|e| panic!("failed to read {path}: {e}"));
    let mut root = {
        let parsed = ruby_prism::parse(source.as_bytes());
        let comments: Vec<(usize, usize)> = parsed.comments().map(|c| range(&c.location())).collect();
        build(&parsed.node(), &comments)
    };
    // ProgramNode's own location can end before the source does (e.g. it
    // doesn't claim a trailing newline after the last statement) - extend the
    // root to the full source so that trivia isn't silently dropped at EOF.
    if root.end < source.len() {
        push_trivia(&mut root.children, root.end, source.len(), &[]);
        root.end = source.len();
    }
    (source, root)
}

fn write_scenario(out: &mut String, source: &str, tree: &Elem, extra: &[(&str, String)]) {
    out.push('{');
    out.push_str("\"source\":");
    escape_json(source, out);
    out.push_str(",\"tree\":");
    write_elem(tree, source, out);
    for (key, value) in extra {
        out.push_str(",\"");
        out.push_str(key);
        out.push_str("\":");
        out.push_str(value);
    }
    out.push('}');
}

fn json_string_field(s: &str) -> String {
    let mut out = String::new();
    escape_json(s, &mut out);
    out
}

fn write_if_unless_diagnostics(diagnostics: &[Diagnostic], source: &str) -> String {
    let mut out = String::from("[");
    for (i, d) in diagnostics.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        let (line, column) = line_col(source, d.start);
        let _ = write!(out, "{{\"start\":{},\"end\":{},\"line\":{line},\"column\":{column},\"message\":{},\"replacement\":{}}}", d.start, d.end, json_string_field(&d.message), json_string_field(&d.replacement));
    }
    out.push(']');
    out
}

fn write_alignment_diagnostics(diagnostics: &[AlignmentDiagnostic], source: &str) -> String {
    let mut out = String::from("[");
    for (i, d) in diagnostics.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        let (line, _) = line_col(source, d.start);
        let _ = write!(out, "{{\"start\":{},\"end\":{},\"line\":{line},\"expected_column\":{},\"actual_column\":{}}}", d.start, d.end, d.expected_column, d.actual_column);
    }
    out.push(']');
    out
}

fn main() {
    let output_path = std::env::args().nth(1).unwrap_or_else(|| "playground/output.json".to_string());

    let mut json = String::from("{");

    let (cond_source, cond_tree) = parse_file("playground/conditionals.rb");
    json.push_str("\"conditionals\":");
    write_scenario(&mut json, &cond_source, &cond_tree, &[]);

    let (fmt_source, fmt_tree) = parse_file("playground/formatter_before.rb");
    let formatted = render(&fmt_tree, &fmt_source, 0) + "\n";
    json.push_str(",\"formatter\":");
    write_scenario(&mut json, &fmt_source, &fmt_tree, &[("formatted", json_string_field(&formatted))]);

    let mut lint_diagnostics = Vec::new();
    check_if_unless_modifier(&cond_tree, &cond_source, &mut lint_diagnostics);
    json.push_str(",\"linter\":");
    let lint_json = write_if_unless_diagnostics(&lint_diagnostics, &cond_source);
    write_scenario(&mut json, &cond_source, &cond_tree, &[("diagnostics", lint_json)]);

    let (align_source, align_tree) = parse_file("playground/alignment.rb");
    let mut align_diagnostics = Vec::new();
    check_alignment(&align_tree, &align_source, &mut align_diagnostics);
    let corrected = corrected_alignment(&align_source, &align_diagnostics);
    json.push_str(",\"alignment\":");
    let align_json = write_alignment_diagnostics(&align_diagnostics, &align_source);
    write_scenario(&mut json, &align_source, &align_tree, &[("diagnostics", align_json), ("corrected_source", json_string_field(&corrected))]);

    json.push('}');

    std::fs::write(&output_path, &json).unwrap_or_else(|e| panic!("failed to write {output_path}: {e}"));
    println!("wrote {output_path} ({} bytes)", json.len());
}
