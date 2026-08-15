#!/usr/bin/env bash
set -euo pipefail

files_source="${1:?files source directory is required}"
archive_output="${2:?files.tar output path is required}"

[ -d "$files_source/app" ] || { echo "missing $files_source/app" >&2; exit 1; }
[ -d "$files_source/data" ] || { echo "missing $files_source/data" >&2; exit 1; }

archive_dir="$(dirname "$archive_output")"
mkdir -p "$archive_dir"
stage_dir="$(mktemp -d "${TMPDIR:-/tmp}/baramos-files.XXXXXX")"
trap 'rm -rf "$stage_dir"' EXIT

mkdir -p "$stage_dir/app" "$stage_dir/data"
cp -R "$files_source/app/." "$stage_dir/app/"
cp -R "$files_source/data/." "$stage_dir/data/"

# The runtime opens application packages as single files. Package each source
# directory first, then put those package files into the common user archive.
for app_directory in "$stage_dir/app"/*.w3a "$stage_dir/app"/*.w4a "$stage_dir/app"/*.s4a; do
    [ -d "$app_directory" ] || continue
    archive_name="$(basename "$app_directory")"
    tar --format=ustar -cf "$stage_dir/$archive_name" -C "$app_directory" .
    rm -rf "$app_directory"
    mv "$stage_dir/$archive_name" "$stage_dir/app/$archive_name"
done

rm -f "$archive_output"
tar --format=ustar -cf "$archive_output" -C "$stage_dir" app data
