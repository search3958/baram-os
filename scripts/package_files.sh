#!/usr/bin/env bash
set -euo pipefail

files_source="${1:?files source directory is required}"
tree_output="${2:?output directory is required}"
profile="${3:-normal}"

[ -d "$files_source/app" ] || { echo "missing $files_source/app" >&2; exit 1; }
[ -d "$files_source/data" ] || { echo "missing $files_source/data" >&2; exit 1; }

stage_dir="$(mktemp -d "${TMPDIR:-/tmp}/baramos-files.XXXXXX")"
trap 'rm -rf "$stage_dir"' EXIT

mkdir -p "$stage_dir/app" "$stage_dir/data"
cp -R "$files_source/app/." "$stage_dir/app/"
cp -R "$files_source/data/." "$stage_dir/data/"

# `.baram-ignore` is a deliberately small YAML subset: under `xiao:`, each
# `- path` entry is relative to the files directory. A trailing slash ignores
# the whole directory below that path.
if [ "$profile" = "xiao" ] && [ -f "$files_source/.baram-ignore" ]; then
    xiao_ignores="$(awk '
        /^[[:space:]]*xiao:[[:space:]]*$/ { in_xiao=1; next }
        /^[^[:space:]][^:]*:[[:space:]]*$/ { in_xiao=0; next }
        in_xiao && /^[[:space:]]*-[[:space:]]*/ {
            sub(/^[[:space:]]*-[[:space:]]*/, "")
            sub(/[[:space:]]+#.*$/, "")
            if ($0 != "") print
        }
    ' "$files_source/.baram-ignore")"

    while IFS= read -r ignored_path; do
        [ -n "$ignored_path" ] || continue
        case "$ignored_path" in
            /*|*..*|*' '*|*'\t'*)
                echo "invalid xiao ignore path: $ignored_path" >&2
                exit 1
                ;;
        esac
        ignored_path="${ignored_path%/}"
        [ -n "$ignored_path" ] || continue
        rm -rf "$stage_dir/$ignored_path"
    done <<< "$xiao_ignores"
fi

# The runtime opens application packages as single files. Package each source
# directory first, while keeping the overall image as ordinary FAT files.
for app_directory in "$stage_dir/app"/*.w3a "$stage_dir/app"/*.w4a "$stage_dir/app"/*.s4a; do
    [ -d "$app_directory" ] || continue
    archive_name="$(basename "$app_directory")"
    tar --format=ustar -cf "$stage_dir/$archive_name" -C "$app_directory" .
    rm -rf "$app_directory"
    mv "$stage_dir/$archive_name" "$stage_dir/app/$archive_name"
done

rm -rf "$tree_output"
mkdir -p "$(dirname "$tree_output")"
cp -R "$stage_dir" "$tree_output"
