#!/usr/bin/env bash
# shellcheck source-path=SCRIPTDIR
# ToolOnize environment setup wizard — Linux.
# ToolOnize is a CRMonize.com open-source project — https://crmonize.com
#
# Checks everything ToolOnize needs, shows ONE plan, asks ONE consent, then
# authorizes ONCE (a single sudo/pkexec call that runs the package manager)
# and installs everything in one shot. Node.js and Rust are installed at
# user level without root. See docs/adr/009-environment-setup-wizard.md.
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=lib/common.sh
. "$HERE/lib/common.sh"

usage() {
  cat <<'EOF'
Usage: scripts/setup/linux.sh [options]

  --profile dev|run   dev (default): build ToolOnize from source.
                      run: only what the built app needs at runtime.
  --check             Report what is present or missing; change nothing.
                      Exit 0 when every required item is present, 1 otherwise.
  --dry-run           Show the full install plan; change nothing.
  --yes               Accept the plan without prompting (automation).
  --no-optional       Do not offer optional tools (ssh client, tmux, git).
  --skip ID           Skip a check/install (node, rust, packages). Repeatable.
  --text              Plain text prompts even if whiptail is available.
  -h, --help          This help.

Exit codes: 0 ok, 1 missing/failed, 2 usage, 3 cancelled, 4 unsupported.
EOF
}

PROFILE=dev
MODE=install
OPTIONAL=1
SKIPS=" "

while [ $# -gt 0 ]; do
  case "$1" in
    --profile)
      [ $# -ge 2 ] || die "--profile needs a value" "$EXIT_USAGE"
      PROFILE="$2"
      shift
      ;;
    --profile=*) PROFILE="${1#--profile=}" ;;
    --check) MODE=check ;;
    --dry-run) MODE=dry-run ;;
    --yes | -y) SETUP_YES=1 ;;
    --no-optional) OPTIONAL=0 ;;
    --skip)
      [ $# -ge 2 ] || die "--skip needs a value" "$EXIT_USAGE"
      SKIPS="$SKIPS$2 "
      shift
      ;;
    --text) SETUP_FORCE_TEXT=1 ;;
    -h | --help)
      usage
      exit 0
      ;;
    *)
      usage >&2
      exit "$EXIT_USAGE"
      ;;
  esac
  shift
done

case "$PROFILE" in dev | run) ;; *) die "unknown profile: $PROFILE" "$EXIT_USAGE" ;; esac
[ "$(uname -s)" = Linux ] || die "this wizard is for Linux; use scripts/setup/macos.sh or scripts/setup/windows.ps1" "$EXIT_UNSUPPORTED"

skipped() { case "$SKIPS" in *" $1 "*) return 0 ;; *) return 1 ;; esac }

# ------------------------------------------------------------ distribution

OS_RELEASE="${TOOLONIZE_SETUP_OS_RELEASE:-/etc/os-release}"
os_field() {
  [ -r "$OS_RELEASE" ] || return 0
  sed -n "s/^$1=//p" "$OS_RELEASE" | head -n 1 | tr -d '"'"'"
}
DISTRO_ID="$(os_field ID)"
DISTRO_LIKE="$(os_field ID_LIKE)"
DISTRO_NAME="$(os_field PRETTY_NAME)"

detect_pm() {
  local id
  for id in $DISTRO_ID $DISTRO_LIKE; do
    case "$id" in
      debian | ubuntu | linuxmint | pop | elementary | zorin | kali | raspbian) echo apt && return ;;
      fedora | rhel | centos | rocky | almalinux | nobara) echo dnf && return ;;
      arch | manjaro | endeavouros | garuda) echo pacman && return ;;
      opensuse* | suse | sles) echo zypper && return ;;
    esac
  done
  for id in apt-get dnf pacman zypper; do
    if command -v "$id" >/dev/null 2>&1; then
      [ "$id" = apt-get ] && id=apt
      echo "$id" && return
    fi
  done
  echo unknown
}
PM="$(detect_pm)"

# Package names per manager, each verified end to end in CI (apt on Ubuntu,
# dnf/pacman/zypper in containers). zypper uses pkg-config capabilities for
# the development libraries because openSUSE package names vary.
packages_for() {
  local group="$1"
  case "$PM:$group" in
    apt:runtime) echo "libwebkit2gtk-4.1-0 xdg-utils" ;;
    apt:build) echo "build-essential pkg-config curl xz-utils file libssl-dev libwebkit2gtk-4.1-dev libjavascriptcoregtk-4.1-dev libgtk-3-dev libayatana-appindicator3-dev librsvg2-dev libsoup-3.0-dev patchelf" ;;
    dnf:runtime) echo "webkit2gtk4.1 xdg-utils" ;;
    dnf:build) echo "gcc gcc-c++ make pkgconf-pkg-config curl xz file openssl-devel webkit2gtk4.1-devel gtk3-devel libappindicator-gtk3-devel librsvg2-devel libsoup3-devel patchelf" ;;
    pacman:runtime) echo "webkit2gtk-4.1 xdg-utils" ;;
    pacman:build) echo "base-devel curl xz file openssl webkit2gtk-4.1 gtk3 libappindicator-gtk3 librsvg patchelf" ;;
    zypper:runtime) echo "libwebkit2gtk-4_1-0 xdg-utils" ;;
    zypper:build) echo "gcc gcc-c++ make pkgconf curl xz file patchelf pkgconfig(openssl) pkgconfig(webkit2gtk-4.1) pkgconfig(javascriptcoregtk-4.1) pkgconfig(gtk+-3.0) pkgconfig(ayatana-appindicator3-0.1) pkgconfig(librsvg-2.0) pkgconfig(libsoup-3.0)" ;;
    *) echo "" ;;
  esac
}

