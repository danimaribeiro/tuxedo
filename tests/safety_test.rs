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
fn a_clause_after_an_empty_body_has_no_line_to_itself() {
    assert_unchanged("begin\nend\n");
    assert_unchanged("def foo\nrescue\nend\n");
    assert_unchanged("def foo\nensure\n  cleanup\nend\n");
    assert_unchanged("proc do\nrescue\nend\n");
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

/// `# tuxedo:off` copies the statements up to `# tuxedo:on` through as
/// written, heredoc included, and formats everything around them.
#[test]
fn a_region_between_the_markers_is_left_as_written() {
    let source = "\
def rows
  a   = 1
  # tuxedo:off
  m = [
    [1,   0],
    [0,   1],
  ]
  t   =   <<~EOS
    keep   this
  EOS
  # tuxedo:on
  b   = 2
end
";
    let expected = source
        .replace("a   = 1", "a = 1")
        .replace("b   = 2", "b = 2");
    assert_eq!(format(source), expected);
    assert_unchanged(&expected);
}

/// With no `on`, the region runs to the end of the body it opened in, and
/// a `# tuxedo:off` before the first statement holds the whole file.
#[test]
fn a_region_without_an_end_runs_to_the_end_of_its_body() {
    assert_eq!(
        format("def a\n  p   1\n  # tuxedo:off\n  q   =   2\nend\nz   =   1\n"),
        "def a\n  p 1\n  # tuxedo:off\n  q   =   2\nend\n\nz = 1\n"
    );
    assert_unchanged("# tuxedo:off\nx   =   1\n\ny   =   2\n");
}

/// A marker that is not on a line of its own is an ordinary comment.
#[test]
fn a_trailing_marker_comment_is_not_a_marker() {
    assert_eq!(
        format("x   =   1 # tuxedo:off\ny   =   2\n"),
        "x = 1 # tuxedo:off\ny = 2\n"
    );
}

#[test]
fn markers_that_do_not_pair_up_are_refused() {
    for (source, line) in [
        ("x = 1\n# tuxedo:on\n", 2),
        ("# tuxedo:off\nx = 1\n# tuxedo:off\ny = 2\n", 3),
        // Inside a literal, where a region cannot be honoured.
        ("h = {\n  # tuxedo:off\n  a:   1\n}\n", 2),
        // Nothing after it to protect.
        ("def a\n  x\n  # tuxedo:off\nend\n", 3),
    ] {
        match tuxedo::format(source) {
            Err(FormatError::Syntax { line: at, .. }) => assert_eq!(at, line, "{source:?}"),
            other => panic!("{source:?}: expected a marker error, got {other:?}"),
        }
    }
}

/// A region keeps its own text, and the file keeps one line ending
/// throughout: the region's are written like everything else's.
#[test]
fn a_region_in_a_crlf_file_comes_out_with_crlf() {
    let output = format("a   =   1\r\n# tuxedo:off\r\nb   =   2\r\nc   =   3\r\n# tuxedo:on\r\n");
    assert_eq!(
        output,
        "a = 1\r\n# tuxedo:off\r\nb   =   2\r\nc   =   3\r\n# tuxedo:on\r\n"
    );
}

#[test]
fn nesting_past_the_limit_is_an_error_not_an_abort() {
    let source = format!("x = {}1{}\n", "(".repeat(5000), ")".repeat(5000));
    assert!(matches!(
        tuxedo::format(&source),
        Err(FormatError::TooDeep { .. })
    ));
}

#[test]
fn long_operator_chains_do_not_overflow_the_stack() {
    let source = format!("x = a{}\n", " + a".repeat(20000));
    assert!(tuxedo::format(&source).is_ok());
}
