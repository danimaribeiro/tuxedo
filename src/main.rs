use std::path::PathBuf;

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

    /// Dump the Prism AST (stdin only)
    #[arg(long)]
    ast: bool,

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
        match tuxedo::format_with_config(&source, format_config) {
            Ok(formatted) => print!("{formatted}"),
            Err(e) => {
                eprintln!("error: stdin:{e}");
                std::process::exit(EXIT_ERROR);
            }
        }
        return;
    }

    let check_mode = args.check;
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

            if check_mode {
                if !quiet {
                    println!("would reformat: {}", path.display());
                }
            } else {
                if let Err(e) = std::fs::write(path, &formatted) {
                    eprintln!("error: could not write {}: {e}", path.display());
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

    if check_mode {
        eprintln!(
            "{} files checked: {} would be reformatted, {} already formatted, {} could not be formatted",
            result.total, result.changed, unchanged, errors
        );
    } else if !quiet {
        eprintln!(
            "{} files checked: {} reformatted, {} unchanged, {} could not be formatted",
            result.total, result.changed, unchanged, errors
        );
    }
    let unreadable = report_unreadable(result.errors);

    if errors > 0 || unreadable {
        std::process::exit(EXIT_ERROR);
    }
    if check_mode && result.changed > 0 {
        std::process::exit(1);
    }
}

fn run_check(args: CheckArgs, _config: tuxedo::Config) {
    if args.paths.is_empty() {
        let source = read_stdin();
        let diagnostics = tuxedo::check(&source);
        for d in &diagnostics {
            println!("stdin:{}:{}: {} [{}]", d.line, d.col, d.message, d.rule);
        }
        return;
    }

    let fix_mode = args.fix;
    let quiet = args.quiet;

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
                if let Err(e) = std::fs::write(path, &fixed) {
                    eprintln!("error: could not write {}: {e}", path.display());
                }
                return Some(fixed);
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

    if report_unreadable(result.errors) {
        std::process::exit(EXIT_ERROR);
    }
    if result.changed > 0 {
        std::process::exit(1);
    }
}
