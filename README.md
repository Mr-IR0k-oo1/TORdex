# TORdex

> **🔍 Knowledge Intelligence Platform**
> *Transform raw data into actionable intelligence*

---

## ⭐ What is TORdex?

TORdex is a **Knowledge Intelligence Platform** that ingests massive amounts of unstructured information and transforms it into a living, searchable, and actionable intelligence graph.

### 🎯 Mission

> Transform massive amounts of unstructured information into a living intelligence graph that can be **searched, analyzed, monitored, investigated, and automated**.

---

## 🏗️ Architecture Philosophy

TORdex draws inspiration from the best intelligence platforms:

| Platform | Inspiration |
|----------|--------------|
| Palantir Gotham | Large-scale data integration |
| Palantir Foundry | Knowledge graph construction |
| Maltego | Entity relationship mapping |
| Recorded Future | Threat intelligence |
| OpenCTI | Collaborative threat intelligence |
| MISP | Information sharing |
| Shodan | Internet asset discovery |
| Wayback Machine | Historical preservation |
| Sourcegraph | Code intelligence |
| Common Crawl | Web-scale crawling |
| Tor Browser | Privacy-first access |

**While remaining:**
- ✅ **Local First** - All data stays on your infrastructure
- ✅ **Privacy First** - No telemetry, complete control
- ✅ **Event Driven** - Real-time processing pipeline
- ✅ **Knowledge Centric** - Intelligence graph at the core
- ✅ **Rust Native** - Performance, safety, and reliability

---

## 📊 The Intelligence Pipeline (20 Layers)

```
┌─────────────────────────────────────────────────────────────────┐
│                        USER INTERFACE                             │
│  Layer 19: Intelligence Console                                    │
├─────────────────────────────────────────────────────────────────┤
│                        DECISION & OUTPUT                           │
│  Layer 18: Decision Engine      Layer 17: Agent Runtime           │
│  Layer 16: Intelligence Products Layer 15: Investigation Space   │
├─────────────────────────────────────────────────────────────────┤
│                       INTELLIGENCE CORE                            │
│  Layer 14: Monitoring Engine     Layer 13: Intelligence Engine    │
│  Layer 12: Search Engine         Layer 11: API Observatory        │
│  Layer 10: Repository Intelligence                               │
├─────────────────────────────────────────────────────────────────┤
│                        KNOWLEDGE GRAPH                             │
│  Layer 9: Temporal Intelligence Graph                            │
│  Layer 8: Correlation Engine      Layer 7: Knowledge DNA          │
│  Layer 6: Knowledge Core                                          │
├─────────────────────────────────────────────────────────────────┤
│                         DATA PROCESSING                            │
│  Layer 5: Processing Fabric      Layer 4: Event Platform          │
│  Layer 3: Evidence Lake           Layer 2: Collection Sessions     │
│  Layer 1: Collection Fabric      Layer 0: Sources                 │
└─────────────────────────────────────────────────────────────────┘
```

### 📈 Data Flow

```
Sources → Collection → Evidence → Processing → Knowledge → Intelligence → Action
     ↓          ↓          ↓           ↓           ↓           ↓        ↓
  Layer 0    Layer 1     Layer 3      Layer 5      Layer 6     Layer 13   Layer 19
```

**Raw Data In** → **Intelligence Out**

---

## 🛠️ Technology Stack

| Layer | Technologies |
|-------|--------------|
| **Core** | Rust, Tokio |
| **API** | Axum |
| **Database** | PostgreSQL |
| **Storage** | MinIO (S3-compatible) |
| **Search** | Qdrant (Vector) |
| **Messaging** | Redis Streams |
| **Crawling** | Reqwest, Lightpanda, Chromium + Playwright |
| **Code Analysis** | Git2, Tree-sitter, Syn |
| **AI/ML** | Candle.rs, ONNX Runtime, Ollama |
| **Observability** | OpenTelemetry, Prometheus, Grafana |
| **Frontend** | Leptos, Tauri |

---

## 🚀 Quick Start

