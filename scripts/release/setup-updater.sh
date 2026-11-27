#!/usr/bin/env bash
# One-time setup for in-app updates (ADR-019), run by the repository owner on
# their own computer. It creates the update signing key OUTSIDE the repository
# and gives GitHub what the release workflow needs:
#   secret   TAURI_SIGNING_PRIVATE_KEY           (the private key)
#   secret   TAURI_SIGNING_PRIVATE_KEY_PASSWORD  (its password)
#   variable TAURI_UPDATER_PUBKEY                (the public key, not secret)
# Requirements: the GitHub CLI (`gh auth login` done) and Node/npm.
set -euo pipefail

dir="${TOOLONIZE_KEY_DIR:-$HOME/.config/toolonize-release}"
key="$dir/updater.key"
root="$(git rev-parse --show-toplevel 2>/dev/null || pwd)"

case "$(realpath -m "$dir")/" in
  "$(realpath -m "$root")"/*)
    echo "Refusing: the key folder must not be inside the repository." >&2
    exit 1
    ;;
esac
command -v gh >/dev/null || { echo "Install the GitHub CLI (gh) and run 'gh auth login' first." >&2; exit 1; }
command -v npm >/dev/null || { echo "Node.js/npm is needed (run ./setup.sh)." >&2; exit 1; }
gh auth status >/dev/null 2>&1 || { echo "Run 'gh auth login' first." >&2; exit 1; }

mkdir -p "$dir"
chmod 700 "$dir"
if [ -e "$key" ]; then
  echo "Using the existing key $key."
  echo "(Making a new key would stop installed copies from accepting new updates.)"
else
  echo "Creating the update signing key. Choose a strong password and keep a backup"
  echo "of $key and the password somewhere safe: without them you cannot publish"
  echo "updates that installed copies accept."
  (cd "$root" && npm run -s tauri -- signer generate -w "$key")
fi
chmod 600 "$key"

read -r -s -p "Password of the key: " password
echo
gh secret set TAURI_SIGNING_PRIVATE_KEY < "$key"
printf '%s' "$password" | gh secret set TAURI_SIGNING_PRIVATE_KEY_PASSWORD
gh variable set TAURI_UPDATER_PUBKEY < "$key.pub"
unset password
echo "Done. The next tagged release can update installed copies once you publish it."
