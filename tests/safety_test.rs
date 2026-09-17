//! The guarantee that matters most: Tuxedo never changes what a program
//! does. `tuxedo::format` parses its own output and compares ASTs, so
//! `Ok(_)` from it already means "same program" -- these tests pin down
//! the inputs that used to come out as a *different* program (or as no
//! program at all), plus what the output looks like now.

use tuxedo::FormatError;

fn format(source: &str) -> String {
    tuxedo::format(source).unwrap_or_else(|e| panic!("{source:?} failed: {e}"))
}

/// Already-formatted input must come back byte for byte.
fn assert_unchanged(source: &str) {
    assert_eq!(format(source), source);
}

#[test]
fn syntax_errors_are_refused() {
    let err = tuxedo::format("def foo(\n  x = ").unwrap_err();
    match err {
        FormatError::Syntax { line, column, .. } => assert_eq!((line, column), (2, 5)),
        other => panic!("expected a syntax error, got {other:?}"),
    }
    assert!(matches!(
        tuxedo::format("x = 1 +\n"),
        Err(FormatError::Syntax { .. })
    ));
}

#[test]
fn nodes_without_a_formatter_are_copied_through() {
    assert_unchanged("puts \"a\" if defined?(Foo)\n");
    assert_unchanged("undef foo\n");
    assert_unchanged("x = __ENCODING__\n");
    assert_unchanged("flip if (a..b)\n");
    assert_unchanged("foo do\n  redo\nend\n");
    assert_unchanged("begin\n  z\nrescue\n  retry\nend\n");
}

#[test]
fn implicit_block_parameters() {
    assert_unchanged("x.map { it * 2 }\n");
    assert_unchanged("x.map { _1 * 2 }\n");
    assert_unchanged("x.each do\n  puts it\nend\n");
    assert_unchanged("-> { it * 2 }\n");
}

#[test]
fn block_local_variables() {
    assert_unchanged("foo do |b; c|\n  c\nend\n");
    assert_unchanged("foo { |a, b; c, d| c }\n");
    assert_unchanged("foo { |; c| c }\n");
}

#[test]
fn pattern_matching() {
    assert_unchanged(
        "y = 1\ncase v\nin Integer | Float => n\n  n\nin [*, 3, *]\n  1\nin ^y\n  2\nin ^(y + 1)\n  3\nend\n",
    );
}

#[test]
fn data_section_is_preserved_verbatim() {
    assert_unchanged("puts 1\n\n__END__\ndata  here\n\n  x");
    assert_unchanged("__END__\ndata\n");
    assert_eq!(format("puts 1\n__END__\nraw\n"), "puts 1\n\n__END__\nraw\n");
}

#[test]
fn operator_methods_called_with_a_dot() {
    assert_unchanged("a&.+(1)\n");
    assert_unchanged("a.+(1)\n");
    assert_unchanged("x = @r&.env&.[](\"a\")\n");
    assert_unchanged("loop do\n  next if t&.!= n\nend\n");
}

#[test]
fn unary_sign_before_a_numeric_literal() {
    assert_unchanged("v = + 1.second\n");
    assert_unchanged("u = - 2.abs\n");
    assert_unchanged("w = -2\n");
    assert_unchanged("x = -a.b\n");
}

#[test]
fn unary_operator_method_names() {
    assert_unchanged("def +@\n  1\nend\n\ndef ~@\n  2\nend\n\ndef !\n  3\nend\n");
}

#[test]
fn squiggly_heredoc_starting_with_interpolation_keeps_its_margin() {
    assert_unchanged("x = <<~MSG\n  #{a} one\n    two\nMSG\n");
    assert_unchanged("foo(<<~A, <<~B)\n  #{a}\n   x\nA\n    #{b}\n  y\nB\n");
}

#[test]
fn not_keyword_keeps_its_precedence() {
    assert_unchanged("x = not a == b\n");
    assert_unchanged("y = not(a) == b\n");
    assert_unchanged("puts \"x\" if not foo\n");
    assert_unchanged("w = !a\n");
}
