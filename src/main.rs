mod cli;
mod merge;
mod pages;

use std::process::ExitCode;

use lopdf::Document;
use thiserror::Error;

use cli::{Command, Input};
use pages::{PageSpecError, resolve_ranges};

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
}

fn run() -> Result<(), Error> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() {
        return Err(Error::NoArguments);
    }

    match cli::parse(&args)? {
        Command::Help => {
            print!("{}", cli::HELP);
            Ok(())
        }
        Command::Version => {
            println!("pdfcat {}", cli::VERSION);
            Ok(())
        }
        Command::Run { inputs, output, .. } => {
            let output = output.expect("temporary: --count-* not yet wired up");
            let sources = load_sources(&inputs)?;
            let mut merged = merge::merge(sources)?;
            merged.save(&output).map_err(|source| Error::WriteOutput {
                path: output.clone(),
                source,
            })?;
            Ok(())
        }
    }
}

/// Load each input document and resolve its page selection against the actual
/// page count.
fn load_sources(inputs: &[Input]) -> Result<Vec<(Document, Vec<u32>)>, Error> {
    let mut sources = Vec::with_capacity(inputs.len());
    for input in inputs {
        let doc = Document::load(&input.path).map_err(|source| Error::ReadInput {
            path: input.path.clone(),
            source,
        })?;
        let total = doc.get_pages().len() as u32;
        if total == 0 {
            return Err(Error::NoPages {
                path: input.path.clone(),
            });
        }
        let selected: Vec<u32> = match &input.ranges {
            None => (1..=total).collect(),
            Some(ranges) => {
                resolve_ranges(ranges, total).map_err(|source| Error::PageSelection {
                    path: input.path.clone(),
                    source,
                })?
            }
        };
        sources.push((doc, selected));
    }
    Ok(sources)
}
