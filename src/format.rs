use ruby_prism::Visit;

pub fn format(source: &str) -> String {
    let result = ruby_prism::parse(source.as_bytes());
    let node = result.node();

    let mut formatter = Formatter::new();
    formatter.visit(&node);
    formatter.output
}

struct Formatter {
    output: String,
}

impl Formatter {
    fn new() -> Self {
        Self {
            output: String::new(),
        }
    }
}

impl Visit<'_> for Formatter {
    fn visit_call_node(&mut self, node: &ruby_prism::CallNode<'_>) {
        let name = node.name();
        let name_str = std::str::from_utf8(name.as_slice()).unwrap_or("");
        self.output.push_str(name_str);

        if let Some(args) = node.arguments() {
            self.output.push(' ');
            let arguments = args.arguments();
            for (i, arg) in arguments.iter().enumerate() {
                if i > 0 {
                    self.output.push_str(", ");
                }
                self.visit(&arg);
            }
        }

        self.output.push('\n');
    }

    fn visit_string_node(&mut self, node: &ruby_prism::StringNode<'_>) {
        self.output.push('"');
        self.output.push_str(std::str::from_utf8(node.unescaped()).unwrap_or(""));
        self.output.push('"');
    }
}
