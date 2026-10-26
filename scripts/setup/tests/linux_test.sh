#!/usr/bin/env bash
# shellcheck disable=SC2016,SC2015 # stub bodies expand inside the stubs; ok() always succeeds
# Tests for scripts/setup/linux.sh. Runs the wizard against stub package
# managers, sudo and downloads in a sandboxed PATH/HOME: no root, no network,
# nothing on the host is changed.
set -uo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WIZARD="$HERE/../linux.sh"
WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT

PASS=0
FAIL=0
ok() {
  PASS=$((PASS + 1))
  printf 'ok   %s\n' "$1"
}
bad() {
  FAIL=$((FAIL + 1))
  printf 'FAIL %s\n' "$1"
  [ -n "${OUT:-}" ] && printf '%s\n' "$OUT" | sed 's/^/     | /'
}

# Core utilities the wizard may use; everything else comes from stubs.
SYSBIN="$WORK/sysbin"
mkdir -p "$SYSBIN"
for tool in bash sh sed awk grep head tr uname mktemp rm mkdir dirname cat tput \
  sha256sum tar ln chmod sort printf env basename cut wc xz; do
  p="$(command -v "$tool" 2>/dev/null)" && ln -s "$p" "$SYSBIN/$tool"
done

# new_case NAME DISTRO — fresh HOME, stub dir and os-release.
new_case() {
  CASE="$WORK/$1"
  mkdir -p "$CASE/home" "$CASE/stubs"
  STUBS="$CASE/stubs"
  LOG="$CASE/log"
  : >"$LOG"
  : >"$CASE/installed"
  case "$2" in
    ubuntu) printf 'ID=ubuntu\nID_LIKE=debian\nPRETTY_NAME="Ubuntu (test)"\n' >"$CASE/os-release" ;;
    fedora) printf 'ID=fedora\nPRETTY_NAME="Fedora (test)"\n' >"$CASE/os-release" ;;
    arch) printf 'ID=arch\nPRETTY_NAME="Arch (test)"\n' >"$CASE/os-release" ;;
    unknown) printf 'ID=exotic\nPRETTY_NAME="Exotic (test)"\n' >"$CASE/os-release" ;;
  esac
  stub id 'if [ "$1" = -u ]; then echo 1000; else echo "uid=1000"; fi'
  stub sudo 'echo "sudo $*" >>"$LOG"; exit 0'
  stub pkexec 'echo "pkexec $*" >>"$LOG"; exit 0'
  stub dpkg-query 'for a; do p="$a"; done; grep -qx "$p" "$CASE/installed" && echo "ii " || exit 1'
  stub rpm 'grep -qx "$2" "$CASE/installed"'
  stub pacman 'grep -qx "$2" "$CASE/installed"'
  stub curl 'out=""; while [ $# -gt 0 ]; do [ "$1" = -o ] && out="$2"; shift; done
echo "curl $out" >>"$LOG"; echo "not the real file" >"$out"'
}

stub() {
  printf '#!/bin/sh\nLOG=%q\nCASE=%q\n%s\n' "$LOG" "$CASE" "$2" >"$STUBS/$1"
  chmod +x "$STUBS/$1"
}

install_pkgs() { printf '%s\n' "$@" >>"$CASE/installed"; }

all_apt_dev() {
  install_pkgs libwebkit2gtk-4.1-0 xdg-utils build-essential pkg-config curl xz-utils file \
    libssl-dev libwebkit2gtk-4.1-dev libjavascriptcoregtk-4.1-dev libgtk-3-dev \
    libayatana-appindicator3-dev librsvg2-dev libsoup-3.0-dev patchelf
}

good_toolchain() {
  stub node 'echo v24.1.0'
  stub npm 'echo 11.0.0'
  stub rustc 'echo "rustc 1.90.0 (abc 2026-01-01)"'
  stub cargo 'echo "cargo 1.90.0"'
}

optional_tools() {
  stub ssh 'exit 0'
  stub tmux 'exit 0'
  stub git 'exit 0'
}

# run ARGS... — run the wizard in the sandbox; sets OUT and RC.
run() {
  OUT="$(env -i HOME="$CASE/home" PATH="$STUBS:$SYSBIN" TERM=dumb \
    TOOLONIZE_SETUP_OS_RELEASE="$CASE/os-release" \
    bash "$WIZARD" "$@" <"${STDIN:-/dev/null}" 2>&1)"
  RC=$?
}

