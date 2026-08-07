use crate::entities::{Crew, Locomotive, SimTime};
use crate::events::{EventKind, EventQueue, SimEvent, ViolationType};
use crate::export::{build_ced_export, build_event_trace, CedExport, EventTrace};
use crate::injection::{InjectedEvent, InjectionPlan};
use crate::network::{Network, StationKind};
use crate::norms::RzdNorms;
use crate::prng::Mulberry32;
use crate::statistics::SimulationStats;
use crate::tu3::Tu3Row;
use crate::violations::{Severity, Violation, ViolationAnalytics};
use std::collections::{HashMap, HashSet};

mod error;
mod handlers;
mod movement;
mod snapshot;

pub use error::EngineError;
pub use snapshot::EngineSnapshot;

pub struct Engine {
    pub network: Network,
    pub norms: RzdNorms,
    pub prng: Mulberry32,

    pub queue: EventQueue,
    pub crews: HashMap<String, Crew>,
    pub locos: HashMap<String, Locomotive>,

    pub violations: ViolationAnalytics,
    pub tu3_rows: Vec<Tu3Row>,
    pub stats: SimulationStats,

    pub current_time: SimTime,
    max_time: SimTime,
    warmup_minutes: SimTime,

    // Spatial indexes
    pub(crate) loco_by_station: HashMap<String, Vec<String>>,
    pub(crate) turnaround_stations: HashSet<String>,

    // Event tracking for cancellation
    pub(crate) pending_arrivals: HashMap<String, u64>,

    // Acceptance retry tracking
    pub(crate) acceptance_retries: HashMap<String, u32>,

    /// Count of engine processing errors (e.g. missing crew_id)
    pub event_errors: u64,
    pub processed_events: Vec<SimEvent>,
}

impl Engine {
    pub fn new(network: Network, seed: u32) -> Self {
        let turnaround_stations: HashSet<String> = network
            .stations
            .values()
            .filter(|s| s.kind == StationKind::Turnaround)
            .map(|s| s.id.clone())
            .collect();

        Self {
            network,
            norms: RzdNorms::default(),
            prng: Mulberry32::new(seed),
            queue: EventQueue::new(),
            crews: HashMap::new(),
            locos: HashMap::new(),
            violations: ViolationAnalytics::new(),
            tu3_rows: Vec::new(),
            stats: SimulationStats::new(),
            current_time: 0,
            max_time: u64::MAX,
            warmup_minutes: 0,
            loco_by_station: HashMap::new(),
            turnaround_stations,
            pending_arrivals: HashMap::new(),
            acceptance_retries: HashMap::new(),
            event_errors: 0,
            processed_events: Vec::new(),
        }
    }

    pub fn set_warmup(&mut self, minutes: SimTime) {
        self.warmup_minutes = minutes;
    }

    pub fn add_crew(&mut self, crew: Crew) {
        self.crews.insert(crew.id.clone(), crew);
    }

    pub fn schedule_all_shifts(&mut self, start_offset: SimTime, interval: SimTime) {
        let crew_ids: Vec<String> = self.crews.keys().cloned().collect();
        let mut crew_ids = crew_ids;
        crew_ids.sort();
        for (idx, crew_id) in crew_ids.into_iter().enumerate() {
            self.emit(
                start_offset + (idx as u64) * interval,
                EventKind::ShiftAppear,
                Some(crew_id),
                None,
                None,
                10,
            );
        }
    }

    pub fn add_loco(&mut self, loco: Locomotive) {
        self.loco_by_station
            .entry(loco.position.clone())
            .or_default()
            .push(loco.id.clone());
        self.locos.insert(loco.id.clone(), loco);
    }

    pub fn run(&mut self, until: SimTime) {
        self.max_time = until;
        while let Some(event) = self.queue.pop() {
            if event.time > until {
                self.queue.push_existing(event);
                break;
            }
            self.current_time = event.time;
            self.processed_events.push(event.clone());
            self.process_event(event);
        }
    }

    /// Run simulation with progress callback that fires every N events
    pub fn run_with_progress<F>(&mut self, until: SimTime, progress_fn: F)
    where
        F: Fn(u64, u64, f64), // (current, total, elapsed_secs)
    {
        self.max_time = until;
        let mut count: u64 = 0;
        let start = std::time::Instant::now();
        let total_estimate = self.queue.len() as u64 * 2; // rough estimate

        while let Some(event) = self.queue.pop() {
            if event.time > until {
                self.queue.push_existing(event);
                break;
            }
            self.current_time = event.time;
            self.processed_events.push(event.clone());
            self.process_event(event);
            count += 1;

            if count.is_multiple_of(1000) {
                progress_fn(count, total_estimate, start.elapsed().as_secs_f64());
            }
        }

        progress_fn(count, count, start.elapsed().as_secs_f64());
    }

