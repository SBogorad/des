use crate::events::SimEvent;
use crate::network::Network;
use crate::scenario::ScenarioConfig;
use crate::statistics::SimulationStats;
use crate::tu3::Tu3Row;
use crate::violations::ViolationSummary;
use crate::violations::{Violation, ViolationAnalytics};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub struct Export;

pub const EVENT_TRACE_SCHEMA_V1: &str = "rzd-des-event-trace/v1";
pub const CED_EXPORT_SCHEMA_V1: &str = "rzd-des-ced-export/v1";

/// Serialize JSON with object keys in lexical order for byte-stable artifacts.
pub fn to_canonical_json<T: Serialize>(value: &T) -> Result<String, serde_json::Error> {
    fn write_value(value: &serde_json::Value, output: &mut Vec<u8>) {
        match value {
            serde_json::Value::Object(object) => {
                output.push(b'{');
                let mut entries: Vec<_> = object.iter().collect();
                entries.sort_unstable_by(|left, right| left.0.cmp(right.0));
                for (index, (key, child)) in entries.into_iter().enumerate() {
                    if index > 0 {
                        output.push(b',');
                    }
                    serde_json::to_writer(&mut *output, key).expect("JSON string cannot fail");
                    output.push(b':');
                    write_value(child, output);
                }
                output.push(b'}');
            }
            serde_json::Value::Array(array) => {
                output.push(b'[');
                for (index, child) in array.iter().enumerate() {
                    if index > 0 {
                        output.push(b',');
                    }
                    write_value(child, output);
                }
                output.push(b']');
            }
            scalar => serde_json::to_writer(&mut *output, scalar).expect("JSON value cannot fail"),
        }
    }

    let value = serde_json::to_value(value)?;
    let mut output = Vec::new();
    write_value(&value, &mut output);
    Ok(String::from_utf8(output).expect("serialized JSON must be valid UTF-8"))
}

impl Export {
    pub fn tu3_to_tsv(rows: &[Tu3Row]) -> String {
        let mut tsv = String::from("\u{FEFF}"); // BOM UTF-8
        tsv.push_str(&Tu3Row::to_tsv_header());
        tsv.push('\n');
        for row in rows {
            tsv.push_str(&row.to_tsv_row());
            tsv.push('\n');
        }
        tsv
    }

