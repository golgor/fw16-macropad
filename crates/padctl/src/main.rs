//! `padctl` binary entry point: parse args, build the real backend, run.

use clap::Parser;
use macropad_hid::HidapiBackend;
use padctl::{Cli, run};

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let backend = HidapiBackend::new()?;
    run(&cli, &backend, &mut std::io::stdout().lock())
}
