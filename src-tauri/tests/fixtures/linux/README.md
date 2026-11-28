# Linux discovery fixtures

All entries are fictional (`org.example.*`, `example.com`). They drive the
T-DISC-LNX-* golden tests in `src/discovery/linux_tests.rs`.

- `desktop-entries/` — registered XDG sources. `data-home` is the user data
  dir (shadows `data-system` by Desktop File ID); `flatpak/exports/share` is
  an extra `XDG_DATA_DIRS` entry tagged as a Flatpak export.
- `desktop-dir/` — `config/user-dirs.dirs` relocates the Desktop folder to
  `$HOME/Bureau`. Entries there have **no Desktop File ID**; identity is
  source root + relative path, and they default to Needs Review.
- `custom-root/` — an opt-in user root with nested entries; symlink and
  over-limit cases are created at test time in temp directories.
