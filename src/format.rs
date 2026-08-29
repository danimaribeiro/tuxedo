use crate::lexer;
use crate::parser::{self, Node};

pub fn format(source: &str) -> String {
    let tokens = lexer::lex(source);
    let nodes = parser::parse(&tokens);

    let mut out = String::new();
    for node in &nodes {
        format_node(node, &mut out);
        out.push('\n');
    }
    out
}

fn format_node(node: &Node, out: &mut String) {
    match node {
        Node::Call { name, args } => {
            out.push_str(name);
            if !args.is_empty() {
                out.push(' ');
                for (i, arg) in args.iter().enumerate() {
                    if i > 0 {
                        out.push_str(", ");
                    }
                    format_node(arg, out);
                }
            }
        }
        Node::StringLit(s) => {
            out.push('"');
            out.push_str(s);
            out.push('"');
        }
    }
}
