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
