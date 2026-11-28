# shellcheck shell=bash
# shellcheck disable=SC2034 # exit codes and versions are used by the sourcing scripts
# Shared helpers for the ToolOnize setup wizards (Linux, macOS).
# ToolOnize is a CRMonize.com open-source project — https://crmonize.com
#
# Must stay compatible with bash 3.2 (macOS): no associative arrays, no
# mapfile, no ${var,,}. Nothing here runs with elevated privileges.

TOOLONIZE_NODE_VERSION="24.22.1" # Node.js "Krypton" LTS
TOOLONIZE_NODE_MIN="20.0.0"
TOOLONIZE_RUST_MIN="1.77.2" # src-tauri/Cargo.toml rust-version
TOOLONIZE_RUSTUP_VERSION="1.29.2"

# Pinned SHA-256 digests of the official downloads, copied from
# https://nodejs.org/dist/v<ver>/SHASUMS256.txt and
# https://static.rust-lang.org/rustup/archive/<ver>/<target>/rustup-init.sha256.
# Bumping a version above means updating these; a mismatch aborts the step.
node_sha256() {
  case "$1" in
    linux-x64) echo 6c172a9ab6718f7b801fb5a7bd486af2ee5e32758086a04b84fcce2aa77095e6 ;;
    linux-arm64) echo dbabf7a1cbf947e6059ffad2c27b80041db97dea1e553b62afdee7f13fca336e ;;
    darwin-x64) echo 65e450f90a6eaacd5000f611861c4ce65fadc87f813795dc4ed0c0e5f2fca2d4 ;;
    darwin-arm64) echo 654ba79de6155647e22321932b45f2a1ccb705976cc337c9b8e278beb085a628 ;;
    *) return 1 ;;
  esac
}

rustup_sha256() {
  case "$1" in
    x86_64-unknown-linux-gnu) echo 5be191e8c70a5007074254cb74d78dbfdd25cd5a61c27de7b8e5410122ec24f4 ;;
    aarch64-unknown-linux-gnu) echo a9c168dbea81a84671c6b7a5c1795cb0eb699020110d107ddfca427bd797e697 ;;
    x86_64-apple-darwin) echo b9610730ddeef52765723847866fd1e3540b2bf7d5ab0c324aa143a5edf25ba0 ;;
    aarch64-apple-darwin) echo f7c7e11eb2e07f006e532a8aaac77b32f4527d305ac386c0c633ca6133d35ca9 ;;
    *) return 1 ;;
  esac
}

# Exit codes shared by all wizards (used by the scripts that source this).
EXIT_OK=0
EXIT_MISSING=1
EXIT_USAGE=2
EXIT_CANCELLED=3
EXIT_UNSUPPORTED=4

SETUP_YES=0
SETUP_FORCE_TEXT=0
SETUP_UI=text
SETUP_TITLE="ToolOnize setup"

say() { printf '%s\n' "$*"; }
warn() { printf 'warning: %s\n' "$*" >&2; }
die() {
  printf 'error: %s\n' "$1" >&2
  exit "${2:-1}"
}

# ---------------------------------------------------------------- versions

# ver_ge A B: true when dotted version A >= B (first three fields, numeric).
ver_ge() {
  local IFS=. i x y
  local -a a b
  read -r -a a <<<"$1"
  read -r -a b <<<"$2"
  for i in 0 1 2; do
    x="${a[$i]:-0}"
    y="${b[$i]:-0}"
    x="${x%%[!0-9]*}"
    y="${y%%[!0-9]*}"
    x="${x:-0}"
    y="${y:-0}"
    [ "$x" -gt "$y" ] && return 0
    [ "$x" -lt "$y" ] && return 1
  done
  return 0
}

# First dotted version number in a string ("rustc 1.97.0 (...)" -> 1.97.0).
first_version() {
  printf '%s\n' "$1" | sed -n 's/^[^0-9]*\([0-9][0-9]*\.[0-9][0-9]*\(\.[0-9][0-9]*\)\{0,1\}\).*/\1/p' | head -n 1
}

# ------------------------------------------------------------ requirements
# Parallel arrays (bash 3.2): one entry per check.
REQ_IDS=()
REQ_LABELS=()
REQ_LEVELS=()  # required | optional
REQ_STATES=()  # ok | missing
REQ_DETAILS=()
REQ_FIXES=()   # pkg:<name> | node | rust | manual:<text> | -

req_reset() {
  REQ_IDS=()
  REQ_LABELS=()
  REQ_LEVELS=()
  REQ_STATES=()
  REQ_DETAILS=()
  REQ_FIXES=()
}

