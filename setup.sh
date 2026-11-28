#!/usr/bin/env bash
# ToolOnize environment setup — a CRMonize.com open-source project.
# Dispatches to the wizard for this OS. Run with --help for options.
set -euo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
case "$(uname -s)" in
  Linux) exec bash "$HERE/scripts/setup/linux.sh" "$@" ;;
  Darwin) exec bash "$HERE/scripts/setup/macos.sh" "$@" ;;
  *)
    echo "On Windows run: powershell -ExecutionPolicy Bypass -File scripts\\setup\\windows.ps1" >&2
    exit 4
    ;;
esac
