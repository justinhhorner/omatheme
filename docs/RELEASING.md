# Releasing

Every client is built and published by GitHub Actions. There's nothing to build by hand.

- **CI** (`.github/workflows/ci.yml`) builds and tests the macOS app, the Windows app and the CLI
  (macOS, Windows, Linux) on every push to `main` and every pull request. Changes to Markdown, `docs/`
  and `design/` alone don't trigger it.
- **Release** (`.github/workflows/release.yml`) runs CI again, builds every client from a tag and
  publishes a GitHub release with them.

## Making a release

**From GitHub:** Actions → **Release** → **Run workflow** on `main`, and enter one of:

| Input | From 0.1.0 | From 1.0.0-rc.1 |
|---|---|---|
| `patch` (the default) | 0.1.1 | 1.0.0 |
| `minor` | 0.2.0 | 1.1.0 |
| `major` | 1.0.0 | 2.0.0 |
| a version: `0.1.0`, `1.2.0`, `2.0.0-beta.1` | that version | that version |

The workflow:

1. works out the version and stops if its tag already exists;
2. runs CI on `main` as it is;
3. writes the version to every client, commits "Release vX.Y.Z" to `main` and tags it (the push fails,
   and nothing is released, if `main` moved during the run);
4. builds the clients from the tag, on macOS, Windows and Linux runners;
5. publishes the release with the files below, a SHA-256 checksum list and notes generated from the
   commits since the last release. A version with a `-suffix` is marked as a pre-release.

**From a terminal**, if you'd rather tag yourself:

```bash
scripts/version.sh set minor          # or patch, major, 1.2.0 …; prints the new version
git commit -am "Release v0.2.0"
git tag -a v0.2.0 -m "Omarchy Themes 0.2.0"
git push origin main v0.2.0
```

Pushing a `v*` tag starts the same workflow, minus the version commit; it checks that the tag matches
the version in the files.

## The version

One version for every client, kept in three files by `scripts/version.sh`:

| File | Field |
|---|---|
| `macos/project.yml` | `MARKETING_VERSION`, plus `CURRENT_PROJECT_VERSION` (the build number, +1 per release) |
| `windows/Directory.Build.props` | `<Version>` |
| `cli/Cargo.toml` and `cli/Cargo.lock` | the `omatheme` package's `version` |

`scripts/version.sh get` prints it and fails if the files disagree; `check`, `next` and `set` are
described at the top of the script.

## What's in a release

| File | What |
|---|---|
| `OmarchyThemes-X.Y.Z-macos.dmg` | macOS 14+ app, universal (Apple silicon and Intel) |
| `OmarchyThemes-X.Y.Z-windows-x64.zip`, `…-arm64.zip` | Windows 10 (19041+)/11 app: unpackaged and self-contained (.NET and the Windows App SDK included). Unzip and run `Omarchy Themes\OmarchyThemes.exe`. |
| `omatheme-X.Y.Z-macos-universal.tar.gz` | CLI for macOS |
| `omatheme-X.Y.Z-windows-x64.zip`, `…-arm64.zip` | CLI for Windows |
| `omatheme-X.Y.Z-linux-x64.tar.gz` | CLI for Linux (list, download and terminal colors) |
| `SHA256SUMS.txt` | checksums of all of the above |

The builds are scripts, so they can be run locally too (each checks its output: architectures,
signature, and that the build reports the repo's version):

```bash
scripts/build-macos-app.sh dist                 # on a Mac, with XcodeGen
scripts/package-cli.sh macos-universal dist     # or linux-x64 on Linux, windows-x64/-arm64 in Git Bash
```

```powershell
./windows/tools/Publish-App.ps1 -Runtime win-x64 -OutputDir dist    # PowerShell 7 on Windows
```

## Signing

**macOS.** Without secrets, the app and CLI are ad-hoc signed like a local build, and the release notes
tell people how to open an app Gatekeeper blocks (System Settings → Privacy & Security → Open Anyway).
To ship a Developer ID signed and notarized build, add these repository secrets (Settings → Secrets and
variables → Actions):

| Secret | Value |
|---|---|
| `MACOS_CERTIFICATE_P12` | a *Developer ID Application* certificate with its private key, exported from Keychain Access as .p12, base64-encoded (`base64 -i cert.p12 \| pbcopy`) |
| `MACOS_CERTIFICATE_PASSWORD` | the .p12's export password |
| `MACOS_TEAM_ID` | optional: your Apple Developer team ID |
| `MACOS_NOTARY_KEY` | the contents of an App Store Connect API key (`AuthKey_XXXX.p8`, *Developer* access) |
| `MACOS_NOTARY_KEY_ID` | that key's ID |
| `MACOS_NOTARY_ISSUER` | the issuer ID shown on the API keys page |

With the certificate alone the builds are signed; with the notary key too, the DMG is notarized and
stapled and the CLI binary is notarized. The certificate goes into a temporary keychain that's deleted
at the end of the job.

**Windows.** The app and CLI aren't Authenticode-signed, so SmartScreen may warn on first run (More
info → Run anyway); the release notes say so.

## When a release fails

- **Before the "Commit and tag" job** (version, CI): nothing was changed. Fix the problem and run the
  workflow again.
- **After it** (a build or the publish step): the version commit and tag are on `main`. If the cause
  was transient, use **Re-run failed jobs**. If the code needs a fix, delete the tag
  (`git push origin :refs/tags/vX.Y.Z`), fix and push, then release again, with the same version or
  `patch`.