### Prerequisites
- Rust (latest stable)
- PostgreSQL
- Redis
- MinIO (or S3-compatible storage)
- Qdrant (optional, for vector search)

### Installation
```bash
# Clone the repository
git clone https://github.com/your-org/tordex
cd tordex

# Build
cargo build --release

# Run
./target/release/tordex
```

*Detailed setup instructions coming soon!*

---

## 📚 Documentation

| Document | Description |
|----------|-------------|
| [📖 Architecture](./docs/architecture.md) | Deep dive into all 20 layers |
| [💻 Technology Stack](./docs/technology-stack.md) | Complete tech breakdown |
| [🏠 Overview](./docs/index.md) | Quick reference and links |

---

## 🎯 Use Cases

- **Threat Intelligence** - Monitor dark web, APIs, and repositories for security threats
- **Competitive Intelligence** - Track competitor technology stacks and changes
- **Research Automation** - Automate data collection and analysis pipelines
- **Compliance Monitoring** - Ensure regulatory compliance through continuous monitoring
- **Investigative Journalism** - Build evidence-based narratives from disparate data sources
- **Code Intelligence** - Deep analysis of software projects and dependencies

---

## 🔒 Privacy & Security

- **Local First**: All data processing happens on your infrastructure
- **No Telemetry**: We don't collect any usage data
- **Respectful Crawling**: Honors robots.txt and rate limits
- **Encryption**: Data at rest and in transit can be encrypted
- **Access Control**: Fine-grained permissions and audit trails

---

## 🤖 Agents

TORdex includes an **Agent Runtime** (Layer 17) that hosts automated agents:

| Agent Type | Function |
|------------|----------|
| Research Agents | Answer complex questions using the knowledge graph |
| Architecture Agents | Analyze codebases and software architecture |
| Monitoring Agents | Track changes and generate alerts |
| Documentation Agents | Generate reports and summaries |
| Curator Agents | Improve data quality and remove duplicates |

---

## 📊 Key Features

### Collection
- Multi-strategy crawling (HTTP, headless, full browser)
- Session tracking and accountability
- Rate limiting and polite crawling
- Evidence preservation (HTML, PDFs, screenshots, HAR files)

### Processing
- Text extraction from HTML/PDF
- Entity recognition and extraction
- Endpoint discovery
- Repository analysis
- Fingerprinting and deduplication

### Intelligence
- Temporal graph with version history
- Correlation and pattern detection
- Semantic and keyword search
- Anomaly detection
- Automated reporting

### Workspace
- Investigation management
- Case tracking with timelines
- Collaborative research
- AI-assisted analysis
- Custom dashboard creation

---

## 🌟 Why TORdex?

| Feature | TORdex | Alternatives |
|---------|--------|--------------|
| **Local Deployment** | ✅ Yes | ❌ Cloud-only |
| **Privacy Focus** | ✅ Built-in | ⚠️ Varies |
| **Event Sourcing** | ✅ Full history | ❌ Limited |
| **Extensible** | ✅ Plugin architecture | ⚠️ Limited |
| **Open Source** | ✅ Yes | ❌ Proprietary |
| **Rust-Based** | ✅ Memory safe | ⚠️ Mixed |

---

## 📞 Community & Support

- **Documentation**: [./docs](./docs)
- **Issues**: GitHub Issues (coming soon)
- **Discussions**: GitHub Discussions (coming soon)
- **Contributing**: See [CONTRIBUTING.md](CONTRIBUTING.md) (coming soon)

---

## 📄 License

*License information will be added here.*

---

## 🙏 Acknowledgments

TORdex stands on the shoulders of giants. We're grateful to the open-source community and the teams behind:
- Rust ecosystem
- Tokio and Axum
- PostgreSQL community
- Redis team
- MinIO project
- Qdrant team
- Lightpanda contributors
- Playwright team
- And many more...

---

> **💡 Knowledge is power. Intelligence is action.**
>
> — TORdex Philosophy

---

*Built with ❤️ and Rust*
