#!/usr/bin/env bash
set -euo pipefail

app_name="zapreq"
repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
desktop_src="$repo_root/assets/linux/zapreq.desktop"
icon_src="$repo_root/assets/linux/zapreq.svg"

bin_path="${1:-$HOME/.cargo/bin/zapreq}"
desktop_dir="${XDG_DATA_HOME:-$HOME/.local/share}/applications"
icon_dir="${XDG_DATA_HOME:-$HOME/.local/share}/icons/hicolor/scalable/apps"
desktop_target="$desktop_dir/zapreq.desktop"
icon_target="$icon_dir/zapreq.svg"

if [[ ! -x "$bin_path" ]]; then
  echo "error: binary not found or not executable: $bin_path" >&2
  exit 1
fi

mkdir -p "$desktop_dir" "$icon_dir"
cp "$icon_src" "$icon_target"

exec_path="$bin_path gui"
sed \
  -e "s|^Exec=.*$|Exec=$exec_path|" \
  -e "s|^Icon=.*$|Icon=$icon_target|" \
  "$desktop_src" > "$desktop_target"

chmod 644 "$desktop_target" "$icon_target"

if command -v update-desktop-database >/dev/null 2>&1; then
  update-desktop-database "$desktop_dir" >/dev/null 2>&1 || true
fi

if command -v gtk-update-icon-cache >/dev/null 2>&1; then
  gtk-update-icon-cache "${XDG_DATA_HOME:-$HOME/.local/share}/icons/hicolor" >/dev/null 2>&1 || true
fi

echo "Installed desktop launcher:"
echo "  Desktop entry: $desktop_target"
echo "  Icon:          $icon_target"
echo "  Exec:          $exec_path"
