#!/usr/bin/env bash
# Checks temor's layer rules (PLAN §2, D23) from `cargo metadata --no-deps`:
#   - an L0 crate depends only on L0 workspace crates;
#   - an L1 crate depends only on L0 or L1 workspace crates;
#   - no L0 or L1 crate depends on ds-*, dioxus, docket, zbus or tokio;
#   - every workspace crate is in the table below (a new crate must be placed in a layer);
#   - the quire-desktop boundary (PLAN §2, D15): only temor-desktop names zbus, docket or porter
#     crates, and there each such dependency is optional and listed in its `quire-desktop` feature,
#     so `--no-default-features` builds with none of them.
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
temor-testkit 0
temor-schema 0
temor-store 1
temor-render 1
temor-pane 1
temor-plugins 1
temor-core 2
temor-client 2
temor-ui 3
temor-desktop 3
temor-remote 3
temor 3
'

metadata=$(cd "$root" && cargo metadata --format-version 1 --no-deps)

if printf '%s\n' "$LAYERS" | python3 -I -c '
import json, re, sys

layers = {}
for line in sys.stdin.read().split("\n"):
    parts = line.split()
    if len(parts) == 2:
        layers[parts[0]] = int(parts[1])

banned = re.compile(r"^(ds-.*|dioxus.*|docket.*|zbus.*|tokio.*)$")
desktop_only = re.compile(r"^(docket.*|zbus.*|porter.*|prov)$")
DESKTOP = "temor-desktop"
FEATURE = "quire-desktop"
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
    gated = set()
    if name == DESKTOP:
        gated = {
            item[len("dep:"):]
            for item in pkg["features"].get(FEATURE, [])
            if item.startswith("dep:")
        }
    for dep in pkg["dependencies"]:
        dname = dep["name"]
        if dname == name:
            continue
        if desktop_only.match(dname) and dep.get("kind") != "dev":
            if name != DESKTOP:
                violations.append(f"{name} -> {dname} (only {DESKTOP} names zbus, docket or porter)")
            elif not dep.get("optional") or dname not in gated:
                violations.append(f"{name} -> {dname} (must be optional and in feature {FEATURE})")
        if layer <= 1 and banned.match(dname):
            violations.append(f"{name} -> {dname} (L{layer} never depends on quire, dioxus, docket or zbus)")
            continue
        if dname in layers and layers[dname] > layer:
            violations.append(f"{name} -> {dname} (L{layer} depends on L{layers[dname]}: dependencies point down only)")

for v in violations:
    print(v)
sys.exit(1 if violations else 0)
' "$metadata"; then
  echo "check-layers: ok"
  exit 0
fi
exit 1
