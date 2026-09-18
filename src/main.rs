use std::path::{Path, PathBuf};

use clap::{Args, Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "tuxedo",
    version,
    about = "A fast Ruby formatter and linter",
    after_help = "Run `tuxedo <command> --help` for more options"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Format Ruby source files
    Format(FormatArgs),
    /// Check Ruby source files for lint violations
    Check(CheckArgs),
}

#[derive(Args)]
struct FormatArgs {
    /// Files or directories to format
    paths: Vec<PathBuf>,

    /// Check if files are formatted without modifying them
    #[arg(long)]
    check: bool,

    /// Print a unified diff of what would change instead of writing
    #[arg(long)]
    diff: bool,

    /// Dump the Prism AST (stdin only)
    #[arg(long)]
    ast: bool,

    /// Report this path instead of "stdin" when reading from stdin
    #[arg(long, value_name = "PATH")]
    stdin_filename: Option<PathBuf>,

    /// Path to config file
    #[arg(long)]
    config: Option<PathBuf>,

    /// Number of threads (default: auto)
    #[arg(long, short)]
    jobs: Option<usize>,

    /// Suppress output except errors
    #[arg(long, short)]
    quiet: bool,

    /// Don't respect .gitignore
    #[arg(long)]
    no_ignore: bool,
}

#[derive(Args)]
struct CheckArgs {
    /// Files or directories to check
    paths: Vec<PathBuf>,

    /// Auto-fix lint violations
    #[arg(long)]
    fix: bool,

    /// Report this path instead of "stdin" when reading from stdin
    #[arg(long, value_name = "PATH")]
    stdin_filename: Option<PathBuf>,

    /// Path to config file
    #[arg(long)]
    config: Option<PathBuf>,

    /// Number of threads (default: auto)
    #[arg(long, short)]
    jobs: Option<usize>,

    /// Suppress output except errors
    #[arg(long, short)]
    quiet: bool,

    /// Don't respect .gitignore
    #[arg(long)]
    no_ignore: bool,
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Command::Format(args) => {
            let config = tuxedo::Config::load(args.config.as_deref());
            run_format(args, config);
        }
        Command::Check(args) => {
            let config = tuxedo::Config::load(args.config.as_deref());
            run_check(args, config);
        }
    }
}

/// Exit status when at least one file could not be formatted (a syntax
/// error, or output that failed the AST-equivalence check) -- distinct from
/// 1, which `--check` uses for "would reformat".
const EXIT_ERROR: i32 = 2;

/// Reads the whole of stdin, or exits.
///
/// An editor pipes a buffer in and writes what comes back over it, so a
/// read that failed -- a broken pipe, bytes that are not UTF-8 -- must
/// not come back as an empty file: printing nothing and exiting 0 would
/// tell the editor the buffer formats to nothing.
fn read_stdin() -> String {
    std::io::read_to_string(std::io::stdin()).unwrap_or_else(|e| {
        eprintln!("error: could not read stdin: {e}");
        std::process::exit(EXIT_ERROR);
    })
}

/// What to call the source on stdin in a message: the path the editor
/// named, so its diagnostics point at a file the user can open.
fn stdin_label(stdin_filename: Option<&PathBuf>) -> String {
    stdin_filename.map_or_else(|| "stdin".to_string(), |path| path.display().to_string())
}

/// What to call a walked file in a diff. `tuxedo format --diff .` walks
/// from `.`, which makes every path `./lib/a.rb`, and `git apply -p0`
/// rejects a patch whose paths start with `./`.
fn diff_label(path: &Path) -> String {
    path.strip_prefix("./")
        .unwrap_or(path)
        .display()
        .to_string()
}

/// A unified diff of one file's formatting, built whole before it is
/// printed: the walk formats on several threads, and one `print!` of one
/// string is one lock on stdout, where a line-at-a-time loop would let
/// two files' diffs interleave.
fn unified_diff(label: &str, before: &str, after: &str) -> String {
    similar::TextDiff::from_lines(before, after)
        .unified_diff()
        .header(label, label)
        .to_string()
}

/// Names the paths the walk could not read, and says whether there were
/// any -- both commands end on the same "did anything go wrong" question.
fn report_unreadable(errors: usize) -> bool {
    if errors == 0 {
        return false;
    }
    let paths = if errors == 1 { "path" } else { "paths" };
    eprintln!("{errors} {paths} could not be read");
    true
}

