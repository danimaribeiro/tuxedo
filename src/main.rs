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

fn run_format(args: FormatArgs, _config: tuxedo::Config) {
    if args.paths.is_empty() {
        let source = std::io::read_to_string(std::io::stdin()).unwrap_or_default();
        if args.ast {
            let result = ruby_prism::parse(source.as_bytes());
            println!("{:#?}", result.node());
        } else {
            print!("{}", tuxedo::format(&source));
        }
        return;
    }

    let check_mode = args.check;
    let quiet = args.quiet;

    let result = tuxedo::walk_and_process(
        &args.paths,
        args.jobs,
        args.no_ignore,
        move |path, source| {
            let formatted = tuxedo::format(source);
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

    let unchanged = result.total - result.changed;

    if check_mode {
        eprintln!(
            "{} files checked: {} would be reformatted, {} already formatted",
            result.total, result.changed, unchanged
        );
        if result.changed > 0 {
            std::process::exit(1);
        }
    } else if !quiet {
        eprintln!(
            "{} files checked: {} reformatted, {} unchanged",
            result.total, result.changed, unchanged
        );
    }
}

fn run_check(args: CheckArgs, _config: tuxedo::Config) {
    if args.paths.is_empty() {
        let source = std::io::read_to_string(std::io::stdin()).unwrap_or_default();
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

            if fix_mode {
                if let Some(fixed) = tuxedo::fix(source) {
                    if let Err(e) = std::fs::write(path, &fixed) {
                        eprintln!("error: could not write {}: {e}", path.display());
                    }
                    return Some(fixed);
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

    if result.changed > 0 {
        std::process::exit(1);
    }
}
