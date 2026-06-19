#!/usr/bin/env bash
# Run the integration test suite against the docker-compose stack.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"

cd "$ROOT_DIR"

cleanup() {
  echo "==> Tearing down stack..."
  docker compose down
}
trap cleanup EXIT

"$SCRIPT_DIR/dev-up.sh"

echo "==> Running integration tests..."
DATABASE_URL="${DATABASE_URL:-postgres://tordex:tordex@localhost:5432/tordex}" \
REDIS_URL="${REDIS_URL:-redis://localhost:6379}" \
  cargo test --workspace --all-features

echo "==> Integration tests passed."