# req_add ID LABEL LEVEL STATE DETAIL FIX
req_add() {
  REQ_IDS+=("$1")
  REQ_LABELS+=("$2")
  REQ_LEVELS+=("$3")
  REQ_STATES+=("$4")
  REQ_DETAILS+=("$5")
  REQ_FIXES+=("$6")
}

req_count_missing() {
  local level="$1" n=0 i
  for i in "${!REQ_IDS[@]}"; do
    if [ "${REQ_STATES[$i]}" = missing ] && [ "${REQ_LEVELS[$i]}" = "$level" ]; then
      n=$((n + 1))
    fi
  done
  echo "$n"
}

req_report() {
  local i mark
  for i in "${!REQ_IDS[@]}"; do
    if [ "${REQ_STATES[$i]}" = ok ]; then
      mark="[ ok ]    "
    elif [ "${REQ_LEVELS[$i]}" = required ]; then
      mark="[MISSING] "
    else
      mark="[optional]"
    fi
    printf '  %s %s — %s\n' "$mark" "${REQ_LABELS[$i]}" "${REQ_DETAILS[$i]}"
  done
}

# Compact form for wizard pages: a count of what is present, then only the
# missing items (the full list is what --check prints).
req_summary() {
  local i ok=0 total=0
  for i in "${!REQ_IDS[@]}"; do
    total=$((total + 1))
    [ "${REQ_STATES[$i]}" = ok ] && ok=$((ok + 1))
  done
  printf '  %s of %s requirements are already present.\n' "$ok" "$total"
  for i in "${!REQ_IDS[@]}"; do
    [ "${REQ_STATES[$i]}" = ok ] && continue
    if [ "${REQ_LEVELS[$i]}" = required ]; then
      printf '  [MISSING]  %s — %s\n' "${REQ_LABELS[$i]}" "${REQ_DETAILS[$i]}"
    else
      printf '  [optional] %s — %s\n' "${REQ_LABELS[$i]}" "${REQ_DETAILS[$i]}"
    fi
  done
  return 0
}

# ------------------------------------------------------------------ checks

check_node() {
  local v npm_ok=1
  v="$(first_version "$(node --version 2>/dev/null || true)")"
  command -v npm >/dev/null 2>&1 || npm_ok=0
  if [ -n "$v" ] && ver_ge "$v" "$TOOLONIZE_NODE_MIN" && [ "$npm_ok" = 1 ]; then
    req_add node "Node.js >= $TOOLONIZE_NODE_MIN and npm" required ok "node $v" -
  elif [ -n "$v" ] && [ "$npm_ok" = 1 ]; then
    req_add node "Node.js >= $TOOLONIZE_NODE_MIN and npm" required missing "node $v is too old" node
  elif [ -n "$v" ]; then
    req_add node "Node.js >= $TOOLONIZE_NODE_MIN and npm" required missing "npm not found" node
  else
    req_add node "Node.js >= $TOOLONIZE_NODE_MIN and npm" required missing "not found" node
  fi
}

check_rust() {
  local rv cv
  rv="$(first_version "$(rustc --version 2>/dev/null || true)")"
  cv="$(first_version "$(cargo --version 2>/dev/null || true)")"
  if [ -n "$rv" ] && [ -n "$cv" ] && ver_ge "$rv" "$TOOLONIZE_RUST_MIN"; then
    req_add rust "Rust >= $TOOLONIZE_RUST_MIN (rustc, cargo)" required ok "rustc $rv" -
  elif [ -n "$rv" ] && [ -n "$cv" ]; then
    req_add rust "Rust >= $TOOLONIZE_RUST_MIN (rustc, cargo)" required missing "rustc $rv is too old" rust
  elif command -v rustup >/dev/null 2>&1; then
    req_add rust "Rust >= $TOOLONIZE_RUST_MIN (rustc, cargo)" required missing "rustup has no default toolchain" rust
  else
    req_add rust "Rust >= $TOOLONIZE_RUST_MIN (rustc, cargo)" required missing "not found" rust
  fi
}

# ---------------------------------------------------------------------- UI

ui_init() {
  if [ "$SETUP_FORCE_TEXT" = 0 ] && [ "$SETUP_YES" = 0 ] && [ -t 0 ] && [ -t 1 ] &&
    command -v whiptail >/dev/null 2>&1; then
    SETUP_UI=whiptail
  else
    SETUP_UI=text
  fi
}

