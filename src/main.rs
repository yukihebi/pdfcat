mod cli;
mod merge;
mod pages;
mod runner;

use std::io;
use std::process::ExitCode;

use thiserror::Error;

use pages::PageSpecError;
use runner::{VerboseLog, execute_run, load_sources};

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        // Bare `pdfcat`: show usage instead of an error line.
        Err(Error::NoArguments) => {
            eprint!("{}", cli::HELP);
            ExitCode::FAILURE
        }
        Err(err) => {
            eprintln!("pdfcat: error: {err}");
            ExitCode::FAILURE
        }
    }
}

/// Anything that can stop pdfcat from producing its output.
#[derive(Debug, Error)]
enum Error {
    #[error("no arguments given")]
    NoArguments,
    #[error(transparent)]
    Cli(#[from] cli::CliError),
    #[error(transparent)]
    Merge(#[from] merge::MergeError),
    #[error("{path}: cannot read PDF: {source}")]
    ReadInput { path: String, source: lopdf::Error },
    #[error("{path}: this PDF has no pages")]
    NoPages { path: String },
    #[error("{path}: {source}")]
    PageSelection { path: String, source: PageSpecError },
    #[error("cannot write {path}: {source}")]
    WriteOutput {
        path: String,
        source: std::io::Error,
    },
    #[error("failed to write count to stdout: {0}")]
    ReportIo(std::io::Error),
}

fn run() -> Result<(), Error> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() {
        return Err(Error::NoArguments);
    }

    match cli::parse(&args)? {
        cli::Command::Help => {
            print!("{}", cli::HELP);
            Ok(())
        }
        cli::Command::Version => {
            println!("pdfcat {}", cli::VERSION);
            Ok(())
        }
        cli::Command::Run {
            inputs,
            output,
            count_pages,
            count_bytes,
            quiet,
            verbose,
        } => {
            let stdout = io::stdout();
            let mut report = stdout.lock();
            let stderr = io::stderr();
            if verbose {
                let mut vlog = VerboseLog::new(true, stderr.lock());
                let sources = load_sources(&inputs, &mut vlog)?;
                let mut merged = merge::merge(sources)?;
                execute_run(
                    &mut merged,
                    output.as_deref(),
                    count_pages,
                    count_bytes,
                    quiet,
                    &mut report,
                    &mut vlog,
                )
            } else {
                let mut vlog = VerboseLog::new(false, io::sink());
                let sources = load_sources(&inputs, &mut vlog)?;
                let mut merged = merge::merge(sources)?;
                execute_run(
                    &mut merged,
                    output.as_deref(),
                    count_pages,
                    count_bytes,
                    quiet,
                    &mut report,
                    &mut vlog,
                )
            }
        }
    }
}
