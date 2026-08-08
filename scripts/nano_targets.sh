#!/usr/bin/env bash

# Cargo metadata is the source of truth for Nano System executables. A target
# is eligible when it belongs to baram-nano-system or its package declares a
# direct dependency on baram-nano-system.
_nano_target_rows() {
    cargo metadata --no-deps --format-version 1 | python3 -c '
import json, sys
metadata = json.load(sys.stdin)
for package in metadata["packages"]:
    linked = package["name"] == "baram-nano-system" or any(
        dependency["name"] == "baram-nano-system"
        for dependency in package.get("dependencies", [])
    )
    if not linked:
        continue
    for target in package.get("targets", []):
        if "bin" in target.get("kind", []):
            print("{}\t{}".format(target["name"], target["src_path"]))
'
}

nano_primary_bin() {
    local rows
    rows="$(_nano_target_rows)"
    if printf '%s\n' "$rows" | cut -f1 | grep -qx 'bootaa64'; then
        printf '%s\n' 'bootaa64'
    else
        printf '%s\n' "$rows" | cut -f1 | grep -m1 -x 'baram-nano-system'
    fi
}

nano_app_bins() {
    _nano_target_rows | awk -F '\t' '$2 ~ /\/src\/bin\// { print $1 }'
}

nano_all_bins() {
    _nano_target_rows | cut -f1
}
