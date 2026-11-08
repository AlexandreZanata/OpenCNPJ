#!/bin/sh
# Compiles src/**/*.ts to dist/js and copies the static assets (html, css, fonts).
set -eu
cd "$(dirname "$0")/.."

rm -rf dist
sh scripts/tsc.sh
cp -R public/. dist/
echo "built: $(pwd)/dist"
