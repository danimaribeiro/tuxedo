use crate::lexer::Token;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Node {
    Call { name: String, args: Vec<Node> },
    StringLit(String),
}

pub fn parse(tokens: &[Token]) -> Vec<Node> {
    let mut nodes = Vec::new();
    let mut pos = 0;

    while pos < tokens.len() {
        match &tokens[pos] {
            Token::Ident(name) => {
                let name = name.clone();
                pos += 1;
                let mut args = Vec::new();
                while pos < tokens.len() {
                    match &tokens[pos] {
                        Token::StringLit(s) => {
                            args.push(Node::StringLit(s.clone()));
                            pos += 1;
                        }
                        Token::Ident(s) => {
                            args.push(Node::StringLit(s.clone()));
                            pos += 1;
                        }
                        _ => break,
                    }
                }
                nodes.push(Node::Call { name, args });
            }
            Token::Newline | Token::Eof => {
                pos += 1;
            }
            Token::StringLit(_) => {
                pos += 1;
            }
        }
    }

    nodes
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::lex;

    #[test]
    fn parses_hello_world() {
        let tokens = lex("puts \"hello world\"\n");
        let ast = parse(&tokens);
        insta::assert_debug_snapshot!(ast);
    }
}
