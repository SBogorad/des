use super::Engine;
use crate::events::{EventQueue, EventQueueSnapshot, SimEvent};
use crate::network::Network;
use crate::norms::RzdNorms;
use crate::prng::Mulberry32;
use crate::statistics::SimulationStats;
use crate::tu3::Tu3Row;
use crate::violations::{ViolationAnalytics, ViolationAnalyticsSnapshot};
use crate::{Crew, Locomotive};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EngineSnapshot {
    pub schema: String,
    pub network: Network,
    pub norms: RzdNorms,
    pub prng: Mulberry32,
    pub queue: EventQueueSnapshot,
    pub crews: HashMap<String, Crew>,
    pub locos: HashMap<String, Locomotive>,
    pub violations: ViolationAnalyticsSnapshot,
    pub tu3_rows: Vec<Tu3Row>,
    pub stats: SimulationStats,
    pub current_time: u64,
    pub max_time: u64,
    pub warmup_minutes: u64,
    pub loco_by_station: HashMap<String, Vec<String>>,
    pub pending_arrivals: HashMap<String, u64>,
    pub acceptance_retries: HashMap<String, u32>,
    pub event_errors: u64,
    pub processed_events: Vec<SimEvent>,
}

impl EngineSnapshot {
    pub const SCHEMA_V1: &'static str = "rzd-des-engine-snapshot/v1";
}

impl Engine {
    pub fn snapshot(&self) -> EngineSnapshot {
        EngineSnapshot {
            schema: EngineSnapshot::SCHEMA_V1.to_string(),
            network: self.network.clone(),
            norms: self.norms.clone(),
            prng: self.prng.clone(),
            queue: self.queue.snapshot(),
            crews: self.crews.clone(),
            locos: self.locos.clone(),
            violations: self.violations.snapshot(),
            tu3_rows: self.tu3_rows.clone(),
            stats: self.stats.clone(),
            current_time: self.current_time,
            max_time: self.max_time,
            warmup_minutes: self.warmup_minutes,
            loco_by_station: self.loco_by_station.clone(),
            pending_arrivals: self.pending_arrivals.clone(),
            acceptance_retries: self.acceptance_retries.clone(),
            event_errors: self.event_errors,
            processed_events: self.processed_events.clone(),
        }
    }

    pub fn from_snapshot(snapshot: EngineSnapshot) -> Result<Self, String> {
        if snapshot.schema != EngineSnapshot::SCHEMA_V1 {
            return Err(format!("Unsupported snapshot schema: {}", snapshot.schema));
        }

        let mut network = snapshot.network;
        network.refresh_derived_state();

        let turnaround_stations: HashSet<String> = network
            .stations
            .values()
            .filter(|station| station.kind == crate::network::StationKind::Turnaround)
            .map(|station| station.id.clone())
            .collect();

        Ok(Self {
            network,
            norms: snapshot.norms,
            prng: snapshot.prng,
            queue: EventQueue::from_snapshot(snapshot.queue),
            crews: snapshot.crews,
            locos: snapshot.locos,
            violations: ViolationAnalytics::from_snapshot(snapshot.violations),
            tu3_rows: snapshot.tu3_rows,
            stats: snapshot.stats,
            current_time: snapshot.current_time,
            max_time: snapshot.max_time,
            warmup_minutes: snapshot.warmup_minutes,
            loco_by_station: snapshot.loco_by_station,
            turnaround_stations,
            pending_arrivals: snapshot.pending_arrivals,
            acceptance_retries: snapshot.acceptance_retries,
            event_errors: snapshot.event_errors,
            processed_events: snapshot.processed_events,
        })
    }
}
