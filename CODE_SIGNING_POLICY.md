# Code Signing Policy

This document describes how release binaries of **Foldu** are signed. It follows
the requirements of the [SignPath Foundation](https://signpath.org), which
provides free code signing certificates to qualifying open-source projects.

## Committers and reviewers

- The project is developed and maintained by **Tran Duy Thuan**
  (<https://tranduythuan.com>), who owns this repository.
- The same maintainer is solely responsible for code signing.
- Only software artifacts built from this repository's own source code are
  signed. No third-party binaries are ever signed with this certificate.

## Privacy policy

This program will not transfer any information to other networked systems unless
specifically requested by the user or the person installing or operating it. In
normal operation, Foldu runs fully offline and stores its data only under the
user's local `%APPDATA%\Foldu\` directory.

## What is signed

Signing applies only to the official Windows release artifacts produced for a
tagged version:

- `foldu.exe` — the portable single-file executable
- `Foldu_<version>_x64-setup.exe` — the NSIS installer

## How artifacts are built and signed

1. A release is triggered by pushing a Git tag of the form `v<version>`
   (for example `v1.1.0`) to the public repository.
2. The GitHub Actions workflow at `.github/workflows/release.yml` builds the
   artifacts on a clean `windows-latest` runner directly from the tagged source.
3. The built artifacts are submitted to SignPath for signing under a signing
   policy that requires an origin verification against this repository.
4. The signing certificate's private key is held exclusively on SignPath
   Foundation's Hardware Security Module (HSM). The maintainer never has access
   to the private key and cannot export it.
5. Signed artifacts are attached to the corresponding GitHub Release.

## Verifying a download

Every release lists a SHA-256 hash for each file on its GitHub Releases page.
After signing is in place, the signature can additionally be inspected on Windows
via the file's **Properties → Digital Signatures** tab, which will show
**Tran Duy Thuan** as the signer.

## Source and reproducibility

All source code is public in this repository under the [MIT License](LICENSE).
Anyone can reproduce a build by following the "Build from source" instructions in
the [README](README.md).
