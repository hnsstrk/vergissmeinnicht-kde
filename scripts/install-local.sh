#!/usr/bin/env bash
# Baut Vergissmeinnicht im Release-Modus und installiert es für den aktuellen
# Benutzer (~/.local). Reproduzierbare Lokal-Install-Pipeline — Pendant zum
# install-local.sh der macOS-Version.
#
#   scripts/install-local.sh [--skip-build]
#
set -euo pipefail

cd "$(dirname "$0")/.."

PREFIX="${PREFIX:-$HOME/.local}"
SKIP_BUILD=0
for arg in "$@"; do
    case "$arg" in
        --skip-build) SKIP_BUILD=1 ;;
        *) echo "Unbekannte Option: $arg" >&2; exit 2 ;;
    esac
done

if [ "$SKIP_BUILD" -eq 0 ]; then
    echo "==> cargo build --release"
    cargo build --release
fi

echo "==> Installiere nach $PREFIX"
install -Dm755 target/release/vergissmeinnicht "$PREFIX/bin/vergissmeinnicht"
install -Dm644 data/de.hnsstrk.vergissmeinnicht.desktop \
    "$PREFIX/share/applications/de.hnsstrk.vergissmeinnicht.desktop"
install -Dm644 data/icons/de.hnsstrk.vergissmeinnicht.svg \
    "$PREFIX/share/icons/hicolor/scalable/apps/de.hnsstrk.vergissmeinnicht.svg"
install -Dm644 data/de.hnsstrk.vergissmeinnicht.metainfo.xml \
    "$PREFIX/share/metainfo/de.hnsstrk.vergissmeinnicht.metainfo.xml"

echo "==> Übersetzungen (msgfmt)"
for po in po/*.po; do
    lang="$(basename "$po" .po)"
    mo_dir="$PREFIX/share/locale/$lang/LC_MESSAGES"
    mkdir -p "$mo_dir"
    msgfmt -o "$mo_dir/vergissmeinnicht.mo" "$po"
    echo "    $lang"
done

# Caches aktualisieren (best effort)
command -v update-desktop-database >/dev/null && update-desktop-database "$PREFIX/share/applications" || true
command -v gtk-update-icon-cache >/dev/null && gtk-update-icon-cache -q "$PREFIX/share/icons/hicolor" || true

echo "==> Fertig. Start: vergissmeinnicht (oder über den Anwendungsstarter)"