    pub fn tu3_to_json(rows: &[Tu3Row]) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(rows)
    }

    pub fn violations_to_json(analytics: &ViolationAnalytics) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(&analytics.violations)
    }

    pub fn violations_to_ndjson(
        analytics: &ViolationAnalytics,
    ) -> Result<String, serde_json::Error> {
        let estimated_bytes = analytics.violations.len().saturating_mul(256);
        let mut output = Vec::with_capacity(estimated_bytes);
        for violation in &analytics.violations {
            serde_json::to_writer(&mut output, violation)?;
            output.push(b'\n');
        }
        Ok(String::from_utf8(output).expect("serialized JSON must be valid UTF-8"))
    }

    pub fn summary_to_json(analytics: &ViolationAnalytics) -> Result<String, serde_json::Error> {
        let summary = analytics.summary();
        serde_json::to_string_pretty(&summary)
    }

    pub fn event_trace_to_json(trace: &EventTrace) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(trace)
    }

    /// Serialize event records directly for NDJSON consumers.
    ///
    /// This avoids constructing an `EventTrace` wrapper and cloning its event
    /// vector when the schema metadata is not part of the requested output.
    pub fn events_to_ndjson(events: &[SimEvent]) -> Result<String, serde_json::Error> {
        let estimated_bytes = events.len().saturating_mul(192);
        let mut output = Vec::with_capacity(estimated_bytes);
        for event in events {
            serde_json::to_writer(&mut output, event)?;
            output.push(b'\n');
        }
        Ok(String::from_utf8(output).expect("serialized JSON must be valid UTF-8"))
    }

    /// Backward-compatible wrapper for callers that already own an EventTrace.
    pub fn event_trace_to_ndjson(trace: &EventTrace) -> Result<String, serde_json::Error> {
        Self::events_to_ndjson(&trace.events)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventTrace {
    pub schema: String,
    pub until_minutes: u64,
    pub events: Vec<SimEvent>,
}

pub fn build_event_trace(events: &[SimEvent], until_minutes: u64) -> EventTrace {
    EventTrace {
        schema: EVENT_TRACE_SCHEMA_V1.to_string(),
        until_minutes,
        events: events.to_vec(),
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplayBundle {
    pub schema: String,
    pub engine: ReplayEngineInfo,
    pub replay: ReplayRunInfo,
    pub network: Network,
    pub results: ReplayResults,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplayEngineInfo {
    pub name: String,
    pub version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplayRunInfo {
    pub seed: u32,
    pub duration_days: u32,
    pub until_minutes: u64,
    pub warmup_minutes: u64,
    pub scenario: ScenarioConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplayResults {
    pub stats: SimulationStats,
    pub violation_summary: ViolationSummary,
    pub ced: CedExport,
    pub tu3_row_count: usize,
}

pub struct ReplayBundleInputs<'a> {
    pub network: &'a Network,
    pub scenario: &'a ScenarioConfig,
    pub seed: u32,
    pub until_minutes: u64,
    pub warmup_minutes: u64,
    pub stats: &'a SimulationStats,
    pub violation_summary: &'a ViolationSummary,
    pub tu3_rows: &'a [Tu3Row],
    pub ced: &'a CedExport,
}

// === ЦЭД Export Format ===

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CedExport {
    pub schema: String,
    pub trips: Vec<CedTrip>,
    pub events: Vec<CedEvent>,
    pub violations: Vec<CedViolation>,
    pub factors: Vec<CedFactor>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CedTrip {
    pub id: String,
    pub crew_id: String,
    pub route: String,
    pub depot: String,
    pub start_time: String,
    pub end_time: String,
    pub distance_km: f32,
    pub violations: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CedEvent {
    pub id: String,
    pub trip_id: String,
    pub shift: u32,
    pub segment: String,
    pub factor: String,
    pub responsible: String,
    pub loss_hours: f32,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CedViolation {
    pub id: String,
    pub trip_id: String,
    pub kind: String,
    pub severity: String,
    pub description: String,
    pub norm_reference: String,
    pub detected_at: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CedFactor {
    pub name: String,
    pub percent: f32,
    pub owner: String,
}

pub fn build_ced_export(tu3_rows: &[Tu3Row], violations: &[Violation]) -> CedExport {
    let mut trips = Vec::new();
    let mut events = Vec::new();
    let mut ced_violations = Vec::new();
    let mut violations_by_crew: HashMap<&str, Vec<String>> = HashMap::new();

    for violation in violations {
        violations_by_crew
            .entry(violation.crew_id.as_str())
            .or_default()
            .push(violation.id.clone());
    }

    for (idx, row) in tu3_rows.iter().enumerate() {
        let trip_id = format!("TRIP-{:04}", idx + 1);
        let trip_violations = violations_by_crew
            .get(row.crew_id.as_str())
            .cloned()
            .unwrap_or_default();

        trips.push(CedTrip {
            id: trip_id.clone(),
            crew_id: row.crew_id.clone(),
            route: format!("{} → {}", row.from_station, row.to_station),
            depot: row.depot_name.clone(),
            start_time: format_min_to_time(row.shift_start),
            end_time: row.shift_end.map_or("N/A".into(), format_min_to_time),
            distance_km: row.total_distance_km,
            violations: trip_violations,
        });

        if row.failure_count > 0 {
            events.push(CedEvent {
                id: format!("EVT-{:04}-FAIL", idx + 1),
                trip_id: trip_id.clone(),
                shift: 1,
                segment: row.route_segments.clone(),
                factor: "Отказ ТМЦ".into(),
                responsible: "loco".into(),
                loss_hours: row.failure_count as f32 * 1.5,
                status: "fact".into(),
            });
        }

        if row.voltage_sag_count > 0 {
            events.push(CedEvent {
                id: format!("EVT-{:04}-VOLT", idx + 1),
                trip_id: trip_id.clone(),
                shift: 1,
                segment: row.route_segments.clone(),
                factor: "Просадка КС".into(),
                responsible: "power".into(),
                loss_hours: row.voltage_sag_count as f32 * 0.5,
                status: "fact".into(),
            });
        }

        if row.wait_count > 0 {
            events.push(CedEvent {
                id: format!("EVT-{:04}-WAIT", idx + 1),
                trip_id: trip_id.clone(),
                shift: 1,
                segment: "".into(),
                factor: "Ожидание поезда".into(),
                responsible: "dispatch".into(),
                loss_hours: row.wait_total_min as f32 / 60.0,
                status: "fact".into(),
            });
        }
    }

    for v in violations {
        ced_violations.push(CedViolation {
            id: v.id.clone(),
            trip_id: String::new(),
            kind: format!("{:?}", v.violation_type),
            severity: format!("{:?}", v.severity),
            description: v.description.clone(),
            norm_reference: v.norm_reference.clone(),
            detected_at: v.detected_at,
        });
    }

    let total_events = events.len() as f32;
    let factors = if total_events > 0.0 {
        let fail_events = events.iter().filter(|e| e.factor == "Отказ ТМЦ").count() as f32;
        let volt_events = events.iter().filter(|e| e.factor == "Просадка КС").count() as f32;
        let wait_events = events
            .iter()
            .filter(|e| e.factor == "Ожидание поезда")
            .count() as f32;
        vec![
            CedFactor {
                name: "Отказы ТМЦ".into(),
                percent: fail_events / total_events * 100.0,
                owner: "loco".into(),
            },
            CedFactor {
                name: "Просадка КС".into(),
                percent: volt_events / total_events * 100.0,
                owner: "power".into(),
            },
            CedFactor {
                name: "Ожидание".into(),
                percent: wait_events / total_events * 100.0,
                owner: "dispatch".into(),
            },
        ]
    } else {
        vec![]
    };

    CedExport {
        schema: CED_EXPORT_SCHEMA_V1.to_string(),
        trips,
        events,
        violations: ced_violations,
        factors,
    }
}

pub fn build_replay_bundle(input: ReplayBundleInputs<'_>) -> ReplayBundle {
    ReplayBundle {
        schema: "rzd-des-replay-bundle/v1".to_string(),
        engine: ReplayEngineInfo {
            name: "rzd-des-engine".to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
        },
        replay: ReplayRunInfo {
            seed: input.seed,
            duration_days: input.scenario.duration_days,
            until_minutes: input.until_minutes,
            warmup_minutes: input.warmup_minutes,
            scenario: input.scenario.clone(),
        },
        network: input.network.clone(),
        results: ReplayResults {
            stats: input.stats.clone(),
            violation_summary: ViolationSummary {
                total: input.violation_summary.total,
                by_type: input.violation_summary.by_type.clone(),
                by_severity: input.violation_summary.by_severity.clone(),
                by_crew: input.violation_summary.by_crew.clone(),
                by_loco: input.violation_summary.by_loco.clone(),
            },
            ced: input.ced.clone(),
            tu3_row_count: input.tu3_rows.len(),
        },
    }
}

fn format_min_to_time(min: u64) -> String {
    let hours = (min / 60) % 24;
    let minutes = min % 60;
    format!("{:02}:{:02}", hours, minutes)
}
