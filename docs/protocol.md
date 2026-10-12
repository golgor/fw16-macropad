# padd socket protocol, version 1

Status: design (layer 3). Types: `crates/macropad-ipc`. Architecture: [ADR 0004](adr/0004-layer-3-daemon-and-claims.md).

## Transport

- Unix stream socket `$XDG_RUNTIME_DIR/padd.sock`, mode `0600` (owner only).
- One JSON object per line, UTF-8, `\n` terminated, at most 4096 bytes. A longer or unparsable line gets an `error` and, if unrecoverable, the connection is closed.
- Client messages carry `"op"`, daemon messages carry `"event"`. Unknown fields are ignored (forward compatible); unknown `op`s get `error` `bad_request`.
- Keys are integers `0..23`, row-major from the top left (0 and 1 are the top-left two). Colours are `"#rrggbb"`.
- `req` is a client-chosen integer echoed in the direct reply. Asynchronous events (`press`, `preempted`, `device`) have no `req`.

## Session

The first line must be `hello`. The daemon answers `welcome` or `error` `version` and closes.

```json
{"op":"hello","version":1,"client":"notify-agent"}
{"event":"welcome","version":1,"device":true}
```

`device: false` means padd is running but the macropad is absent (input deck off, bootloader). Claims are still accepted and applied when it returns.

## Requests

### claim

```json
{"op":"claim","req":1,"keys":[0,1],"color":"#ff0000"}
{"event":"claimed","req":1,"claim":17}
```

`mode` is optional and defaults to `"solid"` (the only v1 mode). Errors: `bad_key` (out of range, duplicate, empty), `too_many_claims`.

### release

```json
{"op":"release","req":2,"claim":17}
{"event":"released","req":2,"claim":17}
```

Only the owning connection can release; anything else is `unknown_claim`. Closing the connection releases every claim it holds.

## Events

```json
{"event":"press","claim":17,"key":1}
{"event":"preempted","claim":17,"lost":[1],"remaining":[0],"by":"calendar"}
{"event":"device","present":false}
{"event":"error","req":3,"code":"bad_key","message":"key 24 is out of range"}
```

- `press` is sent once per key-down to the owner of the key's claim only. Key-up and autorepeat are not sent in v1.
- `preempted`: a newer claim took keys. With `remaining: []` the claim is gone; keys do not come back when the newer claim ends.
- `device`: macropad left or returned. Claims persist across this.

## Error codes

`bad_request`, `version`, `bad_key`, `unknown_claim`, `too_many_claims`.

## Examples

### Shell (the CLI does the protocol)

```sh
# Block until key 0 or 1 is pressed; print it. Exit 2 on timeout, 3 if preempted, 4 if padd is down.
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

send({"op": "claim", "req": 1, "keys": [0, 1], "color": "#ff0000"})
claim = json.loads(f.readline())["claim"]

for line in f:                      # blocks; closing the socket releases the claim
    ev = json.loads(line)
    if ev["event"] == "press" and ev["claim"] == claim:
        print("dismissed with key", ev["key"])
        break
    if ev["event"] == "preempted" and not ev["remaining"]:
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