fn run_format(args: FormatArgs, config: tuxedo::Config) {
    let format_config = &config.format;

    if args.paths.is_empty() {
        let source = read_stdin();
        if args.ast {
            let result = ruby_prism::parse(source.as_bytes());
            println!("{:#?}", result.node());
            return;
        }
        let label = stdin_label(args.stdin_filename.as_ref());
        let formatted = match tuxedo::format_with_config(&source, format_config) {
            Ok(formatted) => formatted,
            Err(e) => {
                eprintln!("error: {label}:{e}");
                std::process::exit(EXIT_ERROR);
            }
        };

        if args.diff {
            print!("{}", unified_diff(&label, &source, &formatted));
        } else if args.check {
            if formatted != source && !args.quiet {
                println!("would reformat: {label}");
            }
        } else {
            // The buffer goes back where it came from, whole: an editor
            // filter replaces what it piped in with what it reads out.
            print!("{formatted}");
        }
        if (args.diff || args.check) && formatted != source {
            std::process::exit(1);
        }
        return;
    }

    // Both report without touching the tree, and both make a run that
    // found work to do exit 1.
    let check_mode = args.check;
    let diff_mode = args.diff;
    let reporting_only = check_mode || diff_mode;
    let quiet = args.quiet;
    let errors = std::sync::atomic::AtomicUsize::new(0);
    let errors_ref = &errors;

    let result = tuxedo::walk_and_process(
        &args.paths,
        args.jobs,
        args.no_ignore,
        move |path, source| {
            let formatted = match tuxedo::format_with_config(source, format_config) {
                Ok(formatted) => formatted,
                Err(e) => {
                    // Left exactly as it was: never write a file we could
                    // not format faithfully.
                    eprintln!("error: {}:{e}", path.display());
                    errors_ref.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    return None;
                }
            };
            if formatted == source {
                return None;
            }

            if diff_mode {
                print!("{}", unified_diff(&diff_label(path), source, &formatted));
            } else if check_mode {
                if !quiet {
                    println!("would reformat: {}", path.display());
                }
            } else {
                if let Err(e) = tuxedo::write_atomic(path, &formatted) {
                    eprintln!("error: could not write {}: {e}", path.display());
                    errors_ref.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    // The file on disk is the one that was read, so it is
                    // neither reformatted nor left alone by choice.
                    return None;
                }
                if !quiet {
                    println!("reformatted: {}", path.display());
                }
            }

            Some(formatted)
        },
    );

    let errors = errors.load(std::sync::atomic::Ordering::Relaxed);
    let unchanged = result.total - result.changed - errors;

    if reporting_only {
        eprintln!(
            "{} files checked: {} would be reformatted, {} already formatted, {} could not be formatted",
            result.total, result.changed, unchanged, errors
        );
    } else if !quiet {
        eprintln!(
            "{} files checked: {} reformatted, {} unchanged, {} could not be formatted or written",
            result.total, result.changed, unchanged, errors
        );
    }
    let unreadable = report_unreadable(result.errors);

    if errors > 0 || unreadable {
        std::process::exit(EXIT_ERROR);
    }
    if reporting_only && result.changed > 0 {
        std::process::exit(1);
    }
}

fn run_check(args: CheckArgs, _config: tuxedo::Config) {
    if args.paths.is_empty() {
        let source = read_stdin();
        let label = stdin_label(args.stdin_filename.as_ref());
        let diagnostics = tuxedo::check(&source);
        for d in &diagnostics {
            println!("{label}:{}:{}: {} [{}]", d.line, d.col, d.message, d.rule);
        }
        return;
    }

    let fix_mode = args.fix;
    let quiet = args.quiet;
    let errors = std::sync::atomic::AtomicUsize::new(0);
    let errors_ref = &errors;

    let result = tuxedo::walk_and_process(
        &args.paths,
        args.jobs,
        args.no_ignore,
        move |path, source| {
            let diagnostics = tuxedo::check(source);

            if !quiet {
                for d in &diagnostics {
                    println!(
                        "{}:{}:{}: {} [{}]",
                        path.display(),
                        d.line,
                        d.col,
                        d.message,
                        d.rule
                    );
                }
            }

            if fix_mode && let Some(fixed) = tuxedo::fix(source) {
                match tuxedo::write_atomic(path, &fixed) {
                    Ok(()) => return Some(fixed),
                    Err(e) => {
                        eprintln!("error: could not write {}: {e}", path.display());
                        errors_ref.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    }
                }
            }

            if diagnostics.is_empty() {
                None
            } else {
                Some(String::new())
            }
        },
    );

    if !quiet {
        eprintln!(
            "{} files checked: {} with issues, {} clean",
            result.total,
            result.changed,
            result.total - result.changed
        );
    }

    let unreadable = report_unreadable(result.errors);

    if errors.load(std::sync::atomic::Ordering::Relaxed) > 0 || unreadable {
        std::process::exit(EXIT_ERROR);
    }
    if result.changed > 0 {
        std::process::exit(1);
    }
}
