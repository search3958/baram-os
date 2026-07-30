#!/usr/bin/env bash
set -euo pipefail

app_source="${1:?app source directory is required}"
archive_output="${2:?archive output directory is required}"

mkdir -p "$archive_output"
find "$archive_output" -maxdepth 1 -type f -name '*.w3a' -delete

for app_directory in "$app_source"/*.w3a; do
    [ -d "$app_directory" ] || continue
    archive_name="$(basename "$app_directory")"
    # USTAR is deliberately used instead of platform-specific extended TAR
    # records, keeping the no_std runtime reader small and deterministic.
    tar --format=ustar -cf "$archive_output/$archive_name" -C "$app_directory" .
done
