#!/usr/bin/env bash
# One version for every client. It lives in three files, kept equal by this script:
#   macos/project.yml               MARKETING_VERSION (+ CURRENT_PROJECT_VERSION, the build number)
#   windows/Directory.Build.props   <Version>
#   cli/Cargo.toml (+ Cargo.lock)   [package] version
#
# Usage:
#   scripts/version.sh get                     print the version (fails if the files disagree)
#   scripts/version.sh check <version>         fail unless every file has <version>
#   scripts/version.sh next <patch|minor|major|X.Y.Z[-pre]>
#                                              print the version a release would get
#   scripts/version.sh set <patch|minor|major|X.Y.Z[-pre]>
#                                              write it to every file and bump the build number
#
# Uses perl rather than sed so it behaves the same on macOS, Linux and Git Bash.
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
project_yml="$root/macos/project.yml"
build_props="$root/windows/Directory.Build.props"
cargo_toml="$root/cli/Cargo.toml"
cargo_lock="$root/cli/Cargo.lock"

semver='^[0-9]+\.[0-9]+\.[0-9]+(-[0-9A-Za-z.-]+)?$'

die() { echo "version.sh: $*" >&2; exit 1; }

mac_version() { perl -ne 'print "$1\n" and exit if /^\s*MARKETING_VERSION:\s*"?([^"\s]+)"?/' "$project_yml"; }
build_number() { perl -ne 'print "$1\n" and exit if /^\s*CURRENT_PROJECT_VERSION:\s*"?(\d+)"?/' "$project_yml"; }
windows_version() { perl -ne 'print "$1\n" and exit if m{<Version>([^<]+)</Version>}' "$build_props"; }
# The first `version =` in Cargo.toml is the [package] one.
cli_version() { perl -ne 'print "$1\n" and exit if /^version\s*=\s*"([^"]+)"/' "$cargo_toml"; }
# The version recorded for the omatheme package in Cargo.lock.
lock_version() { perl -0777 -ne 'print "$1\n" if /\[\[package\]\]\r?\nname = "omatheme"\r?\nversion = "([^"]+)"/' "$cargo_lock"; }

current() {
    local mac win cli lock
    mac="$(mac_version)"; win="$(windows_version)"; cli="$(cli_version)"; lock="$(lock_version)"
    [[ -n "$mac" && -n "$win" && -n "$cli" && -n "$lock" ]] || die "couldn't read a version from every file"
    if [[ "$mac" != "$win" || "$mac" != "$cli" || "$mac" != "$lock" ]]; then
        die "versions disagree: macos/project.yml=$mac windows/Directory.Build.props=$win cli/Cargo.toml=$cli cli/Cargo.lock=$lock"
    fi
    echo "$mac"
}

next() {
    local spec="$1" cur major minor patch
    if [[ "$spec" =~ $semver ]]; then
        echo "$spec"
        return
    fi
    cur="$(current)"
    if [[ "$spec" == patch && "$cur" == *-* ]]; then
        echo "${cur%%-*}" # a patch release after 1.2.0-rc.1 is 1.2.0
        return
    fi
    cur="${cur%%-*}"
    IFS=. read -r major minor patch <<<"$cur"
    case "$spec" in
        patch) echo "$major.$minor.$((patch + 1))" ;;
        minor) echo "$major.$((minor + 1)).0" ;;
        major) echo "$((major + 1)).0.0" ;;
        *) die "expected patch, minor, major or a version like 1.2.3 (got '$spec')" ;;
    esac
}

set_version() {
    local version build
    version="$(next "$1")"
    [[ "$version" =~ $semver ]] || die "not a version: $version"
    current >/dev/null # refuse to start from files that already disagree
    build="$(build_number)"
    [[ -n "$build" ]] || die "couldn't read CURRENT_PROJECT_VERSION from macos/project.yml"
    build=$((build + 1))

    V="$version" B="$build" perl -pi -e '
        s/^(\s*MARKETING_VERSION:\s*)"?[^"\s]+"?/$1"$ENV{V}"/;
        s/^(\s*CURRENT_PROJECT_VERSION:\s*)"?\d+"?/$1"$ENV{B}"/;
    ' "$project_yml"
    V="$version" perl -pi -e 's{<Version>[^<]+</Version>}{<Version>$ENV{V}</Version>}' "$build_props"
    V="$version" perl -pi -e '$done ||= s/^version\s*=\s*"[^"]+"/version = "$ENV{V}"/' "$cargo_toml"
    V="$version" perl -0777 -pi -e 's/(\[\[package\]\]\r?\nname = "omatheme"\r?\nversion = )"[^"]+"/$1"$ENV{V}"/' "$cargo_lock"

    [[ "$(current)" == "$version" ]] || die "wrote $version but the files don't all say so"
    echo "$version"
}

cmd="${1:-}"
case "$cmd" in
    get) current ;;
    check)
        [[ $# -eq 2 ]] || die "usage: version.sh check <version>"
        actual="$(current)"
        [[ "$actual" == "$2" ]] || die "the files say $actual, not $2"
        echo "$actual"
        ;;
    next)
        [[ $# -eq 2 ]] || die "usage: version.sh next <patch|minor|major|X.Y.Z>"
        next "$2"
        ;;
    set)
        [[ $# -eq 2 ]] || die "usage: version.sh set <patch|minor|major|X.Y.Z>"
        set_version "$2"
        ;;
    *) die "usage: version.sh get | check <version> | next <spec> | set <spec>" ;;
esac
