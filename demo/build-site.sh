#!/usr/bin/env bash
# Builds the static demo into $1 (default: site/): the board page in demo mode,
# archboard's plan checking compiled to wasm, and a snapshot per repository in
# demo/repos.json. Needs the wasm32-unknown-unknown target, the wasm-bindgen
# CLI at the version Cargo.toml pins, git and python3.
set -euo pipefail
cd "$(dirname "$0")/.."
out="${1:-site}"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
rm -rf "$out"; mkdir -p "$out/snapshots"

cargo build --locked --release
cargo build --locked --release --lib --target wasm32-unknown-unknown
wasm-bindgen --target web --no-typescript --out-dir "$out/pkg" target/wasm32-unknown-unknown/release/archboard.wasm

python3 - "$out" "$work" <<'PY'
import json, subprocess, sys
out, work = sys.argv[1], sys.argv[2]
repos = json.load(open("demo/repos.json"))
listed = []
for r in repos:
    if r["ref"]:
        path = f"{work}/{r['id']}"
        subprocess.run(["git", "init", "-q", path], check=True)
        subprocess.run(["git", "-C", path, "fetch", "-q", "--depth", "1", r["url"], r["ref"]], check=True)
        subprocess.run(["git", "-C", path, "checkout", "-q", "FETCH_HEAD"], check=True)
        source = f"{r['url']}/tree/{r['ref']}"
    else:
        path = "."
        sha = subprocess.run(["git", "rev-parse", "HEAD"], capture_output=True, text=True, check=True).stdout.strip()
        source = f"{r['url']}/tree/{sha}"
    cmd = ["target/release/archboard", "--repo", path, "export", "--out", f"{out}/snapshots/{r['id']}.json", "--name", r["id"]]
    if r["config"]:
        cmd += ["--config", r["config"]]
    subprocess.run(cmd, check=True)
    listed.append({"id": r["id"], "name": r["name"], "source": source, "note": r["note"]})
page = open("web/index.html").read()
marker = "/*DEMO*/null"
assert marker in page
page = page.replace(marker, json.dumps({"repos": listed}).replace("</", "<\\/"), 1)
open(f"{out}/index.html", "w").write(page)
PY
touch "$out/.nojekyll"
echo "built $out"
