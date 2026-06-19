#!/usr/bin/env bash
# Bring up the TORdex local development stack and wait for services to be healthy.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"

cd "$ROOT_DIR"

echo "==> Starting TORdex stack (postgres, redis, minio, qdrant)..."
docker compose up -d

echo "==> Waiting for services to become healthy..."
ATTEMPTS=0
MAX_ATTEMPTS=60
until docker compose ps --format json | python3 -c "
import json, sys
data = [json.loads(line) for line in sys.stdin if line.strip()]
unhealthy = [s['Name'] for s in data if s.get('Health') not in (None, 'healthy')]
print(','.join(unhealthy))
" 2>/dev/null | grep -q '^$'; do
  ATTEMPTS=$((ATTEMPTS + 1))
  if [ "$ATTEMPTS" -ge "$MAX_ATTEMPTS" ]; then
    echo "==> Timeout waiting for services to become healthy."
    docker compose ps
    exit 1
  fi
  sleep 2
done

echo "==> Stack is up."
docker compose ps