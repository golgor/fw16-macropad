//! End to end: real socket, real client, fake LEDs and fake input.

use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use macropad_client::{ClaimSpec, Client, ClientError, ServerMsg, Wait};
use macropad_input::fake::FakeEvents;
use macropad_input::{EventSource, KeyAction, KeyEvent};
use macropad_proto::{KeyId, Rgb};
use padd::{Config, Daemon, Frame, Handle, Ignore, LedSink};

const RED: Rgb = Rgb { r: 255, g: 0, b: 0 };
const BLUE: Rgb = Rgb { r: 0, g: 0, b: 255 };
const WAIT: Option<Duration> = Some(Duration::from_secs(5));

/// Fake LEDs that behave like `HidLeds`: a diff cache over a device whose
/// state can change behind padd's back (`wipe`) or fail (`fail`).
#[derive(Clone, Default)]
struct Leds(Arc<Mutex<LedState>>);

#[derive(Default)]
struct LedState {
    device: Frame,
    cache: Option<Frame>,
    fail: bool,
    invalidations: usize,
}

impl LedSink for Leds {
    fn show(&mut self, frame: &Frame) -> Result<(), String> {
        let mut s = self.0.lock().unwrap();
        if s.fail {
            s.cache = None; // like HidLeds dropping the handle
            return Err("reply header does not echo request".into());
        }
        match s.cache {
            None => s.device = *frame,
            Some(cache) => {
                for i in 0..24 {
                    if frame.0[i] != cache.0[i] {
                        s.device.0[i] = frame.0[i];
                    }
                }
            }
        }
        s.cache = Some(*frame);
        Ok(())
    }

    fn invalidate(&mut self) {
        let mut s = self.0.lock().unwrap();
        s.cache = None;
        s.invalidations += 1;
    }
}

impl Leds {
    fn get(&self, key: u8) -> Option<Rgb> {
        self.0.lock().unwrap().device.get(k(key))
    }
    /// Another writer cleared the device; padd's cache does not know.
    fn wipe(&self) {
        self.0.lock().unwrap().device = Frame::default();
    }
    fn set_fail(&self, fail: bool) {
        self.0.lock().unwrap().fail = fail;
    }
    fn invalidations(&self) -> usize {
        self.0.lock().unwrap().invalidations
    }
}

struct Harness {
    _dir: tempfile::TempDir,
    socket: std::path::PathBuf,
    leds: Leds,
    keys: Sender<KeyEvent>,
    handle: Handle,
}

fn k(i: u8) -> KeyId {
    KeyId::new(i).unwrap()
}

fn start() -> Harness {
    let dir = tempfile::tempdir().unwrap();
    let socket = dir.path().join("padd.sock");
    let leds = Leds::default();
    let (keys, events) = FakeEvents::channel();
    let mut events = Some(events);
    let input = Box::new(move || {
        events
            .take()
            .map(|e| Box::new(e) as Box<dyn EventSource>)
            .ok_or(macropad_input::InputError::NotFound)
    });
    let (handle, _main) = Daemon::new(Config::new(socket.clone()), leds.clone(), Ignore)
        .spawn(input)
        .unwrap();
    Harness {
        _dir: dir,
        socket,
        leds,
        keys,
        handle,
    }
}

impl Harness {
    fn client(&self, name: &str) -> Client {
        Client::connect(&self.socket, name).unwrap()
    }

    fn press(&self, key: u8) {
        for action in [KeyAction::Pressed, KeyAction::Released] {
            self.keys
                .send(KeyEvent {
                    key: k(key),
                    action,
                })
                .unwrap();
        }
    }

