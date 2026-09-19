//! Config-driven formatter tests.
//!
//! Tests that formatting options (indent_size, line_length) produce
//! the expected output. Uses insta snapshots since each config
//! variation produces different output from the same input.

use tuxedo::config::{AccessModifierStyle, FormatConfig, HashBraceSpace, QuoteStyle};

const CLASS_INPUT: &str = "\
class Dog
  def speak
    puts \"woof\"
  end
end
";

#[test]
fn indent_size_4() {
    let config = FormatConfig {
        indent_size: 4,
        ..Default::default()
    };
    let output = tuxedo::format_with_config(CLASS_INPUT, &config).unwrap();
    insta::assert_snapshot!(output);
}

#[test]
fn indent_size_default() {
    let config = FormatConfig::default();
    let output = tuxedo::format_with_config(CLASS_INPUT, &config).unwrap();
    insta::assert_snapshot!(output);
}

const ARRAY_INPUT: &str = "ROLES = [\"administrator\", \"moderator\", \"contributor\"]\n";

/// The width the formatter measures against is the configured one, not
/// the 120-column default: the same literal fits on one line under the
/// default and expands under a narrow limit.
#[test]
fn line_length_narrow() {
    let config = FormatConfig {
        line_length: 20,
        ..Default::default()
    };
    let output = tuxedo::format_with_config(ARRAY_INPUT, &config).unwrap();
    insta::assert_snapshot!(output);
}

#[test]
fn line_length_default() {
    let config = FormatConfig::default();
    let output = tuxedo::format_with_config(ARRAY_INPUT, &config).unwrap();
    insta::assert_snapshot!(output);
}

const HASH_INPUT: &str = "h = { a: 1, \"b\" => 2 }\nempty = {}\nblock = list.map { |x| x }\n";

/// `no_space` governs the hash literal only: block braces keep theirs, and
/// an empty hash is `{}` either way.
#[test]
fn hash_brace_space_no_space() {
    let config = FormatConfig {
        hash_brace_space: HashBraceSpace::NoSpace,
        ..Default::default()
    };
    let output = tuxedo::format_with_config(HASH_INPUT, &config).unwrap();
    insta::assert_snapshot!(output);
}

const MODIFIER_INPUT: &str = "\
class Foo
  def a; end

  # about the rest
  private

  def b; end

  class Inner
    protected

    def c; end
  end
end
";

/// The modifier, and the comment written above it, sit at the `class`
/// keyword's indent; a class nested in the body places its own.
#[test]
fn access_modifier_outdent() {
    let config = FormatConfig {
        access_modifier_style: AccessModifierStyle::Outdent,
        ..Default::default()
    };
    let output = tuxedo::format_with_config(MODIFIER_INPUT, &config).unwrap();
    insta::assert_snapshot!(output);
}

#[test]
fn access_modifier_indent_is_the_default() {
    let output = tuxedo::format_with_config(MODIFIER_INPUT, &FormatConfig::default()).unwrap();
    insta::assert_snapshot!(output);
}

const QUOTE_INPUT: &str = "\
a = 'plain'
b = 'has \"dq\"'
c = 'back\\slash'
d = 'hash #{x}'
f = \"simple\"
g = \"it's\"
h = \"esc \\\\ and \\\" ok\"
i = \"tab\\t\"
j = \"interp #{a}\"
k = :'a b'
l = :\"a b\"
m = { 'k': 1, \"j\": 2, \"it's\": 3 }
n = 'a' 'b'
o = 'a' \"b#{c}\"
q = %q(x) + %w[a b].join + ?a
r = <<~EOS
  'heredoc'
EOS
";

/// A literal changes quote only when its value survives: the ones that
/// cannot (an escape, an interpolation, a quote of the other kind) keep
/// what was written, and so do `%q`, `?a` and heredocs.
#[test]
fn quote_style_double_is_the_default() {
    let output = tuxedo::format_with_config(QUOTE_INPUT, &FormatConfig::default()).unwrap();
    insta::assert_snapshot!(output);
}

#[test]
fn quote_style_single() {
    let config = FormatConfig {
        quote_style: QuoteStyle::Single,
        ..Default::default()
    };
    let output = tuxedo::format_with_config(QUOTE_INPUT, &config).unwrap();
    insta::assert_snapshot!(output);
}

#[test]
fn quote_style_preserve() {
    let config = FormatConfig {
        quote_style: QuoteStyle::Preserve,
        ..Default::default()
    };
    let output = tuxedo::format_with_config(QUOTE_INPUT, &config).unwrap();
    insta::assert_snapshot!(output);
}
