# TORdex - Technology Stack

> **At its core, TORdex is built on modern, performant, and privacy-focused technologies.**

---

## Core Languages & Runtimes

| Component | Technology | Purpose |
|-----------|------------|---------|
| **Language** | Rust | Primary implementation language |
| **Runtime** | Tokio | Async runtime for Rust |

---

## Web & API Layer

| Component | Technology | Purpose |
|-----------|------------|---------|
| **API Framework** | Axum | Web API framework |

---

## Data Storage Layer

| Component | Technology | Purpose |
|-----------|------------|---------|
| **Relational Database** | PostgreSQL | Structured data storage |
| **Object Storage** | MinIO | Binary/object storage (S3-compatible) |
| **Vector Search** | Qdrant | Vector similarity search |

---

## Messaging & Events

| Component | Technology | Purpose |
|-----------|------------|---------|
| **Queue/Event Transport** | Redis Streams | Event streaming and message queue |

---

## Collection & Crawling

| Component | Technology | Purpose |
|-----------|------------|---------|
| **HTTP Client** | Reqwest | HTTP requests for simple sites |
| **Browser Engine** | Lightpanda | JavaScript rendering for complex pages |
| **Fallback Browser** | Chromium + Playwright | Full browser automation fallback |

---

## Repository Analysis

| Component | Technology | Purpose |
|-----------|------------|---------|
| **Git Library** | Git2 | Git repository operations |
| **Parser Generator** | Tree-sitter | Syntax parsing for code analysis |
| **Syntax Analysis** | Syn | Rust syntax parsing |

---

## Artificial Intelligence

| Component | Technology | Purpose |
|-----------|------------|---------|
| **ML Framework (Rust)** | Candle.rs | Machine learning in Rust |
| **ONNX Runtime** | ONNX Runtime | Cross-platform ML inference |
| **Local LLM** | Ollama | Local large language model execution |

---

## Monitoring & Observability

| Component | Technology | Purpose |
|-----------|------------|---------|
| **Tracing** | OpenTelemetry | Distributed tracing |
| **Metrics** | Prometheus | Metrics collection |
| **Visualization** | Grafana | Metrics visualization and dashboards |

---

## Frontend

| Component | Technology | Purpose |
|-----------|------------|---------|
| **Web Framework** | Leptos | Reactive web framework |
| **Desktop Runtime** | Tauri | Desktop application runtime |

---

## Architecture Summary

```
┌─────────────────────────────────────────────────────────────┐
│                    TORdex Technology Stack                    │
├─────────────────────────────────────────────────────────────┤
│  FRONTEND: Leptos + Tauri                                      │
├─────────────────────────────────────────────────────────────┤
│  API: Axum (Rust + Tokio)                                      │
├─────────────────────────────────────────────────────────────┤
│  AI/ML: Candle.rs + ONNX Runtime + Ollama                       │
├─────────────────────────────────────────────────────────────┤
│  STORAGE: PostgreSQL + MinIO + Qdrant                          │
├─────────────────────────────────────────────────────────────┤
│  MESSAGING: Redis Streams                                      │
├─────────────────────────────────────────────────────────────┤
│  COLLECTION: Reqwest + Lightpanda + Chromium/Playwright       │
├─────────────────────────────────────────────────────────────┤
│  CODE ANALYSIS: Git2 + Tree-sitter + Syn                       │
├─────────────────────────────────────────────────────────────┤
│  OBSERVABILITY: OpenTelemetry + Prometheus + Grafana          │
└─────────────────────────────────────────────────────────────┘
```

---

## Key Characteristics

### ✅ Local First
All processing happens locally. No external cloud dependencies for core functionality.

### ✅ Privacy First
- No telemetry by default
- Data stays on your infrastructure
- Respects source constraints and rate limits

### ✅ Event Driven
- Redis Streams for event transport
- OpenTelemetry for distributed tracing
- Complete audit trail via Event Platform (Layer 4)

### ✅ Knowledge Centric
- PostgreSQL for structured knowledge
- Qdrant for semantic search
- MinIO for evidence preservation

### ✅ Rust Native
- Memory safe
- Zero-cost abstractions
- High performance
- Concurrent by default (Tokio)

---

## Integration Points

The stack is designed for:
- **Extensibility** - New collectors, processors, and agents can be added
- **Scalability** - Async I/O (Tokio) and distributed messaging (Redis)
- **Reliability** - Comprehensive monitoring and tracing
- **Privacy** - Local execution with no external dependencies