    /// Wait until the LED state matches (the daemon paints asynchronously).
    fn wait_leds(&self, key: u8, want: Option<Rgb>) {
        for _ in 0..200 {
            if self.leds.get(key) == want {
                return;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        panic!("key {key}: wanted {want:?}, have {:?}", self.leds.get(key));
    }
}

#[test]
fn claim_press_release_round_trip() {
    let h = start();
    let mut c = h.client("agent");
    let claim = c.claim(&ClaimSpec::new(vec![k(0), k(1)], RED)).unwrap();
    h.wait_leds(0, Some(RED));
    h.wait_leds(2, None);
    h.press(1);
    assert_eq!(c.wait_press(claim, WAIT).unwrap(), Wait::Pressed(k(1)));
    c.release(claim).unwrap();
    h.wait_leds(0, None);
}

#[test]
fn presses_go_only_to_the_owner_and_unclaimed_do_nothing() {
    let h = start();
    let mut a = h.client("a");
    let mut b = h.client("b");
    let ca = a.claim(&ClaimSpec::new(vec![k(0)], RED)).unwrap();
    let cb = b.claim(&ClaimSpec::new(vec![k(5)], BLUE)).unwrap();
    h.press(7); // unclaimed: nobody hears it
    h.press(5);
    assert_eq!(b.wait_press(cb, WAIT).unwrap(), Wait::Pressed(k(5)));
    assert_eq!(
        a.wait_press(ca, Some(Duration::from_millis(200))).unwrap(),
        Wait::TimedOut
    );
}

#[test]
fn newest_claim_preempts_and_notifies() {
    let h = start();
    let mut a = h.client("a");
    let mut b = h.client("calendar");
    let ca = a.claim(&ClaimSpec::new(vec![k(0)], RED)).unwrap();
    b.claim(&ClaimSpec::new(vec![k(0)], BLUE)).unwrap();
    assert_eq!(
        a.wait_press(ca, WAIT).unwrap(),
        Wait::Preempted {
            by: "calendar".into()
        }
    );
    h.wait_leds(0, Some(BLUE));
}

#[test]
fn daemon_timeout_expires_claim() {
    let h = start();
    let mut c = h.client("agent");
    let claim = c
        .claim(&ClaimSpec::new(vec![k(3)], RED).with_timeout(Some(Duration::from_millis(100))))
        .unwrap();
    h.wait_leds(3, Some(RED));
    assert_eq!(c.wait_press(claim, WAIT).unwrap(), Wait::Expired);
    h.wait_leds(3, None);
}

#[test]
fn disconnect_releases_claims() {
    let h = start();
    let mut c = h.client("crashy");
    c.claim(&ClaimSpec::new(vec![k(9)], RED)).unwrap();
    h.wait_leds(9, Some(RED));
    drop(c);
    h.wait_leds(9, None);
}

#[test]
fn pause_hides_lights_swallows_presses_and_keeps_claims() {
    let h = start();
    let mut c = h.client("agent");
    let mut ctl = h.client("padctl");
    let claim = c.claim(&ClaimSpec::new(vec![k(0)], RED)).unwrap();
    h.wait_leds(0, Some(RED));
    ctl.pause().unwrap();
    h.wait_leds(0, None);
    assert!(ctl.status().unwrap().paused);
    h.press(0);
    // The owner hears about the pause, not the press.
    assert_eq!(
        c.next_event(WAIT).unwrap(),
        Some(ServerMsg::Pause { paused: true })
    );
    assert_eq!(
        c.wait_press(claim, Some(Duration::from_millis(200)))
            .unwrap(),
        Wait::TimedOut
    );
    ctl.resume().unwrap();
    h.wait_leds(0, Some(RED));
    h.press(0);
    assert_eq!(c.wait_press(claim, WAIT).unwrap(), Wait::Pressed(k(0)));
    assert_eq!(ctl.status().unwrap().claims.len(), 1);
}

#[test]
fn status_lists_claims() {
    let h = start();
    let mut c = h.client("agent");
    c.claim(&ClaimSpec::new(vec![k(1), k(0)], RED).with_timeout(Some(Duration::from_secs(60))))
        .unwrap();
    let s = c.status().unwrap();
    assert!(s.device && !s.paused);
    assert_eq!(s.clients, 1);
    assert_eq!(s.claims[0].client, "agent");
    assert_eq!(
        s.claims[0]
            .keys
            .iter()
            .map(|k| k.0.index())
            .collect::<Vec<_>>(),
        [0, 1]
    );
    assert!(s.claims[0].expires_in_ms.is_some_and(|ms| ms <= 60_000));
}

#[test]
fn bad_requests_get_errors_not_disconnects() {
    let h = start();
    let mut c = h.client("agent");
    let err = c.claim(&ClaimSpec::new(vec![k(1), k(1)], RED)).unwrap_err();
    assert!(err.to_string().contains("duplicate key 1"), "{err}");
    assert!(c.claim(&ClaimSpec::new(vec![], RED)).is_err());
    assert!(c.release(macropad_client::ClaimId(999)).is_err());
    c.claim(&ClaimSpec::new(vec![k(2)], RED)).unwrap();
}

#[test]
fn second_daemon_refuses_and_shutdown_cleans_up() {
    let h = start();
    let again = Daemon::new(Config::new(h.socket.clone()), Leds::default(), Ignore)
        .spawn(Box::new(|| Err(macropad_input::InputError::NotFound)));
    assert!(matches!(again, Err(padd::DaemonError::AlreadyRunning(_))));
    let mut c = h.client("agent");
    c.claim(&ClaimSpec::new(vec![k(4)], RED)).unwrap();
    h.handle.shutdown();
    assert!(matches!(
        c.next_event(WAIT),
        Err(ClientError::Disconnected) | Ok(None)
    ));
    h.wait_leds(4, None);
    for _ in 0..200 {
        if !h.socket.exists() {
            return;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    panic!("socket not removed");
}

#[test]
fn missing_daemon_is_reported() {
    let dir = tempfile::tempdir().unwrap();
    assert!(matches!(
        Client::connect(&dir.path().join("padd.sock"), "x"),
        Err(ClientError::NoDaemon(_))
    ));
}

/// Regression (hardware, PR #3): after pause/resume the claim must be lit
/// again even if the device lost its state while padd thought it was lit.
#[test]
fn resume_repaints_the_full_frame() {
    let h = start();
    let mut c = h.client("agent");
    let mut ctl = h.client("padctl");
    c.claim(&ClaimSpec::new(vec![k(0), k(5)], RED)).unwrap();
    h.wait_leds(0, Some(RED));
    h.leds.wipe();
    ctl.pause().unwrap();
    h.wait_leds(0, None);
    h.leds.wipe();
    ctl.resume().unwrap();
    h.wait_leds(0, Some(RED));
    h.wait_leds(5, Some(RED));
    assert_eq!(h.leds.invalidations(), 1);
}

/// LED failure (e.g. interleaved replies from `padctl --force`): padd marks
/// the device unavailable, retries after the backoff and repaints everything.
#[test]
fn led_recovery_repaints_the_full_frame() {
    let h = start();
    let mut c = h.client("agent");
    c.claim(&ClaimSpec::new(vec![k(2)], RED)).unwrap();
    h.wait_leds(2, Some(RED));
    h.leds.set_fail(true);
    h.leds.wipe();
    // A change while failing: padd notices and goes unavailable.
    let second = c.claim(&ClaimSpec::new(vec![k(3)], RED)).unwrap();
    assert_eq!(
        c.next_event(WAIT).unwrap(),
        Some(ServerMsg::Device { present: false })
    );
    h.leds.set_fail(false);
    assert_eq!(
        c.next_event(WAIT).unwrap(),
        Some(ServerMsg::Device { present: true })
    );
    h.wait_leds(2, Some(RED));
    h.wait_leds(3, Some(RED));
    c.release(second).unwrap();
    h.wait_leds(3, None);
}
