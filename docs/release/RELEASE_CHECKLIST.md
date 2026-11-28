# Release checklist

How a ToolOnize release is made. Pushing a `vX.Y.Z` tag runs
`.github/workflows/release.yml`, which builds every installer, signs it with
the update key, writes `latest.json`, `SHA256SUMS.txt` and a build-provenance
attestation, and creates a **draft** release whose notes come from
`CHANGELOG.md`. Nothing reaches users until a maintainer publishes the draft.
The update key is set up once, as described in [UPDATES.md](UPDATES.md).

## 1. Prepare

- [ ] Version bumped in `src-tauri/tauri.conf.json`, `src-tauri/Cargo.toml`
      and `package.json` (with `Cargo.lock` and `package-lock.json`).
- [ ] `CHANGELOG.md` has a `## X.Y.Z — YYYY-MM-DD` section written for users;
      it becomes the release notes.
- [ ] CI green on the release commit (`ci` and `repository-safety`).
- [ ] `tools/check-public-safety.sh` clean; a gitleaks scan of the history is
      clean.
- [ ] [DEPENDENCIES.md](../security/DEPENDENCIES.md) lists every new
      dependency with its license.

## 2. Build

- [ ] Tag the release commit and push the tag:
      `git tag vX.Y.Z <commit> && git push origin vX.Y.Z`.
- [ ] The release workflow finished and the draft has the deb, rpm, AppImage
      and Windows installer, a `.sig` for each, `latest.json` and
      `SHA256SUMS.txt`.

## 3. Verify the draft

- [ ] `sha256sum -c SHA256SUMS.txt --ignore-missing` passes for downloaded
      files, and `gh attestation verify <file> --repo OWNER/REPO` succeeds.
- [ ] `latest.json` names the new version and has a signature for every
      platform.
- [ ] Smoke test on clean machines, with fictional data only:

| Check | Ubuntu (deb) | Fedora (rpm) / other (AppImage) | Windows 11 |
| --- | --- | --- | --- |
| Install over the previous release; workspaces and notes are kept | | | |
| Launchers view lists entries; nothing ran during discovery | | | |
| Create a workspace, add a shell, type, resize, search | | | |
| Grid → Focus → Tabs → Master + Stack keeps the same running terminals | | | |
| Close and reopen: same window, terminals in the same folders with their output | | | |
| Kill the app (`kill -9`), reopen: same result, with the "did not close normally" notice | | | |
| A Desktop-folder or custom-folder launcher needs review before it can be added | | | |
| Editing a pinned launcher's command → "re-review required"; it cannot start | | | |
| SSH member: drop the connection → automatic reconnect with countdown | | | |
| WSL distribution terminal | n/a | n/a | |
| Export contains no absolute paths or secrets; import lands in review | | | |
| Keyboard only: command palette, layout switch, member navigation | | | |
| Arabic interface: right-to-left layout, terminals left-to-right | | | |
| High-contrast theme and reduced motion | | | |
| Uninstall leaves only the documented state and config folders | | | |

## 4. Publish

- [ ] Review the generated notes, then **Publish release** (mark it as the
      latest release).
- [ ] `https://github.com/OWNER/REPO/releases/latest/download/latest.json`
      shows the new version.
- [ ] An installed previous release offers the update (Settings › Updates ›
      Check now) and comes back on the new version with its terminals.
