# Code signing policy

Free code signing provided by [SignPath.io](https://signpath.io), certificate by
[SignPath Foundation](https://signpath.org).

**Status:** Declutter has not been accepted by SignPath Foundation yet, and no release is signed so far. This policy
describes how signing works once it is. Until then releases are unsigned (see the notice in the README and
[`DISTRIBUTION.md`](DISTRIBUTION.md)).

## What is signed

Only the Declutter 1.0 Windows release files built from this repository:

- `declutter.exe`, the app (signed before it is packed, so the installed program and the exe in the portable zip both
  carry the signature);
- `Declutter_<version>_x64-setup.exe`, the per-user NSIS installer.

The portable zip itself is not signed (its exe is). Nothing else is signed: no third-party programs, no v0.x Python
releases (`release.yml`), no local or development builds.

## When signing happens

Only for official releases: a push of a `v1.*` tag to `w1tneess/declutter`. A manual run of the workflow is a dry run
and is never signed. Each signing request is approved by hand twice: once in GitHub (the `release-signing` environment
requires the maintainer's approval) and once in SignPath.

## Build system

Every signed file is built by this repository's public workflow
[`.github/workflows/release-v1.yml`](../.github/workflows/release-v1.yml) on GitHub-hosted runners, from the tagged
commit, with no dependency caches. The build jobs upload the unsigned files as workflow artifacts; separate signing
jobs, which run no build tools or project code, send exactly those artifacts to SignPath, which checks that they come
from this repository's workflow. The signed installer, the portable zip and `SHA256SUMS.txt` are then published to the
GitHub release with a build provenance attestation.

## Team roles

Declutter has a single maintainer, [w1tneess](https://github.com/w1tneess). There are no other team members, so one
person holds every role:

| Role | Who | What it means here |
|---|---|---|
| Author (committer) | w1tneess | May change the source in this repository without further review. |
| Reviewer | w1tneess | Reviews every change proposed by anyone else (pull requests) before it is merged. |
| Approver | w1tneess | Approves each signing request in GitHub and in SignPath. |

Commits may also appear under the maintainer's name, Advaita Chandra; it is the same person. Automated dependency
pull requests (Dependabot) are reviewed like any other outside change.

## Multi-factor authentication

Everyone with write access to this repository or access to the SignPath project must use multi-factor authentication
for both GitHub and SignPath.

## Privacy

This program will not transfer any information to other networked systems unless specifically requested by the user.

Declutter works only on the folders you choose on your own PC. The app has no telemetry and makes no network
requests: its interface may talk only to the app itself (its content security policy allows no other connections),
and it is granted no HTTP, shell or file-system permission. Settings, the undo log and the "space freed" tally stay in
`%LOCALAPPDATA%\Declutter` on your PC.

One network call happens during installation: if the Microsoft Edge WebView2 Runtime is missing, the installer
downloads Microsoft's WebView2 bootstrapper from Microsoft to install it. Declutter adds no information about you or
your files to that download.
