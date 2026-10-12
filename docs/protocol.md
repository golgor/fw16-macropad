# padd socket protocol, version 1

Status: implemented (layer 3). Every JSON line below is checked byte for byte by `doc_examples_round_trip` in `crates/macropad-ipc`. Architecture: [ADR 0004](adr/0004-layer-3-daemon-and-claims.md).

## Transport

- Unix stream socket `$XDG_RUNTIME_DIR/padd.sock`, mode `0600` (owner only).
- One JSON object per line, UTF-8, `\n` terminated, at most 4096 bytes. A longer or unparsable line gets an `error` and, if unrecoverable, the connection is closed.
- Client messages carry `"op"`, daemon messages carry `"event"`. Unknown fields are ignored (forward compatible); unknown `op`s get `error` `bad_request`.
- Keys are integers `0..23`, row-major from the top left (0 and 1 are the top-left two). Colours are `"#rrggbb"`.
- `req` is a client-chosen integer echoed in the direct reply. Asynchronous events (`press`, `preempted`, `expired`, `pause`, `device`) have no `req`.

## Session

The first line must be `hello`. The daemon answers `welcome` or `error` `version` and closes.

```json
{"op":"hello","version":1,"client":"notify-agent"}
{"event":"welcome","version":1,"device":true,"paused":false}
```

`device: false` means padd is running but the macropad is absent (input deck off, bootloader). Claims are still accepted and applied when it returns. `paused` is the current pause state (see below).

## Requests

### claim

```json
{"op":"claim","req":1,"keys":[0,1],"color":"#ff0000","mode":"solid"}
{"event":"claimed","req":1,"claim":17}
{"op":"claim","req":2,"keys":[23],"color":"#00ff00","mode":"solid","timeout_ms":600000}
```

- `mode` is optional on input and defaults to `"solid"`, the only v1 mode. padd and the client always write it. Blink and pulse will come later, with the timing done on the host.
- `timeout_ms` is optional. When it is set, padd ends the claim after that many milliseconds and sends `expired` to the owner.
- Errors: `bad_key` (empty or duplicate list), `too_many_claims` (32 per connection). A key outside `0..23` or a malformed colour fails to parse, so it gets `bad_request`.

### release

```json
{"op":"release","req":2,"claim":17}
{"event":"released","req":2,"claim":17}
```

Only the owning connection can release; anything else is `unknown_claim`. Closing the connection releases every claim it holds.

### pause / resume

```json
{"op":"pause","req":4}
{"event":"ok","req":4}
{"op":"resume","req":5}
```

Pausing keeps every claim, turns the LEDs off and drops presses, which reach neither the owners nor the desktop. Resume repaints the claims. Every client gets a `pause` event when the state actually changes. Timeouts keep running while paused. Pausing twice is not an error.

### status

```json
{"op":"status","req":6}
```

The reply is `status` with `daemon` (version string), `device` and `device_error`, `paused`, `clients`, and `claims`. `claims` is a list of `{claim, client, keys, color, expires_in_ms}`, oldest first. `padctl status --json` prints this object.

## Events

```json
{"event":"press","claim":17,"key":1}
{"event":"preempted","claim":17,"lost":[1],"remaining":[0],"by":"calendar"}
{"event":"expired","claim":18}
{"event":"pause","paused":true}
{"event":"device","present":false}
{"event":"error","req":7,"code":"bad_key","message":"duplicate key 3"}
```

- `press` is sent once per key-down to the owner of the key's claim only. Key-up and autorepeat are not sent in v1.
- `preempted`: a newer claim took keys. With `remaining: []` the claim is gone; keys do not come back when the newer claim ends.
- `expired`: the claim's `timeout_ms` ran out and the claim is gone.
- `pause`: another client (or this one) paused or resumed padd.
- `device`: macropad left or returned. Claims persist across this.
- If padd restarts, the socket closes. Clients see EOF and must reconnect and claim again; nothing re-claims automatically.

## Error codes

`bad_request`, `version`, `bad_key`, `unknown_claim`, `too_many_claims`.

## Examples

### Shell (the CLI does the protocol)

```sh
# Block until key 0 or 1 is pressed; print it.
# Exit 2 on timeout/expired, 3 if preempted, 4 if padd is down or went away.
key=$(padctl claim -c red 0 1 --once --timeout 10m) && echo "pressed $key"

# Stream events as JSON lines until killed (claim held meanwhile).
padctl claim -c '#00ff00' 23 | while read -r ev; do echo "$ev"; done
```

### Python, direct socket (no dependencies)

```python
import json, os, socket

sock = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
sock.connect(os.path.join(os.environ["XDG_RUNTIME_DIR"], "padd.sock"))
f = sock.makefile("rw", encoding="utf-8", newline="\n")

def send(msg):
    f.write(json.dumps(msg) + "\n"); f.flush()

send({"op": "hello", "version": 1, "client": "build-watcher"})
assert json.loads(f.readline())["event"] == "welcome"

send({"op": "claim", "req": 1, "keys": [0, 1], "color": "#ff0000", "timeout_ms": 600000})
claim = json.loads(f.readline())["claim"]

for line in f:                      # blocks; closing the socket releases the claim
    ev = json.loads(line)
    if ev["event"] == "press" and ev["claim"] == claim:
        print("dismissed with key", ev["key"])
        break
    if ev["event"] == "expired" or (ev["event"] == "preempted" and not ev["remaining"]):
        break
sock.close()
```

### Python via padctl (simplest for agents)

```python
import subprocess
r = subprocess.run(["padctl", "claim", "-c", "red", "0", "1", "--once", "--timeout", "10m"],
                   capture_output=True, text=True)
if r.returncode == 0:
    dismiss_notification(key=int(r.stdout))
```
