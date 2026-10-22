#!/bin/sh
# Launch the Tauri dev app (React + BlockNote UI, Rust vault backend).
cd "$(dirname "$0")/.."
exec npm run tauri dev
