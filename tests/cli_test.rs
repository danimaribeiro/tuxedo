//! End-to-end tests for the `tuxedo` binary.
//!
//! These pin the parts of the CLI a caller depends on and the library
//! cannot express: which exit status a run ends on, which files the walk
//! picks up, and whether a run that only reports leaves the tree alone.
//! A CI job or an editor integration is built against exactly these.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const TUXEDO: &str = env!("CARGO_BIN_EXE_tuxedo");

/// A directory of this test's own, removed when the test ends.
struct TempDir(PathBuf);

impl TempDir {
    fn new(name: &str) -> Self {
        let dir = std::env::temp_dir().join(format!("tuxedo-cli-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        Self(dir)
    }

    fn write(&self, name: &str, contents: &str) -> PathBuf {
        let path = self.0.join(name);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, contents).unwrap();
        path
    }

    fn read(&self, name: &str) -> String {
        std::fs::read_to_string(self.0.join(name)).unwrap()
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn format_in(dir: &Path, args: &[&str]) -> Output {
    Command::new(TUXEDO)
        .arg("format")
        .args(args)
        .arg(dir)
        .output()
        .unwrap()
}

fn stdout_of(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn stderr_of(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

#[test]
fn formats_the_ruby_a_project_keeps_outside_rb() {
    let dir = TempDir::new("matcher");
    dir.write("Rakefile", "task :build  do\nend\n");
    dir.write("Gemfile", "gem  \"rails\"\n");
    dir.write("lib/tasks/db.rake", "task :migrate  do\nend\n");
    dir.write("thing.gemspec", "x  =  1\n");
    dir.write("config.ru", "run  app\n");
    dir.write("README.md", "not  ruby\n");

    let output = format_in(&dir.0, &[]);

    assert!(output.status.success(), "{}", stderr_of(&output));
    assert_eq!(dir.read("Rakefile"), "task :build do\nend\n");
    assert_eq!(dir.read("Gemfile"), "gem \"rails\"\n");
    assert_eq!(dir.read("lib/tasks/db.rake"), "task :migrate do\nend\n");
    assert_eq!(dir.read("thing.gemspec"), "x = 1\n");
    assert_eq!(dir.read("config.ru"), "run app\n");
    // Left alone: not Ruby, whatever its contents look like.
    assert_eq!(dir.read("README.md"), "not  ruby\n");
}

#[test]
fn check_reports_without_writing_and_exits_one() {
    let dir = TempDir::new("check");
    dir.write("a.rb", "x  =  1\n");

    let output = format_in(&dir.0, &["--check"]);

    assert_eq!(output.status.code(), Some(1));
    assert!(stdout_of(&output).contains("would reformat"));
    assert_eq!(dir.read("a.rb"), "x  =  1\n");
}

#[test]
fn diff_prints_the_change_without_writing_and_exits_one() {
    let dir = TempDir::new("diff");
    dir.write("a.rb", "x  =  1\n");

    let output = format_in(&dir.0, &["--diff"]);

    assert_eq!(output.status.code(), Some(1));
    let diff = stdout_of(&output);
    assert!(diff.contains("-x  =  1"), "{diff}");
    assert!(diff.contains("+x = 1"), "{diff}");
    assert_eq!(dir.read("a.rb"), "x  =  1\n");
}

#[test]
fn a_diff_of_the_current_directory_carries_paths_patch_can_use() {
    let dir = TempDir::new("diff-paths");
    dir.write("lib/a.rb", "x  =  1\n");

    // As an editor or a CI job runs it: from inside the tree, on `.`.
    let output = Command::new(TUXEDO)
        .args(["format", "--diff", "."])
        .current_dir(&dir.0)
        .output()
        .unwrap();

    let diff = stdout_of(&output);
    assert!(diff.contains("--- lib/a.rb"), "{diff}");
    // `git apply -p0` rejects a patch whose paths start with `./`.
    assert!(!diff.contains("./lib/a.rb"), "{diff}");
}

#[test]
fn an_already_formatted_tree_exits_zero_under_every_reporting_flag() {
    let dir = TempDir::new("clean");
    dir.write("a.rb", "x = 1\n");

    for flag in [[].as_slice(), ["--check"].as_slice(), ["--diff"].as_slice()] {
        let output = format_in(&dir.0, flag);
        assert!(output.status.success(), "{flag:?}: {}", stderr_of(&output));
    }
}

#[test]
fn a_file_that_is_not_utf8_is_refused_and_fails_the_run() {
    let dir = TempDir::new("binary");
    std::fs::write(dir.0.join("bad.rb"), [0xff, 0xfe, b'x']).unwrap();
    dir.write("good.rb", "x  =  1\n");

    let output = format_in(&dir.0, &[]);

    assert_eq!(output.status.code(), Some(2));
    assert!(stderr_of(&output).contains("could not be read"));
    // The one file it could read is still formatted: one bad file does
    // not abandon the run.
    assert_eq!(dir.read("good.rb"), "x = 1\n");
}

#[test]
fn a_file_that_does_not_parse_is_left_exactly_as_it_was() {
    let dir = TempDir::new("syntax");
    dir.write("broken.rb", "def   oops\n");

    let output = format_in(&dir.0, &[]);

    assert_eq!(output.status.code(), Some(2));
    assert_eq!(dir.read("broken.rb"), "def   oops\n");
}

/// Runs `tuxedo` with `source` on stdin.
///
/// The write and the wait cannot share a statement: the `ChildStdin`
/// would live to the end of it, leaving the pipe open while `tuxedo`
/// blocks reading it and the test blocks waiting for `tuxedo`.
fn run_on_stdin(args: &[&str], source: &str) -> Output {
    use std::io::Write;

    let mut child = Command::new(TUXEDO)
        .args(args)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();

    let mut stdin = child.stdin.take().unwrap();
    stdin.write_all(source.as_bytes()).unwrap();
    drop(stdin);

    child.wait_with_output().unwrap()
}

#[test]
fn stdin_filename_names_the_buffer_in_messages() {
    let output = run_on_stdin(
        &["format", "--check", "--stdin-filename", "lib/foo.rb"],
        "x  =  1\n",
    );

    assert_eq!(output.status.code(), Some(1));
    assert!(stdout_of(&output).contains("lib/foo.rb"), "{output:?}");
}

#[test]
fn stdin_without_a_reporting_flag_writes_the_formatted_buffer_back() {
    let output = run_on_stdin(&["format"], "x  =  1\n");

    assert!(output.status.success(), "{}", stderr_of(&output));
    assert_eq!(stdout_of(&output), "x = 1\n");
}

#[test]
fn a_syntax_error_on_stdin_prints_nothing_and_fails() {
    let output = run_on_stdin(
        &["format", "--stdin-filename", "lib/foo.rb"],
        "def   oops\n",
    );

    assert_eq!(output.status.code(), Some(2));
    // Nothing on stdout: an editor writes what comes back over the
    // buffer it sent, so a partial answer would eat the user's work.
    assert_eq!(stdout_of(&output), "");
    assert!(stderr_of(&output).contains("lib/foo.rb"), "{output:?}");
}

const UGLY: &str = "x   =   1\n";
const TIDY: &str = "x = 1\n";

/// A tree with one file in each place an `exclude` glob might reach, and a
/// config that excludes a directory, a name anywhere, and a path.
fn tree_with_excludes(name: &str) -> TempDir {
    let dir = TempDir::new(name);
    dir.write(
        ".tuxedo.toml",
        "exclude = [\"vendor/\", \"*_generated.rb\", \"/db/schema.rb\"]\n",
    );
    for name in [
        "app/a.rb",
        "vendor/gem/b.rb",
        "app/c_generated.rb",
        "db/schema.rb",
        "app/db/schema.rb",
    ] {
        dir.write(name, UGLY);
    }
    dir
}

/// `format` and `check` both skip what `exclude` names, and the globs are
/// gitignore-style: `vendor/` matches a directory at any depth, `*_x.rb` a
/// name anywhere, `/db/schema.rb` only that path from the config's root.
#[test]
fn exclude_skips_the_matching_files_in_format() {
    let dir = tree_with_excludes("exclude-format");
    let output = Command::new(TUXEDO)
        .current_dir(&dir.0)
        .args(["format", "."])
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", stderr_of(&output));

    assert_eq!(dir.read("app/a.rb"), TIDY);
    assert_eq!(dir.read("app/db/schema.rb"), TIDY);
    for kept in ["vendor/gem/b.rb", "app/c_generated.rb", "db/schema.rb"] {
        assert_eq!(dir.read(kept), UGLY, "{kept} should be excluded");
    }
}

#[test]
fn exclude_skips_the_matching_files_when_only_reporting() {
    let dir = tree_with_excludes("exclude-check");
    let output = Command::new(TUXEDO)
        .current_dir(&dir.0)
        .args(["format", "--check", "."])
        .output()
        .unwrap();
    let out = stdout_of(&output) + &stderr_of(&output);
    assert!(out.contains("2 would be reformatted"), "{out}");
}

#[test]
fn the_summary_says_one_file_not_one_files() {
    let dir = TempDir::new("singular");
    let path = dir.write("a.rb", "x = 1\n");
    let output = Command::new(TUXEDO)
        .args(["format", "--check"])
        .arg(&path)
        .output()
        .unwrap();
    assert!(
        stderr_of(&output).contains("1 file checked"),
        "{}",
        stderr_of(&output)
    );
}

#[test]
fn the_check_subcommand_and_the_lint_config_are_gone() {
    let output = Command::new(TUXEDO).args(["check", "."]).output().unwrap();
    assert_eq!(output.status.code(), Some(2));
}

/// The match is against the path from the config's directory, so it does
/// not matter where the run starts or how the directory is spelled.
#[test]
fn exclude_holds_from_a_subdirectory_and_from_an_absolute_path() {
    let dir = tree_with_excludes("exclude-cwd");
    let output = Command::new(TUXEDO)
        .current_dir(dir.0.join("app"))
        .args(["format", ".."])
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", stderr_of(&output));
    assert_eq!(dir.read("vendor/gem/b.rb"), UGLY);
    assert_eq!(dir.read("db/schema.rb"), UGLY);
    assert_eq!(dir.read("app/a.rb"), TIDY);

    let output = Command::new(TUXEDO)
        .current_dir(&dir.0)
        .args(["format", "--no-ignore"])
        .arg(&dir.0)
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", stderr_of(&output));
    assert_eq!(
        dir.read("app/c_generated.rb"),
        UGLY,
        "--no-ignore keeps exclude"
    );
}

/// Naming a file on the command line formats it even when it is excluded.
#[test]
fn a_file_named_on_the_command_line_is_formatted_though_excluded() {
    let dir = tree_with_excludes("exclude-named");
    let output = Command::new(TUXEDO)
        .current_dir(&dir.0)
        .args(["format", "vendor/gem/b.rb"])
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", stderr_of(&output));
    assert_eq!(dir.read("vendor/gem/b.rb"), TIDY);
    assert_eq!(dir.read("app/c_generated.rb"), UGLY);
}

#[test]
fn exclude_is_relative_to_an_explicit_config_file() {
    let dir = TempDir::new("exclude-explicit");
    dir.write("conf/tuxedo.toml", "exclude = [\"skip/\"]\n");
    dir.write("conf/skip/a.rb", UGLY);
    dir.write("conf/keep/a.rb", UGLY);
    let output = Command::new(TUXEDO)
        .current_dir(&dir.0)
        .args(["format", "--config", "conf/tuxedo.toml", "conf"])
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", stderr_of(&output));
    assert_eq!(dir.read("conf/skip/a.rb"), UGLY);
    assert_eq!(dir.read("conf/keep/a.rb"), TIDY);
}

#[test]
fn a_negated_exclude_glob_is_refused() {
    let dir = TempDir::new("exclude-negated");
    dir.write(".tuxedo.toml", "exclude = [\"!keep.rb\"]\n");
    dir.write("a.rb", UGLY);
    let output = Command::new(TUXEDO)
        .current_dir(&dir.0)
        .args(["format", "."])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert!(
        stderr_of(&output).contains("leading `!`"),
        "{}",
        stderr_of(&output)
    );
    assert_eq!(dir.read("a.rb"), UGLY);
}

#[test]
fn a_file_nested_too_deep_is_reported_and_the_rest_are_still_formatted() {
    let dir = TempDir::new("too-deep");
    dir.write(
        "deep.rb",
        &format!("x = {}1{}\n", "(".repeat(5000), ")".repeat(5000)),
    );
    dir.write("ok.rb", "x  =  1\n");

    let output = Command::new(TUXEDO)
        .arg("format")
        .arg(&dir.0)
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&output.stderr).contains("deep.rb"));
    assert_eq!(dir.read("ok.rb"), "x = 1\n");
}
