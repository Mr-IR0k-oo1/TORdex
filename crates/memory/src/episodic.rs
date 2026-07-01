use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use ulid::Ulid;

use tordex_core::event_store::{EventEnvelope, EventStore};

/// An episode — a temporally-grounded sequence of events.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Episode {
    pub id: Ulid,
    pub kind: String,
    pub summary: String,
    pub events: Vec<Ulid>,
    pub actors: Vec<String>,
    pub entities: Vec<String>,
    pub importance: f64,
    pub started_at: OffsetDateTime,
    pub ended_at: OffsetDateTime,
    pub context: std::collections::HashMap<String, String>,
    pub metadata: std::collections::HashMap<String, String>,
}

/// Query for episodic memory.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EpisodeQuery {
    pub kinds: Option<Vec<String>>,
    pub actors: Option<Vec<String>>,
    pub entities: Option<Vec<String>>,
    pub from: Option<OffsetDateTime>,
    pub to: Option<OffsetDateTime>,
    pub min_importance: f64,
    pub limit: usize,
}

impl Default for EpisodeQuery {
    fn default() -> Self {
        Self {
            kinds: None,
            actors: None,
            entities: None,
            from: None,
            to: None,
            min_importance: 0.0,
            limit: 100,
        }
    }
}

/// Episodic Memory — temporally-grounded event sequences.
///
/// - Stores enriched episodes (sequences of related events)
/// - Wraps EventStore for canonical persistence
/// - Supports temporal replay and pattern matching
/// - Importance-weighted retrieval
pub trait EpisodicMemory: Send + Sync {
    fn record(&mut self, episode: Episode) -> Result<Ulid, String>;
    fn record_from_events(&mut self, events: &[EventEnvelope], kind: &str) -> Result<Episode, String>;
    fn replay(&self, query: &EpisodeQuery) -> Result<Vec<Episode>, String>;
    fn get(&self, id: Ulid) -> Result<Option<Episode>, String>;
    fn timeline(&self, from: OffsetDateTime, to: OffsetDateTime, limit: usize) -> Result<Vec<Episode>, String>;
    fn delete(&mut self, id: Ulid) -> Result<(), String>;
    fn clear(&mut self) -> Result<(), String>;
}

/// Default episodic memory backed by the EventStore.
pub struct DefaultEpisodicMemory {
    event_store: Box<dyn EventStore>,
    episodes: std::collections::HashMap<Ulid, Episode>,
}

impl DefaultEpisodicMemory {
    pub fn new(event_store: Box<dyn EventStore>) -> Self {
        Self {
            event_store,
            episodes: std::collections::HashMap::new(),
        }
    }

    pub fn build_episode(
        events: &[EventEnvelope],
        kind: &str,
    ) -> Episode {
        let now = OffsetDateTime::now_utc();
        let mut actors = Vec::new();
        let mut entities = Vec::new();
        let start = events.first().map(|e| e.timestamp).unwrap_or(now);
        let end = events.last().map(|e| e.timestamp).unwrap_or(now);
        let event_ids: Vec<Ulid> = events.iter().map(|e| e.id).collect();

        for ev in events {
            if let Some(actor) = ev.metadata.get("actor") {
                if !actors.contains(actor) {
                    actors.push(actor.clone());
                }
            }
            let agg = &ev.aggregate_id;
            if !entities.contains(agg) {
                entities.push(agg.clone());
            }
        }

        Episode {
            id: Ulid::new(),
            kind: kind.to_string(),
            summary: format!("{} events of type {}", events.len(), kind),
            events: event_ids,
            actors,
            entities,
            importance: events.len() as f64 * 0.1,
            started_at: start,
            ended_at: end,
            context: std::collections::HashMap::new(),
            metadata: std::collections::HashMap::new(),
        }
    }
}

impl EpisodicMemory for DefaultEpisodicMemory {
    fn record(&mut self, episode: Episode) -> Result<Ulid, String> {
        let id = episode.id;
        let event = EventEnvelope::new(
            id.to_string(),
            "EpisodicMemory",
            "EpisodeRecorded",
            1,
            serde_json::to_value(&episode).map_err(|e| e.to_string())?,
        );
        self.event_store
            .append(event)
            .map_err(|e| e.to_string())?;
        self.episodes.insert(id, episode);
        Ok(id)
    }

    fn record_from_events(&mut self, events: &[EventEnvelope], kind: &str) -> Result<Episode, String> {
        let episode = Self::build_episode(events, kind);
        let id = episode.id;
        self.event_store
            .append(
                EventEnvelope::new(
                    id.to_string(),
                    "EpisodicMemory",
                    "EpisodeRecorded",
                    1,
                    serde_json::to_value(&episode).map_err(|e| e.to_string())?,
                ),
            )
            .map_err(|e| e.to_string())?;
        self.episodes.insert(id, episode.clone());
        Ok(episode)
    }

    fn replay(&self, query: &EpisodeQuery) -> Result<Vec<Episode>, String> {
        let mut results: Vec<Episode> = self
            .episodes
            .values()
            .filter(|e| e.importance >= query.min_importance)
            .filter(|e| query.kinds.as_ref().map_or(true, |k| k.contains(&e.kind)))
            .filter(|e| query.actors.as_ref().map_or(true, |a| {
                e.actors.iter().any(|ea| a.contains(ea))
            }))
            .filter(|e| query.entities.as_ref().map_or(true, |en| {
                e.entities.iter().any(|ee| en.contains(ee))
            }))
            .filter(|e| query.from.map_or(true, |f| e.started_at >= f))
            .filter(|e| query.to.map_or(true, |t| e.ended_at <= t))
            .cloned()
            .collect();

        results.sort_by(|a, b| b.started_at.cmp(&a.started_at));
        results.truncate(query.limit);
        Ok(results)
    }

    fn get(&self, id: Ulid) -> Result<Option<Episode>, String> {
        Ok(self.episodes.get(&id).cloned())
    }

    fn timeline(&self, from: OffsetDateTime, to: OffsetDateTime, limit: usize) -> Result<Vec<Episode>, String> {
        let query = EpisodeQuery {
            from: Some(from),
            to: Some(to),
            limit,
            ..Default::default()
        };
        self.replay(&query)
    }

    fn delete(&mut self, id: Ulid) -> Result<(), String> {
        self.episodes
            .remove(&id)
            .map(|_| ())
            .ok_or_else(|| format!("episode {id} not found"))
    }

    fn clear(&mut self) -> Result<(), String> {
        self.episodes.clear();
        Ok(())
    }
}
