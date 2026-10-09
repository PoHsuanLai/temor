#!/usr/bin/env bash
# Checks temor's layer rules (PLAN §2, D23) from `cargo metadata --no-deps`:
#   - an L0 crate depends only on L0 workspace crates;
#   - an L1 crate depends only on L0 or L1 workspace crates;
#   - no L0 or L1 crate depends on ds-*, dioxus, docket or zbus;
#   - every workspace crate is in the table below (a new crate must be placed in a layer).
# Dev-dependencies count too, except a crate's own dev-dep on itself (its `testing` feature).
# Third-party crates are not restricted by layer. Prints each violation as
# `crate -> dep (rule)` and exits 1 when there is any.
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"

# The layer table. Keep it in step with PLAN §2.
LAYERS='
temor-vt-sys 0
temor-vt 0
temor-pty 0
temor-shell 0
temor-wire 0
temor-transfer 0
temor-ssh 0
temor-render 1
temor-pane 1
temor-core 2
temor-ui 3
temor-desktop 3
temor-remote 3
temor 3
'

metadata=$(cd "$root" && cargo metadata --format-version 1 --no-deps)

printf '%s\n' "$LAYERS" | python3 -I -c '
import json, re, sys

layers = {}
for line in sys.stdin.read().split("\n"):
    parts = line.split()
    if len(parts) == 2:
        layers[parts[0]] = int(parts[1])

banned = re.compile(r"^(ds-.*|dioxus.*|docket.*|zbus.*)$")
meta = json.loads(sys.argv[1])
violations = []
seen = set()

for pkg in meta["packages"]:
    name = pkg["name"]
    if not name.startswith("temor"):
        continue
    if name not in layers:
        violations.append(f"{name} -> (not in the layer table) (table)")
        continue
    seen.add(name)
    layer = layers[name]
    for dep in pkg["dependencies"]:
        dname = dep["name"]
        if dname == name:
            continue
        if layer <= 1 and banned.match(dname):
            violations.append(f"{name} -> {dname} (L{layer} never depends on quire, dioxus, docket or zbus)")
            continue
        if dname in layers and layers[dname] > layer:
            violations.append(f"{name} -> {dname} (L{layer} depends on L{layers[dname]}: dependencies point down only)")

for v in violations:
    print(v)
sys.exit(1 if violations else 0)
' "$metadata"
status=$?
if [ "$status" -eq 0 ]; then
  echo "check-layers: ok"
fi
exit "$status"
