//! Command line client for Amendable Git storage.

pub mod client;
pub mod config;

mod cli;
mod error;

use std::process::ExitCode;

pub use error::Error;

/// Parse argv and run a command. Process exit happens here.
pub fn run() -> ExitCode {
    match cli::try_run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => err.exit(),
    }
}
