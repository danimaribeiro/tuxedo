fn main() {
    let args: Vec<String> = std::env::args().collect();
    let source = std::io::read_to_string(std::io::stdin()).unwrap_or_default();

    if args.iter().any(|a| a == "--ast") {
        let result = ruby_prism::parse(source.as_bytes());
        println!("{:#?}", result.node());
    } else {
        print!("{}", tuxedo::format(&source));
    }
}
