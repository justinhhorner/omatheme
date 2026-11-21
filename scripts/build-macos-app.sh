#!/usr/bin/env bash
# Builds the macOS app for release: a universal (arm64 + x86_64) Release build, packaged as
#   <out-dir>/OmarchyThemes-<version>-macos.dmg
#
# Usage: scripts/build-macos-app.sh [out-dir]      (default: dist)
#
# Signing is optional and comes from the environment:
#   MACOS_SIGN_IDENTITY   a "Developer ID Application: …" identity in the keychain. Without it the app is
#                         ad-hoc signed, as in a local build, and Gatekeeper asks before the first launch.
#   MACOS_KEYCHAIN        the keychain holding that identity (CI imports it into a temporary one).
#   MACOS_NOTARY_KEY_PATH, MACOS_NOTARY_KEY_ID, MACOS_NOTARY_ISSUER
#                         an App Store Connect API key; with these (and an identity) the DMG is
#                         notarized and the ticket stapled to it.
#
# Needs Xcode 16+ and XcodeGen (`brew install xcodegen`).
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
out_dir="${1:-dist}"
mkdir -p "$out_dir"
out_dir="$(cd "$out_dir" && pwd)"

version="$("$root/scripts/version.sh" get)"
identity="${MACOS_SIGN_IDENTITY:-}"
keychain="${MACOS_KEYCHAIN:-}"
app_name="Omatheme"
dmg="$out_dir/OmarchyThemes-$version-macos.dmg"

cd "$root/macos"
xcodegen generate --quiet

sign_settings=()
if [[ -n "$identity" ]]; then
    flags="--timestamp"
    [[ -n "$keychain" ]] && flags="$flags --keychain $keychain"
    sign_settings=(CODE_SIGN_IDENTITY="$identity" OTHER_CODE_SIGN_FLAGS="$flags")
    [[ -n "${MACOS_TEAM_ID:-}" ]] && sign_settings+=(DEVELOPMENT_TEAM="$MACOS_TEAM_ID")
    echo "Signing with: $identity"
else
    echo "No MACOS_SIGN_IDENTITY: the app is ad-hoc signed."
fi

derived="$root/macos/build/ReleaseDerivedData"
rm -rf "$derived"
xcodebuild \
    -project OmarchyThemes.xcodeproj \
    -scheme OmarchyThemes \
    -configuration Release \
    -destination 'generic/platform=macOS' \
    -derivedDataPath "$derived" \
    ONLY_ACTIVE_ARCH=NO \
    ${sign_settings[@]+"${sign_settings[@]}"} \
    build | { command -v xcbeautify >/dev/null && xcbeautify --quiet || cat; }

app="$derived/Build/Products/Release/$app_name.app"
[[ -d "$app" ]] || { echo "build-macos-app.sh: no app at $app" >&2; exit 1; }

# A release must run on both architectures and carry a valid signature.
archs="$(lipo -archs "$app/Contents/MacOS/$app_name")"
[[ "$archs" == *arm64* && "$archs" == *x86_64* ]] || {
    echo "build-macos-app.sh: expected a universal binary, got: $archs" >&2
    exit 1
}
codesign --verify --deep --strict "$app"
built_version="$(/usr/libexec/PlistBuddy -c 'Print :CFBundleShortVersionString' "$app/Contents/Info.plist")"
[[ "$built_version" == "$version" ]] || {
    echo "build-macos-app.sh: the app says $built_version, the repo says $version" >&2
    exit 1
}
echo "Built $app_name $version ($archs)"

# The DMG: the app beside a link to /Applications, the usual drag-to-install window.
staging="$(mktemp -d)"
trap 'rm -rf "$staging"' EXIT
ditto "$app" "$staging/$app_name.app"
ln -s /Applications "$staging/Applications"
rm -f "$dmg"
hdiutil create -quiet -volname "$app_name" -srcfolder "$staging" -fs HFS+ -format UDZO -ov "$dmg"

if [[ -n "$identity" ]]; then
    codesign --sign "$identity" --timestamp ${keychain:+--keychain "$keychain"} "$dmg"
    if [[ -n "${MACOS_NOTARY_KEY_PATH:-}" && -n "${MACOS_NOTARY_KEY_ID:-}" && -n "${MACOS_NOTARY_ISSUER:-}" ]]; then
        echo "Notarizing $(basename "$dmg")…"
        xcrun notarytool submit "$dmg" \
            --key "$MACOS_NOTARY_KEY_PATH" --key-id "$MACOS_NOTARY_KEY_ID" --issuer "$MACOS_NOTARY_ISSUER" \
            --wait
        xcrun stapler staple "$dmg"
        spctl --assess --type open --context context:primary-signature --verbose "$dmg"
    else
        echo "No notary key: the DMG is signed but not notarized."
    fi
fi

echo "$dmg"
