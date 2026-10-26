#!/usr/bin/env bash
# shellcheck source-path=SCRIPTDIR
# ToolOnize environment setup wizard — macOS (preparatory).
# ToolOnize is a CRMonize.com open-source project — https://crmonize.com
#
# macOS is NOT a supported ToolOnize V1 platform (PRD: Linux and Windows).
# This wizard prepares a contributor toolchain for a future port: Xcode
# Command Line Tools, Node.js LTS and Rust. Same flow as Linux: one check,
# one plan, one consent. No step needs sudo: Node.js and Rust install at
# user level, and Apple's own installer handles the Command Line Tools.
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=lib/common.sh
. "$HERE/lib/common.sh"

usage() {
  cat <<'EOF'
Usage: scripts/setup/macos.sh [--check | --dry-run] [--yes] [--no-optional] [--text]

  --check        Report what is present or missing; change nothing (exit 0/1).
  --dry-run      Show the install plan; change nothing.
  --yes          Accept the plan without prompting.
  --no-optional  Do not check optional tools (tmux).
  --text         Plain text prompts.
EOF
}

MODE=install
OPTIONAL=1
while [ $# -gt 0 ]; do
  case "$1" in
    --check) MODE=check ;;
    --dry-run) MODE=dry-run ;;
    --yes | -y) SETUP_YES=1 ;;
    --no-optional) OPTIONAL=0 ;;
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

[ "$(uname -s)" = Darwin ] || die "this wizard is for macOS" "$EXIT_UNSUPPORTED"

run_checks() {
  req_reset
  if xcode-select -p >/dev/null 2>&1; then
    req_add clt "Xcode Command Line Tools (compiler, git)" required ok "$(xcode-select -p)" -
  else
    req_add clt "Xcode Command Line Tools (compiler, git)" required missing "not installed" clt
  fi
  check_node
  check_rust
  if [ "$OPTIONAL" = 1 ]; then
    if command -v tmux >/dev/null 2>&1; then
      req_add opt-tmux "tmux (persistent sessions)" optional ok "found" -
    elif command -v brew >/dev/null 2>&1; then
      req_add opt-tmux "tmux (persistent sessions)" optional missing "not found" brew:tmux
    else
      req_add opt-tmux "tmux (persistent sessions)" optional missing "not found (needs Homebrew: https://brew.sh)" -
    fi
  fi
}

STEPS=""
build_plan() {
  local include_optional="$1" i
  STEPS=""
  for i in "${!REQ_IDS[@]}"; do
    [ "${REQ_STATES[$i]}" = missing ] || continue
    [ "${REQ_LEVELS[$i]}" = optional ] && [ "$include_optional" = 0 ] && continue
    case "${REQ_FIXES[$i]}" in
      clt | node | rust | brew:*) STEPS="$STEPS ${REQ_FIXES[$i]}" ;;
    esac
  done
  STEPS="${STEPS# }"
}

plan_text() {
  local s="" step
  for step in $STEPS; do
    case "$step" in
      clt) s="${s}-  Xcode Command Line Tools: Apple's installer opens (xcode-select --install)
" ;;
      node) s="${s}-  Node.js $TOOLONIZE_NODE_VERSION LTS for your user: SHA-256 verified nodejs.org tarball in ~/.local/opt
" ;;
      rust) s="${s}-  Rust stable for your user: SHA-256 verified rustup-init $TOOLONIZE_RUSTUP_VERSION
" ;;
      brew:*) s="${s}-  brew install ${step#brew:}
" ;;
    esac
  done
  [ -n "$s" ] || s="Nothing to install."
  printf '%s' "$s"
}

run_checks
REPORT="$(req_report)"
MISSING_REQUIRED="$(req_count_missing required)"
MISSING_OPTIONAL="$(req_count_missing optional)"
NOTICE="Note: macOS is not yet a supported ToolOnize platform; this prepares the toolchain only."

if [ "$MODE" = check ]; then
  say "$NOTICE"
  say "$REPORT"
  say ""
  say "Required missing: $MISSING_REQUIRED, optional missing: $MISSING_OPTIONAL"
  [ "$MISSING_REQUIRED" = 0 ] && exit "$EXIT_OK"
  exit "$EXIT_MISSING"
fi

ui_init
if [ "$MODE" = dry-run ]; then
  build_plan "$OPTIONAL"
  say "$NOTICE"
  say "$REPORT"
  say ""
  say "Plan (dry run — nothing will be changed):"
  plan_text
  exit "$EXIT_OK"
fi

ui_page "Welcome" "This wizard prepares this Mac for building ToolOnize, a CRMonize.com open-source project.

$NOTICE

Nothing is changed until you accept the plan."
ui_page "Check results" "$(req_summary)"

if [ "$MISSING_REQUIRED" = 0 ] && [ "$MISSING_OPTIONAL" = 0 ]; then
  ui_message "Done" "Everything is already installed."
  exit "$EXIT_OK"
fi
INCLUDE_OPTIONAL=0
if [ "$MISSING_OPTIONAL" != 0 ] && ui_yesno "Also install the missing optional tools?"; then
  INCLUDE_OPTIONAL=1
fi
build_plan "$INCLUDE_OPTIONAL"
[ -n "$STEPS" ] || {
  ui_message "Done" "Nothing to install."
  exit "$EXIT_OK"
}
ui_confirm "Install plan" "The following will be installed:

$(plan_text)" "Install" || ui_cancelled

FAILED=""
for step in $STEPS; do
  case "$step" in
    clt)
      xcode-select --install || true
      say "Finish Apple's Command Line Tools installer, then rerun this wizard to verify."
      ;;
    node) install_node_user || FAILED="$FAILED node" ;;
    rust) install_rust_user || FAILED="$FAILED rust" ;;
    brew:*) brew install "${step#brew:}" || FAILED="$FAILED ${step#brew:}" ;;
  esac
done

PATH="$HOME/.local/bin:$HOME/.cargo/bin:$PATH"
export PATH
hash -r
run_checks
SUMMARY="$(req_summary)"
[ -n "$SETUP_PATH_HINTS" ] && SUMMARY="$SUMMARY

Open a new terminal, or run:
$SETUP_PATH_HINTS"
if [ "$(req_count_missing required)" = 0 ] && [ -z "$FAILED" ]; then
  ui_message "Finished" "Setup complete.

$SUMMARY"
  exit "$EXIT_OK"
fi
ui_message "Finished with problems" "Some steps did not complete:${FAILED:- verification}

$SUMMARY"
exit "$EXIT_MISSING"
