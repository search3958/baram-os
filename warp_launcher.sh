#!/bin/bash

# This script acts as a launcher for warp apps.
# It can be symlinked as appname.warp -> warp_launcher.sh
# When run as ./appname.warp, it adds appname.warp to .app_files and runs bld.sh

APP=$(basename "$0")
APP_FILES_DB=".app_files"

if [ "$APP" != "warp_launcher.sh" ]; then
    # Ensure the app is in .app_files
    if [ ! -f "$APP_FILES_DB" ] || ! grep -q "^$APP$" "$APP_FILES_DB"; then
        echo "$APP" >> "$APP_FILES_DB"
        echo "  ➕ Added $APP to .app_files"
    fi
    # Execute the build with auto-run trigger
    ./bld.sh r
else
    # If run directly as warp_launcher.sh, sync symlinks
    echo "  🔄 Syncing warp symlinks..."
    for f in ui/*.warp ui/*.warpc; do
        if [ -f "$f" ]; then
            NAME=$(basename "$f")
            if [ ! -L "$NAME" ]; then
                ln -sf warp_launcher.sh "$NAME"
                echo "  🔗 Linked $NAME"
            fi
        fi
    done
    echo "  ✅ Done. You can now run apps using ./appname.warp"
fi
