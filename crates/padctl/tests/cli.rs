//! CLI behaviour against the in-memory fake backend.

use clap::Parser;
use macropad_hid::fake::{FakeBackend, FakeDevice};
use padctl::{Cli, run};

fn invoke(args: &[&str], backend: &FakeBackend) -> anyhow::Result<String> {
    let cli = Cli::try_parse_from(std::iter::once("padctl").chain(args.iter().copied()))?;
    let mut out = Vec::new();
    run(&cli, backend, &mut out, &mut |_| Ok(false))?;
    Ok(String::from_utf8(out)?)
}

fn stock() -> FakeBackend {
    FakeBackend::with(vec![FakeDevice::stock("fake0")])
}

fn padctl_fw() -> FakeBackend {
    FakeBackend::with(vec![FakeDevice::padctl("fake0")])
}

#[test]
fn info_reports_via_protocol_and_per_key_support() {
    assert_eq!(
        invoke(&["info"], &stock()).unwrap(),
        "via_protocol: 12\nper_key: unavailable (firmware does not answer channel 0)\n"
    );
    assert_eq!(
        invoke(&["info"], &padctl_fw()).unwrap(),
        "via_protocol: 12\nper_key: v2 (24 keys)\n"
    );
}

#[test]
fn list_prints_one_line_per_device() {
    assert_eq!(
        invoke(&["list"], &stock()).unwrap(),
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

#[test]
fn light_commands_work_on_stock_firmware() {
    let b = stock();
    assert_eq!(
        invoke(&["light", "show"], &b).unwrap(),
        "effect: 1\nbrightness: 255\nspeed: 127\nhue: 0\nsat: 0\n"
    );
    for args in [
        &["light", "color", "#00ff00"][..],
        &["light", "brightness", "40"],
        &["light", "effect", "3"],
        &["light", "speed", "9"],
        &["light", "off"],
        &["light", "save"],
    ] {
        invoke(args, &b).unwrap();
    }
}

#[test]
fn key_commands_need_padctl_firmware() {
    let err = invoke(&["key", "set", "-c", "red", "0", "1"], &stock()).unwrap_err();
    assert!(err.to_string().contains("padctl firmware"), "{err}");

    let b = padctl_fw();
    invoke(&["key", "set", "-c", "red", "0", "1"], &b).unwrap();
    invoke(&["key", "set", "--color", "#0000ff", "5"], &b).unwrap();
    invoke(&["key", "clear", "0"], &b).unwrap();
    invoke(&["key", "clear", "--all"], &b).unwrap();
    invoke(&["key", "mode", "overlay"], &b).unwrap();
    invoke(&["key", "mode", "exclusive"], &b).unwrap();
    assert!(invoke(&["key", "mode", "x"], &b).is_err());
}

#[test]
fn key_arguments_are_validated() {
    assert!(invoke(&["key", "set", "-c", "red", "24"], &padctl_fw()).is_err());
    assert!(invoke(&["key", "clear"], &padctl_fw()).is_err());
    assert!(invoke(&["key", "clear", "--all", "3"], &padctl_fw()).is_err());
    assert!(invoke(&["key", "set", "-c", "nope", "1"], &padctl_fw()).is_err());
}

#[test]
fn bootloader_needs_confirmation() {
    let err = invoke(&["bootloader"], &stock()).unwrap_err();
    assert!(err.to_string().contains("--yes"), "{err}");
    assert_eq!(
        invoke(&["bootloader", "--yes"], &stock()).unwrap(),
        "bootloader: jump sent, look for the RPI-RP2 drive\n"
    );
}

#[test]
fn confirmed_bootloader_sends_exactly_the_jump() {
    let cli = Cli::try_parse_from(["padctl", "bootloader"]).unwrap();
    let mut asked = 0;
    let mut out = Vec::new();
    run(&cli, &stock(), &mut out, &mut |_| {
        asked += 1;
        Ok(true)
    })
    .unwrap();
    assert_eq!(asked, 1);
}

#[test]
fn completions_and_man_render() {
    let bash = invoke(&["completions", "bash"], &FakeBackend::default()).unwrap();
    assert!(bash.contains("padctl"));
    let man = invoke(&["man"], &FakeBackend::default()).unwrap();
    assert!(man.contains(".TH padctl"));
}

#[test]
fn version_is_set() {
    let err = Cli::try_parse_from(["padctl", "--version"]).unwrap_err();
    assert!(err.to_string().starts_with("padctl 0.1.0"), "{err}");
}