ui_size() {
  local lines cols
  lines="$(tput lines 2>/dev/null || echo 24)"
  cols="$(tput cols 2>/dev/null || echo 80)"
  UI_H=$((lines - 4))
  UI_W=$((cols - 8))
  [ "$UI_H" -gt 30 ] && UI_H=30
  [ "$UI_W" -gt 100 ] && UI_W=100
  [ "$UI_H" -lt 12 ] && UI_H=12
  [ "$UI_W" -lt 50 ] && UI_W=50
  return 0
}

# Scrolling only when needed: with --scrolltext whiptail focuses the text
# first, so Enter would not press "Next" on short pages.
ui_scroll_args() {
  UI_SCROLL=()
  local lines
  lines="$(printf '%s\n' "$1" | wc -l)"
  if [ "$lines" -gt $((UI_H - 7)) ]; then
    UI_SCROLL=(--scrolltext)
  fi
  return 0
}

ui_cancelled() {
  say "Setup cancelled. Nothing was changed."
  exit "$EXIT_CANCELLED"
}

# ui_page TITLE TEXT — an informational "Next" step.
ui_page() {
  if [ "$SETUP_YES" = 1 ]; then
    printf '\n== %s ==\n%s\n' "$1" "$2"
    return 0
  fi
  if [ "$SETUP_UI" = whiptail ]; then
    ui_size
    ui_scroll_args "$2"
    whiptail --title "$SETUP_TITLE — $1" ${UI_SCROLL[@]+"${UI_SCROLL[@]}"} --yes-button "Next" --no-button "Cancel" \
      --yesno "$2" "$UI_H" "$UI_W" || ui_cancelled
    return 0
  fi
  printf '\n== %s ==\n%s\n\n' "$1" "$2"
  local reply
  printf 'Press Enter for Next (q to cancel): '
  read -r reply || ui_cancelled
  case "$reply" in q | Q) ui_cancelled ;; esac
}

# ui_message TITLE TEXT — a final message (OK only, never cancels).
ui_message() {
  if [ "$SETUP_UI" = whiptail ] && [ "$SETUP_YES" = 0 ]; then
    ui_size
    ui_scroll_args "$2"
    whiptail --title "$SETUP_TITLE — $1" ${UI_SCROLL[@]+"${UI_SCROLL[@]}"} --msgbox "$2" "$UI_H" "$UI_W" || true
    return 0
  fi
  printf '\n== %s ==\n%s\n' "$1" "$2"
}

# ui_confirm TITLE TEXT YES_LABEL — the single consent step. Returns 0 on yes.
ui_confirm() {
  if [ "$SETUP_YES" = 1 ]; then
    printf '\n== %s ==\n%s\n(--yes given: proceeding)\n' "$1" "$2"
    return 0
  fi
  if [ "$SETUP_UI" = whiptail ]; then
    ui_size
    ui_scroll_args "$2"
    whiptail --title "$SETUP_TITLE — $1" ${UI_SCROLL[@]+"${UI_SCROLL[@]}"} --yes-button "$3" --no-button "Cancel" \
      --yesno "$2" "$UI_H" "$UI_W"
    return $?
  fi
  printf '\n== %s ==\n%s\n\n' "$1" "$2"
  local reply
  printf '%s? [y/N]: ' "$3"
  read -r reply || return 1
  case "$reply" in y | Y | yes | YES | Yes) return 0 ;; *) return 1 ;; esac
}

# ui_yesno QUESTION — a secondary yes/no (defaults to yes).
ui_yesno() {
  if [ "$SETUP_YES" = 1 ]; then
    return 0
  fi
  if [ "$SETUP_UI" = whiptail ]; then
    ui_size
    whiptail --title "$SETUP_TITLE" --yesno "$1" 12 "$UI_W"
    return $?
  fi
  local reply
  printf '%s [Y/n]: ' "$1"
  read -r reply || return 1
  case "$reply" in n | N | no | NO | No) return 1 ;; *) return 0 ;; esac
}

# ------------------------------------------------------------- downloads

sha256_of() {
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$1" | cut -d ' ' -f 1
  elif command -v shasum >/dev/null 2>&1; then
    shasum -a 256 "$1" | cut -d ' ' -f 1
  else
    return 1
  fi
}

