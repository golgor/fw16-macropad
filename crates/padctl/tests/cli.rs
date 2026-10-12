//! CLI behaviour against the in-memory fake backend.

use clap::Parser;
use macropad_hid::fake::{FakeBackend, FakeDevice};
use padctl::{Cli, run};

fn invoke(args: &[&str], backend: &FakeBackend) -> anyhow::Result<String> {
    // Never talk to a padd that happens to run on the dev machine.
    let pre = ["padctl", "--socket", "/nonexistent/padd.sock"];
    let cli = Cli::try_parse_from(pre.into_iter().chain(args.iter().copied()))?;
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
        "via_protocol: 12\nper_key: v3 (24 keys)\n"
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

mod with_padd {
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    use macropad_input::fake::FakeEvents;
    use macropad_input::{EventSource, KeyAction, KeyEvent};
    use macropad_proto::KeyId;
    use padctl::claim::Exit;
    use padd::{Config, Daemon, Frame, Ignore, LedSink};

    use super::*;

    #[derive(Clone, Default)]
    struct Leds(Arc<Mutex<Frame>>);
    impl LedSink for Leds {
        fn show(&mut self, frame: &Frame) -> Result<(), String> {
            *self.0.lock().unwrap() = *frame;
            Ok(())
        }
    }

    struct Padd {
        dir: tempfile::TempDir,
        keys: std::sync::mpsc::Sender<KeyEvent>,
    }

    fn padd() -> Padd {
        let dir = tempfile::tempdir().unwrap();
        let (keys, events) = FakeEvents::channel();
        let mut events = Some(events);
        let input = Box::new(move || {
            events
                .take()
                .map(|e| Box::new(e) as Box<dyn EventSource>)
                .ok_or(macropad_input::InputError::NotFound)
        });
        Daemon::new(
            Config::new(dir.path().join("padd.sock")),
            Leds::default(),
            Ignore,
        )
        .spawn(input)
        .unwrap();
        Padd { dir, keys }
    }

    impl Padd {
        fn run(&self, args: &[&str]) -> anyhow::Result<String> {
            let sock = self.dir.path().join("padd.sock");
            let pre = ["padctl", "--socket", sock.to_str().unwrap()];
            let cli = Cli::try_parse_from(pre.into_iter().chain(args.iter().copied()))?;
            let mut out = Vec::new();
            run(&cli, &stock(), &mut out, &mut |_| Ok(false))?;
            Ok(String::from_utf8(out)?)
        }

        fn press_soon(&self, key: u8) {
            let keys = self.keys.clone();
            std::thread::spawn(move || {
                std::thread::sleep(Duration::from_millis(200));
                let key = KeyId::new(key).unwrap();
                keys.send(KeyEvent {
                    key,
                    action: KeyAction::Pressed,
                })
                .unwrap();
            });
        }
    }

    fn code(e: &anyhow::Error) -> Option<u8> {
        e.downcast_ref::<Exit>().map(|e| e.code)
    }

    #[test]
    fn claim_once_prints_the_pressed_key() {
        let p = padd();
        p.press_soon(1);
        assert_eq!(
            p.run(&["claim", "-c", "red", "0", "1", "--once", "--timeout", "10s"])
                .unwrap(),
            "1\n"
        );
    }

    #[test]
    fn claim_timeout_exits_2() {
        let p = padd();
        let e = p
            .run(&["claim", "-c", "red", "0", "--once", "--timeout", "200ms"])
            .unwrap_err();
        assert_eq!(code(&e), Some(2), "{e}");
    }

    #[test]
    fn claim_without_padd_exits_4() {
        let e = invoke(&["claim", "-c", "red", "0", "--once"], &stock()).unwrap_err();
        assert_eq!(code(&e), Some(4), "{e}");
        let e = invoke(&["status"], &stock()).unwrap_err();
        assert_eq!(code(&e), Some(4), "{e}");
    }

    #[test]
    fn streaming_claim_prints_json_lines_until_expired() {
        let p = padd();
        p.press_soon(0);
        let e = p
            .run(&["claim", "-c", "#00ff00", "0", "--timeout", "1s"])
            .unwrap_err();
        assert_eq!(code(&e), Some(2), "{e}");
    }

    #[test]
    fn pause_resume_status() {
        let p = padd();
        p.run(&["pause"]).unwrap();
        assert!(p.run(&["status"]).unwrap().contains("(paused)"));
        p.run(&["resume"]).unwrap();
        let json = p.run(&["status", "--json"]).unwrap();
        assert!(json.contains("\"paused\":false"), "{json}");
    }

    #[test]
    fn raw_led_commands_refuse_while_padd_runs() {
        let p = padd();
        let e = p.run(&["light", "off"]).unwrap_err();
        assert!(e.to_string().contains("padd is running"), "{e}");
        assert!(p.run(&["key", "clear", "--all"]).is_err());
        p.run(&["light", "off", "--force"]).unwrap();
        // Stock fake has no per-key support, but the refusal is gone.
        let e = p.run(&["key", "--force", "clear", "--all"]).unwrap_err();
        assert!(!e.to_string().contains("padd is running"), "{e}");
    }
}