has() { printf '%s\n' "$OUT" | grep -qF -- "$1"; }
sudo_calls() { grep -c '^sudo ' "$LOG" || true; }

# 1. --check passes when everything is present.
new_case check-ok ubuntu
all_apt_dev
good_toolchain
optional_tools
run --check
[ "$RC" = 0 ] && has "Required missing: 0" && ok "check: all present exits 0" || bad "check: all present exits 0 (rc=$RC)"

# 2. --check reports a missing package and exits 1, changes nothing.
new_case check-missing ubuntu
all_apt_dev
sed -i '/^libsoup-3.0-dev$/d' "$CASE/installed"
good_toolchain
optional_tools
run --check
[ "$RC" = 1 ] && has "[MISSING]  libsoup-3.0-dev" && [ "$(sudo_calls)" = 0 ] &&
  ok "check: missing package exits 1 without changes" || bad "check: missing package (rc=$RC)"

# 3. One consent, one authorization: every missing package in ONE sudo call.
new_case install-once ubuntu
install_pkgs libwebkit2gtk-4.1-0 build-essential curl xz-utils
good_toolchain
run --yes
calls="$(sudo_calls)"
line="$(grep '^sudo ' "$LOG" | head -n 1)"
if [ "$calls" = 1 ] && printf '%s' "$line" | grep -q -- "-- sh -c" &&
  printf '%s' "$line" | grep -q " sh xdg-utils pkg-config file libssl-dev" &&
  printf '%s' "$line" | grep -q "openssh-client tmux git" &&
  ! printf '%s' "$line" | grep -q "build-essential"; then
  ok "install: single sudo call with all missing packages as arguments"
else
  bad "install: single sudo call (calls=$calls) $line"
fi

# 4. --no-optional leaves optional tools out of the transaction.
new_case no-optional ubuntu
install_pkgs libwebkit2gtk-4.1-0
good_toolchain
run --yes --no-optional
line="$(grep '^sudo ' "$LOG" | head -n 1)"
printf '%s' "$line" | grep -q "xdg-utils" && ! printf '%s' "$line" | grep -q "tmux" &&
  ok "install: --no-optional skips optional tools" || bad "install: --no-optional ($line)"

# 5. --dry-run prints the plan and never authorizes.
new_case dry-run ubuntu
good_toolchain
run --dry-run
[ "$RC" = 0 ] && has "apt-get update && apt-get install -y" && [ "$(sudo_calls)" = 0 ] &&
  ok "dry-run: plan shown, nothing executed" || bad "dry-run (rc=$RC)"

# 6. A download whose SHA-256 does not match is refused and not installed.
new_case bad-checksum ubuntu
all_apt_dev
optional_tools
stub rustc 'echo "rustc 1.90.0"'
stub cargo 'echo "cargo 1.90.0"'
run --yes
[ "$RC" = 1 ] && has "checksum mismatch" && ! [ -e "$CASE/home/.local/bin/node" ] &&
  ! [ -d "$CASE/home/.local/opt" ] && grep -q '^curl ' "$LOG" &&
  ok "download: checksum mismatch refused, nothing installed" || bad "download: checksum mismatch (rc=$RC)"

# 7. An old Node.js is reported as too old.
new_case old-node ubuntu
all_apt_dev
optional_tools
stub node 'echo v18.19.1'
stub npm 'echo 9.0.0'
stub rustc 'echo "rustc 1.90.0"'
stub cargo 'echo "cargo 1.90.0"'
run --check
[ "$RC" = 1 ] && has "node 18.19.1 is too old" && ok "check: old Node.js flagged" || bad "check: old Node.js (rc=$RC)"

# 8. An old Rust is reported as too old.
new_case old-rust ubuntu
all_apt_dev
optional_tools
stub node 'echo v22.0.0'
stub npm 'echo 10.0.0'
stub rustc 'echo "rustc 1.75.0 (distro)"'
stub cargo 'echo "cargo 1.75.0"'
run --check
[ "$RC" = 1 ] && has "rustc 1.75.0 is too old" && ok "check: old Rust flagged" || bad "check: old Rust (rc=$RC)"

