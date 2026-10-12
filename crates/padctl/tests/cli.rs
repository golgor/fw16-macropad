//! CLI behaviour against the in-memory fake backend.

use clap::Parser;
use macropad_hid::fake::{FakeBackend, FakeDevice};
use padctl::{Cli, run};

fn invoke(args: &[&str], backend: &FakeBackend) -> anyhow::Result<String> {
    let cli = Cli::try_parse_from(std::iter::once("padctl").chain(args.iter().copied()))?;
    let mut out = Vec::new();
    run(&cli, backend, &mut out)?;
    Ok(String::from_utf8(out)?)
}

#[test]
fn info_reports_via_protocol_version() {
    let backend = FakeBackend::with(vec![FakeDevice::stock("fake0")]);
    assert_eq!(invoke(&["info"], &backend).unwrap(), "via_protocol: 12\n");
}

#[test]
fn list_prints_one_line_per_device() {
    let backend = FakeBackend::with(vec![FakeDevice::stock("fake0")]);
    assert_eq!(
        invoke(&["list"], &backend).unwrap(),
        "fake0\tLaptop 16 RGB Macropad\tFAKE0001\n"
    );
}

#[test]
fn info_without_device_fails_clearly() {
    let err = invoke(&["info"], &FakeBackend::default()).unwrap_err();
    assert!(
        err.to_string()
            .contains("no Framework 16 RGB Macropad found")
    );
}
