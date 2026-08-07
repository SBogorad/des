use std::collections::HashMap;

use rzd_core::export::{build_replay_bundle, CedExport, ReplayBundleInputs};
use rzd_core::network::{Leg, Network, Road, Segment, Station, StationKind, VoltageKind};
use rzd_core::scenario::ScenarioConfig;
use rzd_core::statistics::SimulationStats;
use rzd_core::violations::ViolationSummary;

fn minimal_network() -> Network {
    let mut stations = HashMap::new();
    stations.insert(
        "ST-001".to_string(),
        Station {
            id: "ST-001".to_string(),
            name: "Альфа".to_string(),
            kind: StationKind::MainDepot,
            region: "R-A".to_string(),
            coords: Some((0.0, 0.0)),
            depot_id: Some("DEP-01".to_string()),
            voltage_kv: VoltageKind::AC25000,
            electrification: true,
        },
    );
    stations.insert(
        "ST-002".to_string(),
        Station {
            id: "ST-002".to_string(),
            name: "Бета".to_string(),
            kind: StationKind::Turnaround,
            region: "R-A".to_string(),
            coords: Some((1.0, 0.0)),
            depot_id: Some("DEP-02".to_string()),
            voltage_kv: VoltageKind::AC25000,
            electrification: true,
        },
    );

    let mut segments = HashMap::new();
    segments.insert(
        "SEG-001".to_string(),
        Segment {
            id: "SEG-001".to_string(),
            from: "ST-001".to_string(),
            to: "ST-002".to_string(),
            distance_km: 100.0,
            base_speed_kmh: 80.0,
            gradient_factor: 1.0,
            failure_base_rate: 0.001,
            voltage_sag_prob: 0.01,
            single_track: false,
        },
    );

    let mut legs = HashMap::new();
    legs.insert(
        "LEG-001".to_string(),
        Leg {
            id: "LEG-001".to_string(),
            name: "Альфа - Бета".to_string(),
            from_depot: "DEP-01".to_string(),
            to_depot: "DEP-02".to_string(),
            segments: vec!["SEG-001".to_string()],
            total_km: 100.0,
            road_id: "ROAD-001".to_string(),
        },
    );

    let mut roads = HashMap::new();
    roads.insert(
        "ROAD-001".to_string(),
        Road {
            id: "ROAD-001".to_string(),
            name: "Synthetic Road".to_string(),
            stations: vec!["ST-001".to_string(), "ST-002".to_string()],
            depots: vec!["DEP-01".to_string(), "DEP-02".to_string()],
            legs: vec!["LEG-001".to_string()],
            region_multiplier: 1.0,
        },
    );

    Network::new(stations, segments, legs, roads)
}

#[test]
fn replay_bundle_serializes_versioned_run_contract() {
    let network = minimal_network();
    let scenario = ScenarioConfig {
        crew_count: 10,
        loco_count: 4,
        seed: 1234,
        duration_days: 7,
        ..Default::default()
    };
    let stats = SimulationStats::new();
    let violation_summary = ViolationSummary {
        total: 0,
        by_type: HashMap::new(),
        by_severity: HashMap::new(),
        by_crew: HashMap::new(),
        by_loco: HashMap::new(),
    };
    let ced = CedExport {
        trips: Vec::new(),
        events: Vec::new(),
        violations: Vec::new(),
        factors: Vec::new(),
    };

    let bundle = build_replay_bundle(ReplayBundleInputs {
        network: &network,
        scenario: &scenario,
        seed: 1234,
        until_minutes: 10080,
        warmup_minutes: 30,
        stats: &stats,
        violation_summary: &violation_summary,
        tu3_rows: &[],
        ced: &ced,
    });

    let text = serde_json::to_string_pretty(&bundle).expect("bundle JSON");
    assert!(text.contains("\"schema\": \"rzd-des-replay-bundle/v1\""));
    assert!(text.contains("\"seed\": 1234"));
    assert!(text.contains("\"duration_days\": 7"));
    assert!(text.contains("\"name\": \"rzd-des-engine\""));
}
