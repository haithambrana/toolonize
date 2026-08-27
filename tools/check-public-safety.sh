#!/usr/bin/env bash
# Public-safety checker — generic, no personal data embedded.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

fail=0

say() { echo "[public-safety] $*"; }
err() { echo "[public-safety] ERROR: $*" >&2; fail=1; }

# 1. Private key / credential headers (case-insensitive)
#    Exclude build artifacts and dependencies (node_modules, target, dist, gen).
if grep -R -I -n -i "BEGIN.*PRIVATE KEY" -- . 2>/dev/null | grep -v ".git/" | grep -v "node_modules/" | grep -v "/target/" | grep -v "/dist/" | grep -v "src-tauri/gen" | grep -v "check-public-safety.sh" | grep -q .; then
  err "private key header found"
  grep -R -I -n -i "BEGIN.*PRIVATE KEY" -- . 2>/dev/null | grep -v ".git/" | grep -v "node_modules/" | grep -v "/target/" | grep -v "/dist/" | grep -v "src-tauri/gen" | head -20 >&2
fi

# 2. AWS-style / generic secret patterns (narrow, not docs prose about the pattern name itself)
if grep -R -I -n "AKIA[0-9A-Z]\{16\}" -- . 2>/dev/null | grep -v ".git/" | grep -v "node_modules/" | grep -v "/target/" | grep -v "/dist/" | grep -v "src-tauri/gen" | grep -q .; then
  err "AKIA-style token found"
fi
if grep -R -I -n "BEGIN OPENSSH PRIVATE KEY" -- . 2>/dev/null | grep -v ".git/" | grep -v "node_modules/" | grep -v "/target/" | grep -v "/dist/" | grep -v "src-tauri/gen" | grep -q .; then
  err "OpenSSH private key header found"
fi

# 3. Suspicious real-looking IPv4 (allow RFC5737 doc ranges)
#    Flag 10.x, 172.16-31.x, 192.168.x that look like real addresses outside docs prose about RFC ranges.
#    We allow 192.0.2.x, 198.51.100.x, 203.0.113.x and 2001:db8:: explicitly.
#    Exclude build artifacts and dependencies.
if grep -R -I -n -E "[0-9]{1,3}\.[0-9]{1,3}\.[0-9]{1,3}\.[0-9]{1,3}" -- . 2>/dev/null \
  | grep -v ".git/" | grep -v "node_modules/" | grep -v "/target/" | grep -v "/dist/" | grep -v "src-tauri/gen" \
  | grep -v "192\.0\.2\." | grep -v "198\.51\.100\." | grep -v "203\.0\.113\." \
  | grep -v "127\.0\.0\.1" | grep -v "0\.0\.0\.0" \
  | grep -E "10\.[0-9]+\.[0-9]+\.[0-9]+|172\.(1[6-9]|2[0-9]|3[0-1])\.[0-9]+\.[0-9]+|192\.168\.[0-9]+\.[0-9]+" \
  | grep -q .; then
  # Only error if not inside FIXTURE_POLICY or safety checker allowlist comments
  hits=$(grep -R -I -n -E "[0-9]{1,3}\.[0-9]{1,3}\.[0-9]{1,3}\.[0-9]{1,3}" -- . 2>/dev/null \
    | grep -v ".git/" | grep -v "node_modules/" | grep -v "/target/" | grep -v "/dist/" | grep -v "src-tauri/gen" | grep -v "192\.0\.2\." | grep -v "198\.51\.100\." | grep -v "203\.0\.113\." \
    | grep -E "10\.[0-9]+\.[0-9]+\.[0-9]+|172\.(1[6-9]|2[0-9]|3[0-1])\.[0-9]+\.[0-9]+|192\.168\.[0-9]+\.[0-9]+")
  if echo "$hits" | grep -v "FIXTURE_POLICY\|check-public-safety" | grep -v "node_modules/" | grep -v "/target/" | grep -v "/dist/" | grep -v "src-tauri/gen" | grep -q .; then
    err "suspicious private-range IPv4 found (use RFC5737 examples)"
    echo "$hits" | grep -v "FIXTURE_POLICY\|check-public-safety" | grep -v "node_modules/" | grep -v "/target/" | grep -v "/dist/" | grep -v "src-tauri/gen" | head -20 >&2
  fi
fi

# 4. Personal absolute home paths like /home/<name>/ that are not example.com fixtures
if grep -R -I -n -E "/home/[^/]+/" -- . 2>/dev/null | grep -v ".git/" | grep -v "node_modules/" | grep -v "/target/" | grep -v "/dist/" | grep -v "src-tauri/gen" | grep -v "check-public-safety" | grep -q .; then
  # Allow if it's inside a comment about what NOT to do, but flag real paths
  # Heuristic: flag lines that look like actual filesystem references
  if grep -R -I -n -E "/home/[a-zA-Z0-9._-]+/(Desktop|Documents|projects|\.ssh|\.config)" -- . 2>/dev/null | grep -v ".git/" | grep -v "node_modules/" | grep -v "/target/" | grep -v "/dist/" | grep -v "src-tauri/gen" | grep -v "check-public-safety" | grep -q .; then
    err "personal absolute home path found"
    grep -R -I -n -E "/home/[a-zA-Z0-9._-]+/(Desktop|Documents|projects|\.ssh|\.config)" -- . 2>/dev/null | grep -v ".git/" | grep -v "node_modules/" | grep -v "/target/" | grep -v "/dist/" | grep -v "src-tauri/gen" | head -20 >&2
  fi
fi

# 5. Environment secret files committed
for f in .env credentials.json secrets.json "*.pem" "*.key" "*.p12" "*.pfx"; do
  # shellcheck pattern: check literal files that shouldn't be tracked
  if [ -f "$f" ] 2>/dev/null; then
    case "$f" in
      .env|.env.*|*.pem|*.key) err "secret file present at repo root: $f" ;;
    esac
  fi
done
if [ -d "secrets" ] || [ -d "private" ] || [ -d ".ssh" ]; then
  if [ -d "secrets" ]; then err "secrets/ directory present"; fi
  if [ -d "private" ]; then err "private/ directory present"; fi
fi

# 6. M1 application artifact guard (M0 prohibition removed)
#    Legitimate M1 sources are allowed: src/, src-tauri/, package.json,
#    package-lock.json, Cargo manifests/locks. Only verify that
#    untracked local artifacts are gitignored, not committed.
#    Secret/privacy detection above remains authoritative.
#
#    Ensure no blocked lockfiles are tracked (pnpm/yarn/bun).
for f in pnpm-lock.yaml yarn.lock bun.lockb; do
  if [ -e "$f" ]; then err "blocked lockfile present (use npm): $f"; fi
done
# Ensure build artifacts are not tracked (they must be gitignored)
# Use git ls-files to check tracked files only (local untracked target/ is okay if ignored)
if git ls-files --error-unmatch target 2>/dev/null | grep -q .; then
  err "target/ should not be tracked (gitignored)"
fi
if git ls-files --error-unmatch node_modules 2>/dev/null | grep -q .; then
  err "node_modules/ should not be tracked (gitignored)"
fi
if git ls-files --error-unmatch dist 2>/dev/null | grep -q .; then
  err "dist/ should not be tracked (gitignored)"
fi

if [ "$fail" -eq 0 ]; then
  say "clean"
  exit 0
else
  say "FAILED"
  exit 1
fi
