//! `padctl` binary entry point: parse args, build the real backend, run.

use std::io::{BufRead, IsTerminal, Write};
use std::process::ExitCode;

use clap::Parser;
use macropad_hid::HidapiBackend;
use padctl::{Cli, run};

/// Ask on stderr, read y/N from stdin. Without a terminal, refuse.
fn confirm(question: &str) -> anyhow::Result<bool> {
    let stdin = std::io::stdin();
    if !stdin.is_terminal() {
        return Ok(false);
    }
    eprint!("{question} [y/N] ");
    std::io::stderr().flush()?;
    let mut answer = String::new();
    stdin.lock().read_line(&mut answer)?;
    Ok(matches!(answer.trim(), "y" | "Y" | "yes"))
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let result = HidapiBackend::new()
        .map_err(anyhow::Error::from)
        .and_then(|backend| run(&cli, &backend, &mut std::io::stdout().lock(), &mut confirm));
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("padctl: {e:#}");
            match e.downcast_ref::<padctl::claim::Exit>() {
                Some(exit) => ExitCode::from(exit.code),
                None => ExitCode::FAILURE,
            }
        }
    }
}