# 9. The run profile needs only runtime packages, no toolchain.
new_case run-profile ubuntu
install_pkgs libwebkit2gtk-4.1-0 xdg-utils
stub ssh 'exit 0'
stub tmux 'exit 0'
run --check --profile run
[ "$RC" = 0 ] && ! has "Node.js" && ! has "libgtk-3-dev" && ok "check: run profile is runtime-only" || bad "check: run profile (rc=$RC)"

# 10. Unknown distributions: check works, install refuses with exit 4.
new_case unknown unknown
rm "$STUBS/pacman" "$STUBS/rpm" "$STUBS/dpkg-query"
good_toolchain
run --check
rc_check=$RC
run --yes
[ "$rc_check" = 1 ] && [ "$RC" = 4 ] && [ "$(sudo_calls)" = 0 ] &&
  ok "unknown distro: check reports, install refuses" || bad "unknown distro (check=$rc_check install=$RC)"

# 11. Text wizard: cancelling on the first page changes nothing.
new_case cancel ubuntu
good_toolchain
printf 'q\n' >"$CASE/in"
STDIN="$CASE/in" run --text
[ "$RC" = 3 ] && [ "$(sudo_calls)" = 0 ] && has "Nothing was changed" &&
  ok "wizard: cancel on first page" || bad "wizard: cancel (rc=$RC)"

# 12. Text wizard: Next, Next, decline the plan -> nothing changed.
new_case decline ubuntu
good_toolchain
optional_tools
printf '\n\nn\n' >"$CASE/in"
STDIN="$CASE/in" run --text
[ "$RC" = 3 ] && [ "$(sudo_calls)" = 0 ] && ok "wizard: declining the plan changes nothing" || bad "wizard: decline (rc=$RC)"

# 13. Text wizard: Next, Next, (optional? yes), Install -> exactly one authorization.
new_case accept ubuntu
good_toolchain
printf '\n\ny\ny\n' >"$CASE/in"
STDIN="$CASE/in" run --text
[ "$(sudo_calls)" = 1 ] && grep -q "tmux" "$LOG" && ok "wizard: next, next, install -> one authorization" ||
  bad "wizard: accept (rc=$RC, sudo=$(sudo_calls))"

# 14. Fedora maps to dnf and rpm.
new_case fedora fedora
good_toolchain
optional_tools
run --yes --profile run
grep -q "dnf install -y" "$LOG" && grep -q "webkit2gtk4.1" "$LOG" && ok "fedora: dnf transaction" || bad "fedora: dnf ($(cat "$LOG"))"

# 15. Arch maps to pacman without a partial upgrade (-Sy).
new_case arch arch
good_toolchain
optional_tools
run --yes --profile run
grep -q "pacman -S --needed --noconfirm" "$LOG" && ! grep -q -- "-Sy" "$LOG" && ok "arch: pacman transaction, no -Sy" || bad "arch: pacman"

# 16. Without sudo, pkexec is used, still exactly once.
new_case pkexec ubuntu
good_toolchain
optional_tools
rm "$STUBS/sudo"
run --yes --profile run
[ "$(grep -c '^pkexec ' "$LOG")" = 1 ] && ok "pkexec fallback: one authorization" || bad "pkexec fallback"

# 17. A foreign ~/.local/bin/node is never replaced.
new_case foreign-node ubuntu
all_apt_dev
optional_tools
stub rustc 'echo "rustc 1.90.0"'
stub cargo 'echo "cargo 1.90.0"'
mkdir -p "$CASE/home/.local/bin"
echo "#!/bin/sh" >"$CASE/home/.local/bin/node"
run --yes
has "was not created by this setup" && ! grep -q '^curl ' "$LOG" && ok "node: foreign binary left alone" || bad "node: foreign binary"

# 18. Bad arguments are usage errors.
new_case usage ubuntu
run --bogus
rc1=$RC
run --profile nope
[ "$rc1" = 2 ] && [ "$RC" = 2 ] && ok "usage errors exit 2" || bad "usage errors ($rc1/$RC)"

printf '\n%d passed, %d failed\n' "$PASS" "$FAIL"
[ "$FAIL" = 0 ]
