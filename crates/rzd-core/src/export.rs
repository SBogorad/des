use serde::{Deserialize, Serialize};
use crate::tu3::Tu3Row;
use crate::violations::{ViolationAnalytics, Violation};

pub struct Export;

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

    pub fn summary_to_json(analytics: &ViolationAnalytics) -> Result<String, serde_json::Error> {
        let summary = analytics.summary();
        serde_json::to_string_pretty(&summary)
    }
}

// === ЦЭД Export Format ===

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CedExport {
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

pub fn build_ced_export(
    tu3_rows: &[Tu3Row],
    violations: &[Violation],
) -> CedExport {
    let mut trips = Vec::new();
    let mut events = Vec::new();
    let mut ced_violations = Vec::new();

    for (idx, row) in tu3_rows.iter().enumerate() {
        let trip_id = format!("TRIP-{:04}", idx + 1);
        let trip_violations: Vec<String> = violations.iter()
            .filter(|v| v.crew_id == row.crew_id)
            .map(|v| v.id.clone())
            .collect();

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
        let wait_events = events.iter().filter(|e| e.factor == "Ожидание поезда").count() as f32;
        vec![
            CedFactor { name: "Отказы ТМЦ".into(), percent: fail_events / total_events * 100.0, owner: "loco".into() },
            CedFactor { name: "Просадка КС".into(), percent: volt_events / total_events * 100.0, owner: "power".into() },
            CedFactor { name: "Ожидание".into(), percent: wait_events / total_events * 100.0, owner: "dispatch".into() },
        ]
    } else {
        vec![]
    };

    CedExport {
        trips,
        events,
        violations: ced_violations,
        factors,
    }
}

fn format_min_to_time(min: u64) -> String {
    let hours = (min / 60) % 24;
    let minutes = min % 60;
    format!("{:02}:{:02}", hours, minutes)
}
