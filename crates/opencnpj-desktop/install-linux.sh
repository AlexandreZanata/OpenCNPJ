#!/bin/sh
# Installs OpenCNPJ Admin for the current user from a release build:
#   binary        -> ~/.local/bin/opencnpj-admin
#   icon          -> ~/.local/share/icons/hicolor/scalable/apps/opencnpj-admin.svg
#   desktop entry -> ~/.local/share/applications/opencnpj-admin.desktop
#
# Usage (from anywhere in the repository):
#   cargo build --release -p opencnpj-desktop
#   crates/opencnpj-desktop/install-linux.sh            # install
#   crates/opencnpj-desktop/install-linux.sh --uninstall
#
# Environment: BIN (path to the binary; default: <workspace>/target/release/opencnpj-admin),
# PREFIX (default: ~/.local).
set -eu

here=$(cd "$(dirname "$0")" && pwd)
root=$(cd "$here/../.." && pwd)
prefix=${PREFIX:-"$HOME/.local"}
bin=${BIN:-"${CARGO_TARGET_DIR:-$root/target}/release/opencnpj-admin"}
bin_dir="$prefix/bin"
icon_dir="$prefix/share/icons/hicolor/scalable/apps"
app_dir="$prefix/share/applications"

refresh_caches() {
    if command -v update-desktop-database >/dev/null 2>&1; then
        update-desktop-database "$app_dir" >/dev/null 2>&1 || true
    fi
    if command -v gtk-update-icon-cache >/dev/null 2>&1 && [ -f "$prefix/share/icons/hicolor/index.theme" ]; then
        gtk-update-icon-cache -q -t "$prefix/share/icons/hicolor" >/dev/null 2>&1 || true
    fi
}

if [ "${1:-}" = "--uninstall" ]; then
    rm -f "$bin_dir/opencnpj-admin" "$icon_dir/opencnpj-admin.svg" "$app_dir/opencnpj-admin.desktop"
    refresh_caches
    echo "Removed OpenCNPJ Admin from $prefix (preferences in ${XDG_CONFIG_HOME:-$HOME/.config}/opencnpj-admin are kept)."
    exit 0
fi

if [ ! -x "$bin" ]; then
    echo "error: $bin not found; run 'cargo build --release -p opencnpj-desktop' first (or set BIN=...)" >&2
    exit 1
fi

mkdir -p "$bin_dir" "$icon_dir" "$app_dir"
install -m 0755 "$bin" "$bin_dir/opencnpj-admin"
# Release builds keep line tables for backtraces; the installed copy does not need them.
if command -v strip >/dev/null 2>&1; then
    strip "$bin_dir/opencnpj-admin" 2>/dev/null || true
fi
install -m 0644 "$here/assets/opencnpj-admin.svg" "$icon_dir/opencnpj-admin.svg"
# Absolute Exec path: ~/.local/bin is not on every desktop session's PATH.
sed "s|^Exec=opencnpj-admin$|Exec=\"$bin_dir/opencnpj-admin\"|" "$here/assets/opencnpj-admin.desktop" >"$app_dir/opencnpj-admin.desktop"
chmod 0644 "$app_dir/opencnpj-admin.desktop"
refresh_caches

echo "Installed:"
echo "  $bin_dir/opencnpj-admin"
echo "  $icon_dir/opencnpj-admin.svg"
echo "  $app_dir/opencnpj-admin.desktop"
case ":$PATH:" in
    *":$bin_dir:"*) ;;
    *) echo "note: $bin_dir is not on PATH; the menu entry works anyway (it uses the absolute path)." ;;
esac
