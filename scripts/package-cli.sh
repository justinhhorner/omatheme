#!/usr/bin/env bash
# Builds the omatheme command-line tool for release and packages it as
#   <out-dir>/omatheme-<version>-<platform>.tar.gz   (macOS, Linux)
#   <out-dir>/omatheme-<version>-<platform>.zip      (Windows)
#
# Usage: scripts/package-cli.sh <platform> [out-dir]      (default out-dir: dist)
#   platform: macos-universal | windows-x64 | windows-arm64 | linux-x64
# Run it on the matching OS (Git Bash on Windows). macos-universal builds both architectures and joins
# them with lipo.
#
# On macOS the binary is signed and notarized when the same variables as build-macos-app.sh are set
# (MACOS_SIGN_IDENTITY, MACOS_KEYCHAIN, MACOS_NOTARY_KEY_PATH/_KEY_ID/_ISSUER); otherwise it's left as
# the linker signed it.
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
platform="${1:-}"
out_dir="${2:-dist}"
mkdir -p "$out_dir"
out_dir="$(cd "$out_dir" && pwd)"

case "$platform" in
    macos-universal) triples=(aarch64-apple-darwin x86_64-apple-darwin); exe=omatheme; archive=tar.gz ;;
    linux-x64) triples=(x86_64-unknown-linux-gnu); exe=omatheme; archive=tar.gz ;;
    windows-x64) triples=(x86_64-pc-windows-msvc); exe=omatheme.exe; archive=zip ;;
    windows-arm64) triples=(aarch64-pc-windows-msvc); exe=omatheme.exe; archive=zip ;;
    *) echo "usage: package-cli.sh <macos-universal|windows-x64|windows-arm64|linux-x64> [out-dir]" >&2; exit 2 ;;
esac

version="$("$root/scripts/version.sh" get)"
name="omatheme-$version-$platform"
staging="$(mktemp -d)"
trap 'rm -rf "$staging"' EXIT
mkdir "$staging/$name"

if command -v rustup >/dev/null; then
    rustup target add "${triples[@]}"
fi
for triple in "${triples[@]}"; do
    cargo build --release --locked --manifest-path "$root/cli/Cargo.toml" --target "$triple"
done

binaries=()
for triple in "${triples[@]}"; do
    binaries+=("$root/cli/target/$triple/release/$exe")
done
if [[ ${#binaries[@]} -gt 1 ]]; then
    lipo -create -output "$staging/$name/$exe" "${binaries[@]}"
else
    cp "${binaries[0]}" "$staging/$name/$exe"
fi

if [[ "$platform" == macos-* && -n "${MACOS_SIGN_IDENTITY:-}" ]]; then
    keychain="${MACOS_KEYCHAIN:-}"
    codesign --force --options runtime --timestamp --sign "$MACOS_SIGN_IDENTITY" \
        ${keychain:+--keychain "$keychain"} "$staging/$name/$exe"
    codesign --verify --strict "$staging/$name/$exe"
    if [[ -n "${MACOS_NOTARY_KEY_PATH:-}" && -n "${MACOS_NOTARY_KEY_ID:-}" && -n "${MACOS_NOTARY_ISSUER:-}" ]]; then
        # A bare binary can't carry a stapled ticket; notarizing records it with Apple, which is what
        # Gatekeeper checks when a downloaded copy first runs.
        ditto -c -k --keepParent "$staging/$name/$exe" "$staging/notarize.zip"
        xcrun notarytool submit "$staging/notarize.zip" \
            --key "$MACOS_NOTARY_KEY_PATH" --key-id "$MACOS_NOTARY_KEY_ID" --issuer "$MACOS_NOTARY_ISSUER" \
            --wait
        rm "$staging/notarize.zip"
    fi
fi

# A release binary must report the release's version.
reported="$("$staging/$name/$exe" --version 2>/dev/null || true)"
if [[ "$platform" != windows-arm64 || "$(uname -m)" == aarch64 || "$(uname -m)" == arm64 ]]; then
    [[ "$reported" == *"$version"* ]] || {
        echo "package-cli.sh: '$exe --version' printed '$reported', expected $version" >&2
        exit 1
    }
fi

case "$archive" in
    tar.gz)
        out="$out_dir/$name.tar.gz"
        tar -C "$staging" -czf "$out" "$name"
        ;;
    zip)
        out="$out_dir/$name.zip"
        rm -f "$out"
        # Native Windows tools want C:\ paths, not Git Bash's /c/ ones.
        winpath() { if command -v cygpath >/dev/null; then cygpath -w "$1"; else echo "$1"; fi; }
        if command -v 7z >/dev/null; then
            (cd "$staging" && 7z a -tzip -bso0 -bsp0 "$(winpath "$out")" "$name")
        elif command -v zip >/dev/null; then
            (cd "$staging" && zip -qr "$out" "$name")
        else
            pwsh -NoProfile -Command \
                "Compress-Archive -Path '$(winpath "$staging/$name")' -DestinationPath '$(winpath "$out")'"
        fi
        ;;
esac
echo "$out"