    pub fn crew_count(&self) -> usize {
        self.crews.len()
    }

    pub fn loco_count(&self) -> usize {
        self.locos.len()
    }

    pub fn stats(&self) -> &SimulationStats {
        &self.stats
    }

    pub fn violations_summary(&self) -> crate::violations::ViolationSummary {
        self.violations.summary()
    }

    pub fn export_ced(&self) -> CedExport {
        build_ced_export(&self.tu3_rows, &self.violations.violations)
    }

    pub fn export_event_trace(&self) -> EventTrace {
        build_event_trace(&self.processed_events, self.current_time)
    }

    pub fn apply_injection_plan(&mut self, plan: &InjectionPlan) -> Result<(), EngineError> {
        plan.validate().map_err(EngineError::InvalidInjection)?;
        for injected in &plan.events {
            self.schedule_injected_event(injected)?;
        }
        Ok(())
    }

    // === EVENT BUILDER HELPERS ===

    fn emit(
        &mut self,
        time: SimTime,
        kind: EventKind,
        crew_id: Option<String>,
        loco_id: Option<String>,
        segment_id: Option<String>,
        priority: u8,
    ) -> u64 {
        self.queue.push(SimEvent {
            id: 0,
            time,
            kind,
            crew_id,
            loco_id,
            segment_id,
            priority,
        })
    }

    fn emit_now(
        &mut self,
        kind: EventKind,
        crew_id: Option<String>,
        loco_id: Option<String>,
        segment_id: Option<String>,
        priority: u8,
    ) -> u64 {
        self.emit(
            self.current_time,
            kind,
            crew_id,
            loco_id,
            segment_id,
            priority,
        )
    }

    fn schedule_injected_event(&mut self, injected: &InjectedEvent) -> Result<(), EngineError> {
        match injected {
            InjectedEvent::WaitWindow {
                time,
                crew_id,
                loco_id,
                duration_min,
            } => {
                self.require_known_crew(crew_id)?;
                self.emit(
                    *time,
                    EventKind::WaitStart,
                    Some(crew_id.clone()),
                    loco_id.clone(),
                    None,
                    1,
                );
                self.emit(
                    *time + *duration_min,
                    EventKind::WaitEnd,
                    Some(crew_id.clone()),
                    loco_id.clone(),
                    None,
                    1,
                );
            }
            InjectedEvent::FailureWindow {
                time,
                crew_id,
                loco_id,
                segment_id,
                failure_kind,
                duration_min,
            } => {
                self.require_known_crew(crew_id)?;
                self.require_known_loco(loco_id)?;
                self.require_known_segment(segment_id)?;
                self.emit(
                    *time,
                    EventKind::FailureOccurred(*failure_kind),
                    Some(crew_id.clone()),
                    Some(loco_id.clone()),
                    Some(segment_id.clone()),
                    1,
                );
                self.emit(
                    *time + *duration_min,
                    EventKind::FailureResolved,
                    Some(crew_id.clone()),
                    Some(loco_id.clone()),
                    Some(segment_id.clone()),
                    1,
                );
            }
            InjectedEvent::VoltageSagWindow {
                time,
                crew_id,
                loco_id,
                segment_id,
                voltage_v,
                duration_min,
            } => {
                self.require_known_crew(crew_id)?;
                self.require_known_segment(segment_id)?;
                self.emit(
                    *time,
                    EventKind::VoltageSagStart {
                        voltage_v: *voltage_v,
                        segment_id: segment_id.clone(),
                    },
                    Some(crew_id.clone()),
                    loco_id.clone(),
                    Some(segment_id.clone()),
                    1,
                );
                self.emit(
                    *time + *duration_min,
                    EventKind::VoltageSagEnd,
                    Some(crew_id.clone()),
                    loco_id.clone(),
                    Some(segment_id.clone()),
                    1,
                );
            }
            InjectedEvent::ManualViolation {
                time,
                crew_id,
                loco_id,
                segment_id,
                violation_type,
                severity,
                description,
                norm_reference,
            } => {
                self.require_known_crew(crew_id)?;
                if let Some(loco_id) = loco_id {
                    self.require_known_loco(loco_id)?;
                }
                if let Some(segment_id) = segment_id {
                    self.require_known_segment(segment_id)?;
                }
                self.emit(
                    *time,
                    EventKind::InjectedViolation {
                        violation_type: *violation_type,
                        severity: severity.clone(),
                        description: description.clone(),
                        norm_reference: norm_reference.clone(),
                    },
                    Some(crew_id.clone()),
                    loco_id.clone(),
                    segment_id.clone(),
                    1,
                );
            }
        }
        Ok(())
    }

    // === VIOLATION HELPERS ===

