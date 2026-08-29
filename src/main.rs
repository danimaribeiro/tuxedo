fn main() {
    let source = std::io::read_to_string(std::io::stdin()).unwrap_or_default();
    print!("{}", tuxedo::format(&source));
}
