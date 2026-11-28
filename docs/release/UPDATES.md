# In-app updates

ToolOnize checks for a newer release by itself and installs it with one
click (ADR-019). Users never need to look at GitHub.

## How it works

1. A tag `vX.Y.Z` runs `.github/workflows/release.yml`. With the update
   signing key available it builds with `createUpdaterArtifacts`, so every
   installer (AppImage, deb, rpm, Windows setup) gets a `.sig` signature, and
   `tools/make-update-manifest.py` writes `latest.json` into the **draft**
   release.
2. Nothing reaches users until you **publish** the draft. Installed copies
   read `https://github.com/<this repository>/releases/latest/download/latest.json`,
   which always points at the latest published release.
3. The app checks at start (after 15 s) and every 6 hours (Settings ›
   Updates › "Check for updates automatically", on by default) or when the
   user clicks "Check now". When a newer version exists it shows "Update to
   X.Y.Z" in the sidebar. Installing needs a click; ToolOnize downloads the
   file, refuses it unless the signature matches the public key built into
   the app, installs it, and restarts — session memory brings the terminals
   back.

Each installed format updates itself in place: AppImage replaces its file,
deb/rpm install through the system package tool (it asks for the
administrator password), Windows runs the installer in passive mode.

## One-time setup (maintainers)

Run on your own computer, from a checkout of the repository:

```bash
scripts/release/setup-updater.sh
```

It creates the signing key in `~/.config/toolonize-release/` (never in the
repository), asks for a password, and stores the private key and password as
repository secrets and the public key as the repository variable
`TAURI_UPDATER_PUBKEY`, using the GitHub CLI. Back up the key file and its
password: installed copies only accept updates signed by that key.

## Publishing an update

```bash
git tag v0.2.0 && git push origin v0.2.0
```

Wait for the release workflow, check the draft (the `latest.json` it
contains, `SHA256SUMS.txt`, attestations), then publish it. Running copies
see the update within 6 hours, or at once with "Check now".

## Builds without updates

Development builds, self-built copies and releases built without the key
have no updater configuration: the updater is not loaded, Settings says the
build is not set up for updates, and nothing is ever contacted.
