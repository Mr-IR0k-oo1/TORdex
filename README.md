# TORDEX

> **Open-source intelligence platform for collecting, correlating, searching, and investigating information across websites, repositories, APIs, documents, and archives.**

**Local First • Privacy First • Rust Native**

[🚀 Quick Start](#-quick-start) | [📖 Documentation](./docs) | [🏗️ Architecture](./docs/architecture.md)

---

## What is TORDEX?

TORDEX is an **open-source intelligence operating system**.

It continuously collects information from websites, repositories, APIs, documents, archives, and external sources, transforming them into a **searchable, navigable intelligence graph**.

Think:
- Palantir for individuals and small teams
- Maltego with modern automation
- Sourcegraph for knowledge
- OpenCTI beyond cybersecurity

---

## Why TORDEX?

Modern intelligence workflows are fragmented.

Analysts use browser tabs, search engines, PDFs, Git repositories, APIs, and spreadsheets — none of which talk to each other.

**TORDEX unifies them into a single intelligence graph.**

**Collect. Correlate. Investigate. Automate.**

---

## Core Capabilities

### 📥 Collection
Websites, APIs, repositories, documents, archives — all in one place.

### 🧠 Intelligence
Entity extraction, correlation, similarity analysis, temporal tracking.

### 🔍 Investigation
Case workspaces, evidence linking, timeline reconstruction.

### 🔎 Search
Full-text, semantic search, knowledge graph traversal.

### ⚡ Automation
Monitoring, alerting, agents, reports.

---

## 🚀 Quick Start

### Prerequisites
- Rust 1.82+ (we pin `rust-version = "1.82"`)
- Docker + Docker Compose v2 (for the local backing services)

### 1. Start the backing services
```bash
cp .env.example .env
./scripts/dev-up.sh
```
This brings up PostgreSQL, Redis, MinIO, and Qdrant with healthchecks.

### 2. Build and run
```bash
cargo build --release
./target/release/tordex
```
The server listens on `0.0.0.0:8080` (configurable via `TORDEX_HTTP_BIND`).
The Prometheus metrics endpoint listens on `:9100`.

### 3. Smoke test
```bash
# Create a source
curl -X POST http://localhost:8080/sources \
  -H 'content-type: application/json' \
  -d '{"kind":"website","display_name":"Example","locator":"https://example.org","routing_policy":"auto"}'
# Returns: {"id":"01H...","kind":"website",...}

# Start a collection
curl -X POST http://localhost:8080/collections \
  -H 'content-type: application/json' \
  -H 'Idempotency-Key: smoke-1' \
  -d '{"source_id":"01H..."}'
# Returns: 202 {"collection_id":"01H...","status":"accepted"}

# Poll the result
curl http://localhost:8080/collections/01H...
# Returns: status + result metadata; collector_used should be "http"
```

### 4. Verify the event on Redis
```bash
docker compose exec redis redis-cli XRANGE tordex:events - +
```
You should see a `collection.completed` entry.

### 5. Tear down
```bash
./scripts/dev-down.sh
```

### Integration tests
```bash
./scripts/test-integration.sh
```
This spins up the stack, runs `cargo test --workspace`, and tears down.

---

## 🛠️ Development

| Command | Purpose |
|---|---|
| `cargo check --workspace` | Type-check the whole workspace |
| `cargo clippy --workspace --all-targets -- -D warnings` | Lint cleanly |
| `cargo test --workspace` | Run the test suite (needs services running) |
| `cargo build --release --features tordex-collection/browser` | Build with the optional browser collector |
| `./scripts/dev-up.sh` / `./scripts/dev-down.sh` | Manage the local stack |

---

## Project Status

| Status | Component |
|--------|-----------|
| ✅ Implemented | Layer 0 — Sources |
| ✅ Implemented | Layer 1 — Collection Fabric (HTTP collector; Auto escalation) |
| ✅ Implemented | Event transport (Redis Streams; in-memory fallback) |
| 🚧 Partial | Layer 2 — Collection Sessions (stub) |
| 🚧 Partial | Layer 3 — Evidence Lake (stub) |
| 🚧 Partial | Layer 4 — Event Platform (stub) |
| 📋 Planned | Layers 5–19 (Processing, Knowledge Core, …, Intelligence Console) |

The 20-layer roadmap is documented in [`docs/architecture.md`](./docs/architecture.md).

---

## Workspace layout

```
TORdex/
├── Cargo.toml                 # workspace manifest
├── docker-compose.yml         # local dev stack (postgres/redis/minio/qdrant)
├── .env.example
├── migrations/                # sqlx versioned SQL (0001 sources, 0002 collections)
├── scripts/                   # dev-up, dev-down, test-integration
└── crates/
    ├── tordex-core/           # IDs, time, config, errors
    ├── tordex-events/         # EventBus trait + Redis/InMemory impls
    ├── tordex-sources/        # Layer 0
    ├── tordex-collection/     # Layer 1
    ├── tordex-sessions/       # Layer 2 (stub)
    ├── tordex-evidence/       # Layer 3 (stub)
    ├── tordex-event-platform/ # Layer 4 (stub)
    └── tordex-bin/            # main binary
```

---

## Built With

- **Core:** Rust, Tokio
- **API:** Axum
- **Database:** PostgreSQL (sqlx)
- **Search:** Qdrant
- **Storage:** MinIO
- **Messaging:** Redis Streams
- **Browser (optional):** chromiumoxide against Lightpanda/Chromium CDP
- **Metrics:** Prometheus exporter
- **UI (planned):** Leptos, Tauri

---

## Documentation

- [Architecture](./docs/architecture.md) — Deep dive into the 20-layer stack
- [Technology Stack](./docs/technology-stack.md) — Complete technical breakdown
- [Overview](./docs/index.md) — Quick reference

---

## License

MIT License — Copyright (c) 2024 TORDEX Team

[Full License](LICENSE)

---

TORDEX turns information into intelligence.

Built in Rust. Owned by the user. Designed to scale from individual researchers to intelligence teams.