# download_verified URL DEST SHA256 — HTTPS only; removes DEST on mismatch.
download_verified() {
  local url="$1" dest="$2" want="$3" got
  command -v curl >/dev/null 2>&1 || {
    warn "curl is required to download $url"
    return 1
  }
  if ! curl --proto '=https' --tlsv1.2 -fsSL --retry 2 -o "$dest" "$url"; then
    rm -f "$dest"
    warn "download failed: $url"
    return 1
  fi
  got="$(sha256_of "$dest" || true)"
  if [ -z "$got" ] || [ "$got" != "$want" ]; then
    rm -f "$dest"
    warn "checksum mismatch for $url (expected $want, got ${got:-nothing}); refusing to use it"
    return 1
  fi
  return 0
}

host_arch() {
  case "$(uname -m)" in
    x86_64 | amd64) echo x64 ;;
    aarch64 | arm64) echo arm64 ;;
    *) echo unsupported ;;
  esac
}

# node_platform: linux-x64 | linux-arm64 | darwin-x64 | darwin-arm64
node_platform() {
  local os arch
  case "$(uname -s)" in
    Linux) os=linux ;;
    Darwin) os=darwin ;;
    *) os=unsupported ;;
  esac
  arch="$(host_arch)"
  echo "$os-$arch"
}

rust_target() {
  case "$(node_platform)" in
    linux-x64) echo x86_64-unknown-linux-gnu ;;
    linux-arm64) echo aarch64-unknown-linux-gnu ;;
    darwin-x64) echo x86_64-apple-darwin ;;
    darwin-arm64) echo aarch64-apple-darwin ;;
    *) echo unsupported ;;
  esac
}

path_has() {
  case ":$PATH:" in *":$1:"*) return 0 ;; *) return 1 ;; esac
}

# install_node_user — official Node.js LTS tarball into ~/.local/opt, linked
# from ~/.local/bin. No root. Refuses to replace a foreign ~/.local/bin/node.
install_node_user() {
  local plat sha name url tmp opt bin target
  plat="$(node_platform)"
  sha="$(node_sha256 "$plat")" || {
    warn "no pinned Node.js build for $plat; install Node.js >= $TOOLONIZE_NODE_MIN manually"
    return 1
  }
  name="node-v$TOOLONIZE_NODE_VERSION-$plat"
  url="https://nodejs.org/dist/v$TOOLONIZE_NODE_VERSION/$name.tar.xz"
  opt="$HOME/.local/opt"
  bin="$HOME/.local/bin"
  for target in node npm npx; do
    if [ -e "$bin/$target" ] && ! [ -L "$bin/$target" ]; then
      warn "$bin/$target exists and was not created by this setup; leaving Node.js alone"
      return 1
    fi
  done
  tmp="$(mktemp -d)"
  say "Downloading $url"
  if ! download_verified "$url" "$tmp/$name.tar.xz" "$sha"; then
    rm -rf "$tmp"
    return 1
  fi
  mkdir -p "$opt" "$bin"
  rm -rf "${opt:?}/$name"
  tar -xJf "$tmp/$name.tar.xz" -C "$opt"
  rm -rf "$tmp"
  for target in node npm npx; do
    ln -sfn "$opt/$name/bin/$target" "$bin/$target"
  done
  say "Node.js $TOOLONIZE_NODE_VERSION installed in $opt/$name (linked from $bin)."
  path_has "$bin" || SETUP_PATH_HINTS="${SETUP_PATH_HINTS}  export PATH=\"\$HOME/.local/bin:\$PATH\"
"
  return 0
}

# install_rust_user — rustup (user level, ~/.cargo). Uses an existing rustup
# when present; otherwise the pinned, checksum-verified rustup-init.
install_rust_user() {
  local target sha url tmp
  if command -v rustup >/dev/null 2>&1; then
    rustup toolchain install stable --profile minimal && rustup default stable
    return $?
  fi
  target="$(rust_target)"
  sha="$(rustup_sha256 "$target")" || {
    warn "no pinned rustup-init for this platform; see https://rustup.rs"
    return 1
  }
  url="https://static.rust-lang.org/rustup/archive/$TOOLONIZE_RUSTUP_VERSION/$target/rustup-init"
  tmp="$(mktemp -d)"
  say "Downloading $url"
  if ! download_verified "$url" "$tmp/rustup-init" "$sha"; then
    rm -rf "$tmp"
    return 1
  fi
  chmod +x "$tmp/rustup-init"
  # rustup adds ~/.cargo/bin to the shell profile, as with any rustup install.
  "$tmp/rustup-init" -y --profile minimal --default-toolchain stable
  local rc=$?
  rm -rf "$tmp"
  path_has "$HOME/.cargo/bin" || SETUP_PATH_HINTS="${SETUP_PATH_HINTS}  . \"\$HOME/.cargo/env\"
"
  return "$rc"
}

SETUP_PATH_HINTS=""
