//! Comments stay next to the code they describe.
//!
//! `tuxedo::format` already guarantees no comment is lost (see
//! `src/format/verify.rs`). These pin down *placement*: every input here is
//! already formatted, and each one used to come back with its comments
//! moved to after the statement, where they described nothing.

fn assert_unchanged(source: &str) {
    let output = tuxedo::format(source).unwrap_or_else(|e| panic!("{source:?} failed: {e}"));
    assert_eq!(output, source);
}

#[test]
fn trailing_comment_on_a_header_line() {
    assert_unchanged(
        "module A # :nodoc:\n  class B < C # :nodoc:\n    def bar # why\n      x\n    end\n  end\nend\n",
    );
    assert_unchanged("if a # why\n  b\nelse # otherwise\n  c\nend\n");
    assert_unchanged("unless a # why\n  b\nend\n");
    assert_unchanged("while a # why\n  b\nend\n");
    assert_unchanged("case x\nwhen 1 # one\n  a\nend\n");
    assert_unchanged("foo do |x| # blk\n  x\nend\n");
    assert_unchanged(
        "begin # start\n  a\nrescue Foo => e # oops\n  b\nensure # always\n  c\nend\n",
    );
}

#[test]
fn comment_at_the_bottom_of_a_body() {
    assert_unchanged("def foo\n  # Override in subclasses\nend\n");
    assert_unchanged("def bar\n  x\n\n  # tail\nend\n");
    assert_unchanged("class A\n  # nothing yet\nend\n");
    assert_unchanged("foo do |x|\n  x\n  # end of blk\nend\n");
    assert_unchanged("if a\n  b\nelse\n  c\n  # tail\nend\n");
    assert_unchanged("def m\n  a\n  # before rescue\nrescue\n  b\n  # before end\nend\n");
}

#[test]
fn comment_above_a_branch_keyword_keeps_its_level() {
    // Aligned with the keyword: about the branch that follows.
    assert_unchanged("if a\n  b\n# about else\nelse\n  c\nend\n");
    // Indented like the body: the bottom of the branch above.
    assert_unchanged(
        "case x\n# about one\nwhen 1\n  a\n  # bottom of one\n# about two\nwhen 2\n  b\nend\n",
    );
}

#[test]
fn comment_inside_a_brace_block_forces_it_open() {
    assert_unchanged("bar { # brace\n  y\n}\n");
    assert_unchanged("baz { |z|\n  # only comment\n}\n");
}

#[test]
fn comment_inside_a_list_forces_it_open() {
    assert_unchanged("foo(\n  1, # one\n  2 # two\n)\n");
    assert_unchanged(
        "x = [ # list\n  # leading\n  1,\n\n  # second group\n  2,\n  3 # three\n  # dangling\n]\n",
    );
    assert_unchanged("h = {\n  a: 1, # one\n  # lead b\n  b: [\n    1 # nested\n  ]\n}\n");
    assert_unchanged("bar(\n  x,\n  k: 1, # kw\n  &blk\n)\n");
    assert_unchanged(
        "def m\n  super(\n    1, # one\n    2\n  )\n  yield(\n    # lead\n    3\n  )\nend\n",
    );
}

#[test]
fn trailing_comment_in_a_bare_argument_list_stays_with_its_argument() {
    assert_unchanged("foo a,\n  b, # about b\n  c\n");
    assert_unchanged("foo a, # about a\n  b,\n  c # about c\n");
    // Two arguments on the comment's line: the comment is the second's.
    let output = tuxedo::format("foo a, b, # about b\n  c\n").unwrap();
    assert_eq!(output, "foo a,\n  b, # about b\n  c\n");
}

#[test]
fn a_list_without_comments_still_collapses() {
    let output = tuxedo::format("foo(\n  1,\n  2\n) # after\n").unwrap();
    assert_eq!(output, "foo(1, 2) # after\n");
}

#[test]
fn comment_between_operands_or_chain_links_keeps_the_expression_verbatim() {
    assert_unchanged("x = a && # why\n  b\n");
    assert_unchanged("foo\n  .bar # first\n  # about baz\n  .baz\n");
    assert_unchanged("methods = (a(true) -\n  # Except base\n  B.c(true)).uniq\n");
}

#[test]
fn placing_comments_is_idempotent() {
    let source = "case d\n    # Quoted\nwhen 1\n  a\nend\n";
    let once = tuxedo::format(source).unwrap();
    assert_eq!(once, "case d\n# Quoted\nwhen 1\n  a\nend\n");
    assert_eq!(tuxedo::format(&once).unwrap(), once);
}
