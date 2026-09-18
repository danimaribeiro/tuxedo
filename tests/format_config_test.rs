//! Config-driven formatter tests.
//!
//! Tests that formatting options (indent_size, line_length) produce
//! the expected output. Uses insta snapshots since each config
//! variation produces different output from the same input.

use tuxedo::config::FormatConfig;

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
