# TORdex Architecture - Layer-by-Layer

> **Think of TORdex as a digital intelligence factory.**
> Each layer has a specific responsibility. Data enters at the bottom as raw information and leaves at the top as actionable intelligence.

---

## Layer 0: Sources

**This is where all information originates.**

Sources include:
- Websites
- Dark web onion services
- APIs
- Repositories
- PDFs and documents
- RSS feeds
- Browser history
- Local files
- Research papers

**Key Principle:** TORdex does not create information here. It simply **observes and collects** what already exists. This layer represents the **outside world**.

---

## Layer 1: Collection Fabric

**Responsible for gathering information from sources.**

The Collection Fabric decides how content should be collected:
- Simple websites → Normal HTTP requests
- JavaScript-heavy pages → Rendered using **Lightpanda**
- Highly complex sites → Fall back to **Chromium**
- Specialized collectors handle repositories, RSS feeds, files, and other data sources

**Goal:** Efficiently acquire information while respecting rate limits and source constraints.

---

## Layer 2: Collection Sessions

**Every collection operation becomes a session.**

A session records:
- What was collected
- When it was collected
- How it was collected
- Which collector was used

**Purpose:** Creates **accountability and traceability**.

If a page changes or a bug appears later, you can trace everything back to the exact collection session that produced the data.

---

## Layer 3: Evidence Lake

**The digital equivalent of an evidence locker.**

Everything collected is preserved in its **original form**:
- HTML pages
- Screenshots
- PDFs
- HAR files
- HTTP headers
- Network metadata
- Documents

**Key Principle:** Nothing is modified. This ensures the platform can always **reconstruct history and verify findings**.

> Evidence is the foundation upon which **trust** is built.

---

## Layer 4: Event Platform

**Records everything that happens inside the system.**

Events are generated for:
- Discovering a page
- Extracting an entity
- Detecting a new endpoint
- Creating a relationship
- Any state change or action

**Purpose:** Provides a **complete history** of how knowledge evolved over time.

This allows:
- Replaying the system
- Auditing decisions
- Recovering from processing mistakes

---

## Layer 5: Processing Fabric

**Transforms raw evidence into structured information.**

Processing tasks include:
- Extracting text from HTML and PDFs
- Identifying entities
- Discovering endpoints
- Analyzing repositories
- Computing fingerprints
- Detecting versions
- Removing duplicates
- Normalizing content

**Goal:** Convert messy data into something the system can understand.

---

## Layer 6: Knowledge Core

**The central repository of truth.**

Stores normalized objects:
- Websites
- Pages
- Repositories
- APIs
- Entities
- Technologies
- Organizations
- Services
- Datasets

**Key Features:**
- Every object has an **identity**
- Every object has **history**
- Every object has **metadata**
- Every object has **provenance**

This layer represents **what the system currently knows about the world**.

---

## Layer 7: Knowledge DNA

**Gives every object a unique identity through fingerprinting.**

Uses:
- Hashes
- Similarity algorithms

**Capabilities:**
- Recognize duplicates
- Track changes
- Cluster related content
- Identify relationships between seemingly unrelated objects

**Purpose:** Prevents the knowledge base from becoming cluttered with **redundant information**.

---

## Layer 8: Correlation Engine

**Finds connections that are not immediately obvious.**

Looks for:
- Shared content
- Common technologies
- Overlapping entities
- Similar behaviors
- Temporal patterns

**Significance:** This is where isolated pieces of information begin to form a **coherent picture**.

> Correlation is the first step from **information** toward **intelligence**.

---

## Layer 9: Temporal Intelligence Graph

**Stores knowledge and relationships across time.**

Instead of only knowing that two objects are connected, the graph knows:
- **When** the connection existed
- **How** it evolved

**Capabilities:**
- Answer questions about **history**
- Track **evolution**
- Analyze **change**

**Transformation:** Turns the system from a **static database** into a **living model of reality**.

---

## Layer 10: Repository Intelligence

**Analyzes software projects at a deep level.**

Understands:
- Repositories
- Workspaces
- Crates
- Modules
- Traits
- Functions
- Endpoints
- Services

**Approach:** Rather than treating source code as text, it treats software as a **structured system**.

**Enables:**
- Architecture analysis
- Dependency tracking
- Documentation generation
- Impact analysis

---

## Layer 11: API Observatory

**Focuses entirely on APIs.**

Capabilities:
- Discovers endpoints
- Extracts schemas
- Tracks authentication methods
- Identifies consumers and producers
- Monitors version changes

**Outcome:** Over time, builds a **complete history** of how APIs evolve.

> This layer effectively becomes a **private observatory** for understanding software interfaces.

---

## Layer 12: Search Engine

**Provides access to the knowledge stored within the platform.**

Search capabilities:
- Keywords
- Semantic meaning
- Graph relationships
- Time ranges
- Structural characteristics

**Significance:** Search is the **primary gateway** into the intelligence ecosystem.

---

## Layer 13: Intelligence Engine

**Performs higher-level reasoning.**

Capabilities:
- Discovers patterns
- Identifies trends
- Detects anomalies
- Finds similarities
- Infers relationships
- Generates recommendations

**Transformation:** Transforms **structured knowledge** into **meaningful insights**.

> This is where the platform begins to **think about information** rather than simply store it.

---

## Layer 14: Monitoring Engine

**Continuously watches for changes.**

Users can monitor:
- Keywords
- Entities
- Websites
- Repositories
- APIs
- Technologies
- Entire topics

**Actions:** When something changes, the system generates:
- Alerts
- Reports
- Notifications

**Transformation:** Turns the platform into an **active intelligence system** rather than a passive archive.

---

## Layer 15: Investigation Workspace

**Where analysts work.**

Investigations are organized into **cases** containing:
- Evidence
- Entities
- Timelines
- Notes
- Reports
- Relationships

**Purpose:** Provides the tools necessary to:
- Conduct research
- Build narratives
- Manage investigations over long periods of time

---

## Layer 16: Intelligence Products

**The outputs produced by the system.**

Product types:
- Daily briefings
- Investigation reports
- Architecture analyses
- API change reports
- Monitoring summaries

**Purpose:** Packages knowledge into formats that **humans can consume and act upon**.

---

## Layer 17: Agent Runtime

**Hosts automated agents that operate on the knowledge base.**

Agent types:
- **Research agents** → Answer questions
- **Architecture agents** → Analyze codebases
- **Monitoring agents** → Track changes
- **Documentation agents** → Generate reports
- **Curator agents** → Improve data quality

**Purpose:** Automates **repetitive analytical work**.

---

## Layer 18: Decision Engine

**Sits above all intelligence layers.**

**Purpose:** Help users answer critical questions:
- What changed?
- What matters?
- What poses risk?
- What should be investigated next?

**Approach:** Combines evidence, history, relationships, and intelligence to **support decision-making**.

---

## Layer 19: Intelligence Console

**The user-facing interface.**

Integrates:
- Search
- Graph exploration
- Timelines
- Monitoring dashboards
- Investigations
- Reports
- Repository analysis
- API observatories
- AI assistance

**Result:** A **single workspace** that users interact with every day.

---

## Summary

The 20-layer architecture transforms **raw data** → **structured information** → **actionable intelligence** through a systematic, traceable, and automated process.

Each layer builds upon the previous one, creating a **living intelligence graph** that enables deep analysis, monitoring, investigation, and decision-making.
