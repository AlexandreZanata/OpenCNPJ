#!/bin/sh
# Runs the TypeScript 7 compiler pinned to 7.0.2.
# The compiler is a build tool only: nothing from npm ships in `dist/`, and this
# project declares no dependencies. A global/local `tsc` is used when it is 7.0.2,
# otherwise the exact version is fetched on demand by npx.
set -eu
cd "$(dirname "$0")/.."

TS_VERSION="7.0.2"

if command -v tsc >/dev/null 2>&1 && [ "$(tsc -v | awk '{print $2}')" = "$TS_VERSION" ]; then
  exec tsc -p tsconfig.json "$@"
fi
exec npx --yes --package="typescript@${TS_VERSION}" tsc -p tsconfig.json "$@"
