use std::sync::Mutex;

use ulid::Ulid;

use tordex_memory::{
    EpisodicMemory, LongTermMemory, ProceduralMemory, SemanticMemory,
    SemanticQuery, WorkingMemory,
};

/// Bridge between the VM and the TORdex memory subsystems.
pub struct MemoryManager {
    working: Mutex<Box<dyn WorkingMemory>>,
    semantic: Mutex<Box<dyn SemanticMemory>>,
    episodic: Mutex<Box<dyn EpisodicMemory>>,
    long_term: Mutex<Box<dyn LongTermMemory>>,
    procedural: Mutex<Box<dyn ProceduralMemory>>,
}

impl MemoryManager {
    pub fn new(
        working: Box<dyn WorkingMemory>,
        semantic: Box<dyn SemanticMemory>,
        episodic: Box<dyn EpisodicMemory>,
        long_term: Box<dyn LongTermMemory>,
        procedural: Box<dyn ProceduralMemory>,
    ) -> Self {
        Self {
            working: Mutex::new(working),
            semantic: Mutex::new(semantic),
            episodic: Mutex::new(episodic),
            long_term: Mutex::new(long_term),
            procedural: Mutex::new(procedural),
        }
    }

    // ─── Working Memory ───────────────────────────────────────────────────

    pub fn wm_set(&self, key: &str, value: serde_json::Value, ttl_secs: u64) -> String {
        self.working.lock().unwrap().set(key, value, ttl_secs)
    }

    pub fn wm_get(&self, key: &str) -> Option<serde_json::Value> {
        self.working.lock().unwrap().get(key)
    }

    // ─── Semantic Memory ──────────────────────────────────────────────────

    pub fn store_semantic_fact(
        &self,
        fact: tordex_memory::SemanticFact,
    ) -> Result<Ulid, String> {
        self.semantic.lock().unwrap().store_fact(fact)
    }

    pub fn query_semantic(&self, query: &SemanticQuery) -> Result<Vec<tordex_memory::SemanticFact>, String> {
        self.semantic.lock().unwrap().query(query)
    }

    // ─── Episodic Memory ──────────────────────────────────────────────────

    pub fn record_episode(
        &self,
        episode: tordex_memory::Episode,
    ) -> Result<Ulid, String> {
        self.episodic.lock().unwrap().record(episode)
    }

    // ─── Long-Term Memory ─────────────────────────────────────────────────

    pub fn store_ltm(&self, entry: tordex_memory::LTEntry) -> Result<Ulid, String> {
        self.long_term.lock().unwrap().store(entry)
    }

    pub fn retrieve_ltm(&self, query: &tordex_memory::LTQuery) -> Result<Vec<tordex_memory::LTEntry>, String> {
        self.long_term.lock().unwrap().retrieve(query)
    }

    // ─── Procedural Memory ────────────────────────────────────────────────

    pub fn execute_procedure(
        &self,
        name: &str,
        input: serde_json::Value,
        session_id: &str,
    ) -> Result<tordex_memory::ProcedureOutcome, String> {
        self.procedural.lock().unwrap().execute(name, input, session_id)
    }
}
