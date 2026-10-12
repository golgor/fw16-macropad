//! `padctl` binary entry point: parse args, build the real backend, run.

use std::io::{BufRead, IsTerminal, Write};

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

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let backend = HidapiBackend::new()?;
    run(&cli, &backend, &mut std::io::stdout().lock(), &mut confirm)
}