# optional_tool CMD -> package providing it
optional_package() {
  case "$PM:$1" in
    apt:ssh) echo openssh-client ;;
    dnf:ssh | zypper:ssh) echo openssh-clients ;;
    pacman:ssh) echo openssh ;;
    *:tmux) echo tmux ;;
    *:git) echo git ;;
  esac
}

pkg_installed() {
  case "$PM" in
    apt) dpkg-query -W -f='${db:Status-Abbrev}' "$1" 2>/dev/null | grep -q '^ii' ;;
    dnf | zypper) rpm -q --whatprovides "$1" >/dev/null 2>&1 ;;
    pacman) pacman -Q "$1" >/dev/null 2>&1 ;;
    *) return 1 ;;
  esac
}

# ------------------------------------------------------------------ checks

run_checks() {
  req_reset
  local groups="runtime" group pkg
  [ "$PROFILE" = dev ] && groups="runtime build"
  if ! skipped packages && [ "$PM" = unknown ]; then
    req_add pkg-unknown "System libraries (WebKitGTK 4.1, GTK 3, ...)" required missing \
      "unknown package manager: see docs/setup/SETUP.md" "manual:packages"
  elif ! skipped packages; then
    for group in $groups; do
      for pkg in $(packages_for "$group"); do
        case " ${REQ_IDS[*]:-} " in *" pkg-$pkg "*) continue ;; esac
        if pkg_installed "$pkg"; then
          req_add "pkg-$pkg" "$pkg" required ok "installed" -
        else
          req_add "pkg-$pkg" "$pkg" required missing "not installed ($group)" "pkg:$pkg"
        fi
      done
    done
  fi
  if [ "$PROFILE" = dev ]; then
    skipped node || check_node
    skipped rust || check_rust
  fi
  if [ "$OPTIONAL" = 1 ]; then
    local tool label
    for tool in ssh tmux git; do
      case "$tool" in
        ssh) label="OpenSSH client (SSH members)" ;;
        tmux) label="tmux (persistent sessions)" ;;
        git) label="git" ;;
      esac
      [ "$tool" = git ] && [ "$PROFILE" = run ] && continue
      if command -v "$tool" >/dev/null 2>&1; then
        req_add "opt-$tool" "$label" optional ok "found" -
      else
        req_add "opt-$tool" "$label" optional missing "not found" "pkg:$(optional_package "$tool")"
      fi
    done
  fi
}

# ------------------------------------------------------------------- plan

ROOT_PKGS=""
USER_STEPS=""

build_plan() {
  local include_optional="$1" i fix
  ROOT_PKGS=""
  USER_STEPS=""
  for i in "${!REQ_IDS[@]}"; do
    [ "${REQ_STATES[$i]}" = missing ] || continue
    if [ "${REQ_LEVELS[$i]}" = optional ] && [ "$include_optional" = 0 ]; then
      continue
    fi
    fix="${REQ_FIXES[$i]}"
    case "$fix" in
      pkg:?*) ROOT_PKGS="$ROOT_PKGS ${fix#pkg:}" ;;
      node | rust) USER_STEPS="$USER_STEPS $fix" ;;
    esac
  done
  ROOT_PKGS="${ROOT_PKGS# }"
  USER_STEPS="${USER_STEPS# }"
}

root_command_text() {
  case "$PM" in
    apt) echo "apt-get update && apt-get install -y $ROOT_PKGS" ;;
    dnf) echo "dnf install -y $ROOT_PKGS" ;;
    pacman) echo "pacman -Syu --needed --noconfirm $ROOT_PKGS   (also updates the system: Arch does not support partial upgrades)" ;;
    zypper) echo "zypper --non-interactive install $ROOT_PKGS" ;;
  esac
}

plan_text() {
  local s="" step
  if [ -n "$ROOT_PKGS" ]; then
    s="${s}-  System packages (one administrator authorization, $PM):
     $(root_command_text)
"
  fi
  for step in $USER_STEPS; do
    case "$step" in
      node) s="${s}-  Node.js $TOOLONIZE_NODE_VERSION LTS for your user (no root):
     official nodejs.org tarball, SHA-256 verified, into ~/.local/opt, linked from ~/.local/bin
" ;;
      rust) s="${s}-  Rust stable for your user (no root) via rustup $TOOLONIZE_RUSTUP_VERSION:
     SHA-256 verified rustup-init; rustup adds ~/.cargo/bin to your shell profile
" ;;
    esac
  done
  [ -n "$s" ] || s="Nothing to install."
  printf '%s' "$s"
}