    #[allow(clippy::too_many_arguments)]
    fn add_violation(
        &mut self,
        vtype: ViolationType,
        severity: Severity,
        crew_id: &str,
        loco_id: Option<String>,
        segment_id: Option<String>,
        description: &str,
        norm_reference: &str,
    ) -> String {
        if self.in_warmup() {
            return String::new();
        }
        let vid = self.violations.next_id();
        self.violations.add_violation(Violation {
            id: vid.clone(),
            violation_type: vtype,
            severity,
            crew_id: crew_id.to_string(),
            loco_id,
            segment_id,
            detected_at: self.current_time,
            description: description.to_string(),
            norm_reference: norm_reference.to_string(),
        });
        self.stats.record_violation();
        vid
    }

    fn in_warmup(&self) -> bool {
        self.current_time < self.warmup_minutes
    }

    // === EVENT EXTRACTION HELPERS ===

    fn req_crew_id(&self, event: &SimEvent) -> Result<String, EngineError> {
        event.crew_id.clone().ok_or(EngineError::MissingCrewId)
    }

    fn req_seg_id(&self, event: &SimEvent) -> Result<String, EngineError> {
        event
            .segment_id
            .clone()
            .ok_or(EngineError::MissingSegmentId)
    }

    fn require_known_crew(&self, crew_id: &str) -> Result<(), EngineError> {
        if self.crews.contains_key(crew_id) {
            Ok(())
        } else {
            Err(EngineError::InvalidInjection(format!(
                "unknown crew_id '{}'",
                crew_id
            )))
        }
    }

    fn require_known_loco(&self, loco_id: &str) -> Result<(), EngineError> {
        if self.locos.contains_key(loco_id) {
            Ok(())
        } else {
            Err(EngineError::InvalidInjection(format!(
                "unknown loco_id '{}'",
                loco_id
            )))
        }
    }

    fn require_known_segment(&self, segment_id: &str) -> Result<(), EngineError> {
        if self.network.segments.contains_key(segment_id) {
            Ok(())
        } else {
            Err(EngineError::InvalidInjection(format!(
                "unknown segment_id '{}'",
                segment_id
            )))
        }
    }

    // === MAIN EVENT DISPATCH ===

    fn process_event(&mut self, event: SimEvent) {
        self.stats.record_event();
        let result = match event.kind.clone() {
            EventKind::ShiftAppear => self.handle_appear(event),
            EventKind::MedCheckStart => self.handle_med_check_start(event),
            EventKind::MedCheckEnd => self.handle_med_check_end(event),
            EventKind::PreTripBriefingEnd => self.handle_pretrip_briefing_end(event),
            EventKind::AcceptanceStart => self.handle_acceptance_start(event),
            EventKind::AcceptanceEnd => self.handle_acceptance_end(event),
            EventKind::DepartureCheckpoint => self.handle_departure_checkpoint(event),
            EventKind::WaitStart => self.handle_wait_start(event),
            EventKind::WaitEnd => self.handle_wait_end(event),
            EventKind::ArriveStation => self.handle_arrive(event),
            EventKind::PassWaypoint => self.handle_pass_waypoint(event),
            EventKind::FailureOccurred(kind) => self.handle_failure(event, kind),
            EventKind::FailureResolved => self.handle_failure_resolved(event),
            EventKind::LineChangeStart => self.handle_line_change_start(event),
            EventKind::VoltageSagStart {
                voltage_v,
                segment_id: ref seg_id,
            } => {
                let seg_id = seg_id.clone();
                self.handle_voltage_sag_start(event, voltage_v, seg_id)
            }
            EventKind::VoltageSagEnd => self.handle_voltage_sag_end(event),
            EventKind::ReserveStart => self.handle_reserve_start(event),
            EventKind::ReserveEnd => self.handle_reserve_end(event),
            EventKind::TurnaroundRestStart => self.handle_turnaround_rest_start(event),
            EventKind::TurnaroundRestEnd => self.handle_turnaround_rest_end(event),
            EventKind::HandoverStart => self.handle_handover_start(event),
            EventKind::HandoverEnd => self.handle_handover_end(event),
            EventKind::ArrivalCheckpoint => self.handle_arrival_checkpoint(event),
            EventKind::ShiftEnd => self.handle_shift_end(event),
            EventKind::HomeRestStart => self.handle_home_rest_start(event),
            EventKind::HomeRestEnd => self.handle_home_rest_end(event),
            EventKind::ViolationDetected(vtype) => self.handle_violation_detected(event, &vtype),
            EventKind::InjectedViolation {
                violation_type,
                severity,
                ref description,
                ref norm_reference,
            } => self.handle_injected_violation(
                event,
                violation_type,
                severity,
                description.clone(),
                norm_reference.clone(),
            ),
        };
        if let Err(_e) = result {
            self.event_errors += 1;
        }
    }
}
