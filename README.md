# TORDEX

> **Open-source intelligence platform for collecting, correlating, searching, and investigating information across websites, repositories, APIs, documents, and archives.**

**Local First • Privacy First • Rust Native**

[🚀 Quick Start](#quick-start) | [📖 Documentation](./docs) | [🏗️ Architecture](./docs/architecture.md)

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

## The Flow

```
GitHub Repo
  + Company Website
  + API Docs
  + Archived Pages
      ↓
  TORDEX
      ↓
  Entity Graph
      ↓
  Investigation Workspace
      ↓
  Actionable Intelligence
```

---

## 🚀 Quick Start

### Prerequisites
- Rust (latest stable)
- PostgreSQL
- Redis
- MinIO (or S3-compatible storage)

### Installation
```bash
git clone https://github.com/your-org/tordex
cd tordex
cargo build --release
./target/release/tordex
```

---

## Current Status

**TORDEX is under active development.**

| Status | Component |
|--------|-----------|
| ✅ Implemented | Collection Framework |
| ✅ Implemented | Evidence Lake |
| ✅ Implemented | Event Platform |
| 🔄 In Progress | Search Engine |
| 🔄 In Progress | Investigation Workspace |
| 📋 Planned | Agent Runtime |
| 📋 Planned | Intelligence Products |

---

## Built With

- **Core:** Rust, Tokio
- **API:** Axum
- **Database:** PostgreSQL
- **Search:** Qdrant
- **Storage:** MinIO
- **Messaging:** Redis Streams
- **UI:** Leptos, Tauri

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
