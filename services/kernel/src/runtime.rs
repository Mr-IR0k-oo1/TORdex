use std::sync::Arc;

use tokio::time::{interval, Duration};
use tracing::{info, warn};
use ulid::Ulid;

use tordex_core::event_store::EventStore;
use tordex_core::Kernel;

/// The kernel runtime loop — drives agents, dispatches events, runs watchers.
pub struct KernelRuntime {
    kernel: Arc<Kernel>,
    event_store: Box<dyn EventStore>,
    tick_interval_ms: u64,
}

impl KernelRuntime {
    pub fn new(
        kernel: Arc<Kernel>,
        event_store: Box<dyn EventStore>,
        tick_interval_ms: u64,
    ) -> Self {
        Self {
            kernel,
            event_store,
            tick_interval_ms,
        }
    }

    pub async fn run(&self) {
        info!(
            interval_ms = self.tick_interval_ms,
            "kernel runtime loop started"
        );

        let mut last_event_id = Ulid::nil();
        let mut tick_timer = interval(Duration::from_millis(self.tick_interval_ms));

        loop {
            tick_timer.tick().await;

            if let Err(e) = self.tick_agents() {
                warn!(error = %e, "agent tick failed");
            }

            if let Err(e) = self.dispatch_new_events(&mut last_event_id) {
                warn!(error = %e, "event dispatch failed");
            }
        }
    }

    fn tick_agents(&self) -> Result<(), String> {
        tokio::task::block_in_place(|| {
            self.kernel
                .agents
                .tick_all(&self.kernel)
                .map_err(|e| e.to_string())
        })
    }

    fn dispatch_new_events(&self, last_id: &mut Ulid) -> Result<(), String> {
        let new_events = self
            .event_store
            .read_since(*last_id)
            .map_err(|e| e.to_string())?;

        if new_events.is_empty() {
            return Ok(());
        }

        for event in &new_events {
            tokio::task::block_in_place(|| {
                self.kernel
                    .agents
                    .dispatch_event(&self.kernel, event)
                    .ok();
            });
        }

        if let Some(last) = new_events.last() {
            if last.id > *last_id {
                *last_id = last.id;
            }
        }

        Ok(())
    }
}
