mod cli;
mod merge;
mod pages;
mod runner;

use std::process::ExitCode;

use thiserror::Error;

use cli::Command;
use pages::PageSpecError;
use runner::OutputOpts;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() {
        eprint!("{}", cli::HELP);
        return ExitCode::FAILURE;
    }
    match run(&args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("pdfcat: {err}");
            ExitCode::FAILURE
        }
    }
}

/// Anything that can stop pdfcat from producing its output.
#[derive(Debug, Error)]
enum Error {
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

fn run(args: &[String]) -> Result<(), Error> {
    match cli::parse(args)? {
        Command::Help => {
            print!("{}", cli::HELP);
            Ok(())
        }
        Command::Version => {
            println!("pdfcat {}", cli::VERSION);
            Ok(())
        }
        Command::Run {
            inputs,
            output,
            count_pages,
            count_bytes,
            quiet,
            verbose,
        } => {
            let opts = OutputOpts {
                output: output.as_deref(),
                count_pages,
                count_bytes,
                quiet,
            };
            runner::run_pipeline(&inputs, &opts, verbose)
        }
    }
}