# The single privileged step: one sudo/pkexec invocation runs the whole
# package-manager transaction. Package names travel as positional arguments,
# never through string interpolation.
run_root_once() {
  local script
  # shellcheck disable=SC2016 # expanded by the privileged shell, not here
  case "$PM" in
    apt) script='export DEBIAN_FRONTEND=noninteractive; apt-get update && apt-get install -y "$@"' ;;
    dnf) script='dnf install -y "$@"' ;;
    pacman) script='pacman -Syu --needed --noconfirm "$@"' ;;
    zypper) script='zypper --non-interactive install "$@"' ;;
    *) return 1 ;;
  esac
  # shellcheck disable=SC2086 # ROOT_PKGS is a fixed list of package names
  if [ "$(id -u)" = 0 ]; then
    sh -c "$script" sh $ROOT_PKGS
  elif command -v sudo >/dev/null 2>&1; then
    say "Administrator authorization (asked once for the whole install):"
    sudo -- sh -c "$script" sh $ROOT_PKGS
  elif command -v pkexec >/dev/null 2>&1; then
    pkexec sh -c "$script" sh $ROOT_PKGS
  else
    warn "neither sudo nor pkexec is available; run as root: $(root_command_text)"
    return 1
  fi
}

# ------------------------------------------------------------------- main

run_checks
REPORT="$(req_report)"
MISSING_REQUIRED="$(req_count_missing required)"
MISSING_OPTIONAL="$(req_count_missing optional)"
HEADER="System: ${DISTRO_NAME:-unknown Linux} (package manager: $PM, profile: $PROFILE)"

if [ "$MODE" = check ]; then
  say "$HEADER"
  say "$REPORT"
  say ""
  say "Required missing: $MISSING_REQUIRED, optional missing: $MISSING_OPTIONAL"
  [ "$MISSING_REQUIRED" = 0 ] && exit "$EXIT_OK"
  exit "$EXIT_MISSING"
fi

if [ "$PM" = unknown ] && ! skipped packages; then
  say "$HEADER"
  say "$REPORT"
  die "unsupported distribution: install the packages listed in docs/setup/SETUP.md manually, then rerun with --skip packages" "$EXIT_UNSUPPORTED"
fi

ui_init

if [ "$MODE" = dry-run ]; then
  build_plan "$OPTIONAL"
  say "$HEADER"
  say "$REPORT"
  say ""
  say "Plan (dry run — nothing will be changed):"
  plan_text
  say ""
  exit "$EXIT_OK"
fi

ui_page "Welcome" "This wizard prepares this computer for ToolOnize, a CRMonize.com open-source project.

1. Check what is already installed.
2. Show one plan with everything that is missing.
3. Ask for your consent and administrator authorization once.
4. Install everything in one go and verify the result.

Nothing is changed until you accept the plan."

ui_page "Check results" "$HEADER

$(req_summary)"

if [ "$MISSING_REQUIRED" = 0 ] && [ "$MISSING_OPTIONAL" = 0 ]; then
  ui_message "Done" "Everything ToolOnize needs is already installed."
  exit "$EXIT_OK"
fi

INCLUDE_OPTIONAL=0
if [ "$MISSING_OPTIONAL" != 0 ]; then
  if ui_yesno "Also install the missing optional tools (SSH client, tmux, git)?"; then
    INCLUDE_OPTIONAL=1
  fi
fi
build_plan "$INCLUDE_OPTIONAL"

if [ -z "$ROOT_PKGS" ] && [ -z "$USER_STEPS" ]; then
  ui_message "Done" "Nothing to install. Required items are present."
  exit "$EXIT_OK"
fi

ui_confirm "Install plan" "The following will be installed:

$(plan_text)

You will be asked for administrator authorization once, then everything is installed in one go." "Install" || ui_cancelled

FAILED=""
if [ -n "$ROOT_PKGS" ]; then
  run_root_once || FAILED="$FAILED system-packages"
fi
for step in $USER_STEPS; do
  case "$step" in
    node) install_node_user || FAILED="$FAILED node" ;;
    rust) install_rust_user || FAILED="$FAILED rust" ;;
  esac
done

# Verify with the freshly installed user-level tools on PATH.
PATH="$HOME/.local/bin:$HOME/.cargo/bin:$PATH"
export PATH
hash -r
run_checks
VERIFY="$(req_summary)"
STILL_MISSING="$(req_count_missing required)"

SUMMARY="$VERIFY"
if [ -n "$SETUP_PATH_HINTS" ]; then
  SUMMARY="$SUMMARY

Open a new terminal, or run:
$SETUP_PATH_HINTS"
fi
if [ "$STILL_MISSING" = 0 ] && [ -z "$FAILED" ]; then
  if [ "$PROFILE" = dev ]; then
    SUMMARY="$SUMMARY

Next: npm ci && npm run tauri -- dev"
  fi
  ui_message "Finished" "Setup complete.

$SUMMARY"
  exit "$EXIT_OK"
fi
ui_message "Finished with problems" "Some steps did not complete:${FAILED:- verification}

$SUMMARY"
exit "$EXIT_MISSING"
