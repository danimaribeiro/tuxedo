//! What Tuxedo does with the shape of the input itself, before any Ruby is
//! laid out: line endings, a byte-order mark, files with no code in them,
//! and the check that refuses output which is a different program.

use tuxedo::FormatError;

fn format(source: &str) -> String {
    tuxedo::format(source).unwrap_or_else(|e| panic!("{source:?} failed: {e}"))
}

#[test]
fn a_crlf_file_keeps_crlf_everywhere() {
    let source = "x  =  1 # c\r\n\r\n\r\ny = \"a\r\nb\"\r\nz = <<~E\r\n  hi\r\nE\r\n";
    let expected = "x = 1 # c\r\n\r\ny = \"a\r\nb\"\r\nz = <<~E\r\n  hi\r\nE\r\n";
    assert_eq!(format(source), expected);
    assert_eq!(format(expected), expected);
}

#[test]
fn an_lf_file_stays_lf() {
    assert_eq!(format("x  =  1\ny = 2\n"), "x = 1\ny = 2\n");
}

#[test]
fn a_byte_order_mark_is_kept() {
    assert_eq!(format("\u{feff}x  =  1\n"), "\u{feff}x = 1\n");
    assert_eq!(format("\u{feff}x = 1\n"), "\u{feff}x = 1\n");
}

#[test]
fn an_empty_file_becomes_one_newline() {
    assert_eq!(format(""), "\n");
    assert_eq!(format("\n"), "\n");
    assert_eq!(format("\n\n\n"), "\n");
}

#[test]
fn a_file_of_only_comments_keeps_them_and_their_blank_lines() {
    assert_eq!(format("# a\n# b\n"), "# a\n# b\n");
    assert_eq!(format("#a\n\n\n# b\n"), "# a\n\n# b\n");
    assert_eq!(format("=begin\ndoc\n=end\n"), "=begin\ndoc\n=end\n");
}

#[test]
fn a_file_of_only_data_is_copied_through() {
    assert_eq!(format("__END__\ndata\n"), "__END__\ndata\n");
    assert_eq!(format("__END__\r\ndata\r\n"), "__END__\r\ndata\r\n");
    // Data is bytes, not code: no newline is added, none is removed.
    assert_eq!(format("__END__\nno newline"), "__END__\nno newline");
}

#[test]
fn code_and_data_are_each_left_their_own_line_endings() {
    assert_eq!(
        format("x  = 1\n__END__\r\nfoo\r\n"),
        "x = 1\n\n__END__\r\nfoo\r\n"
    );
}

#[test]
fn a_different_program_is_refused_as_an_ast_mismatch() {
    assert!(matches!(
        tuxedo::verify_same_program("x = 1\n", "x = 2\n"),
        Err(FormatError::AstMismatch { .. })
    ));
    assert!(matches!(
        tuxedo::verify_same_program("x = 1\n", "x = = 1\n"),
        Err(FormatError::AstMismatch { .. })
    ));
}

#[test]
fn the_same_program_in_another_layout_is_accepted() {
    assert!(tuxedo::verify_same_program("x=[1,2]\n", "x = [1, 2]\n").is_ok());
    assert!(tuxedo::verify_same_program("foo(1) # a\n", "foo(1) # a\n").is_ok());
}

#[test]
fn a_lost_comment_is_a_different_program() {
    assert!(matches!(
        tuxedo::verify_same_program("x = 1 # keep\n", "x = 1\n"),
        Err(FormatError::AstMismatch { .. })
    ));
}
