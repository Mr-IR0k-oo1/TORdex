#!/usr/bin/env bash
# Tear down the TORdex local development stack.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"

cd "$ROOT_DIR"

echo "==> Stopping TORdex stack..."
docker compose down

echo "==> Done."
