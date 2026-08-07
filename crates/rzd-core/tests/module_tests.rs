use rzd_core::engine::Engine;
use rzd_core::entities::{Crew, CrewState, LocoKind, LocoState, Locomotive};
use rzd_core::events::{EventKind, SimEvent, ViolationType};
use rzd_core::export::Export;
use rzd_core::network::{Network, Segment, Station, StationKind, VoltageKind};
use rzd_core::prng::Mulberry32;
use rzd_core::tu3::Tu3Row;
use rzd_core::violations::{Severity, Violation, ViolationAnalytics};
use std::collections::HashMap;

fn make_test_network() -> Network {
    let mut stations = HashMap::new();
    let mut segments = HashMap::new();

    for i in 1..=5 {
        let id = format!("ST-{:03}", i);
        stations.insert(
            id.clone(),
            Station {
                id: id.clone(),
                name: format!("Station {}", i),
                kind: if i == 1 {
                    StationKind::MainDepot
                } else if i == 5 {
                    StationKind::Terminal
                } else if i == 3 {
                    StationKind::Turnaround
                } else {
                    StationKind::Waypoint
                },
                region: "NORTH".to_string(),
                coords: None,
                depot_id: if i == 1 {
                    Some("DEP-01".to_string())
                } else {
                    None
                },
                voltage_kv: VoltageKind::DC3000,
                electrification: true,
            },
        );
        if i > 1 {
            let seg_id = format!("SEG-{:03}", i - 1);
            segments.insert(
                seg_id.clone(),
                Segment {
                    id: seg_id,
                    from: format!("ST-{:03}", i - 1),
                    to: id.clone(),
                    distance_km: 50.0 + (i as f32) * 10.0,
                    base_speed_kmh: 80.0,
                    gradient_factor: 1.0 + (i as f32) * 0.05,
                    failure_base_rate: 0.001,
                    voltage_sag_prob: 0.01,
                    single_track: false,
                },
            );
        }
    }

    Network::new(stations, segments, HashMap::new(), HashMap::new())
}

// === NETWORK TESTS ===

#[test]
fn test_network_from_json() {
    let json = r#"{
        "stations": {
            "ST-001": {"id":"ST-001","name":"A","kind":"MainDepot","region":"R","coords":null,"depot_id":"D1","voltage_kv":"DC3000","electrification":true},
            "ST-002": {"id":"ST-002","name":"B","kind":"Terminal","region":"R","coords":null,"depot_id":null,"voltage_kv":"DC3000","electrification":true}
        },
        "segments": {
            "SEG-001": {"id":"SEG-001","from":"ST-001","to":"ST-002","distance_km":50.0,"base_speed_kmh":80.0,"gradient_factor":1.0,"failure_base_rate":0.001,"voltage_sag_prob":0.01,"single_track":false}
        },
        "legs": {},
        "roads": {
            "ROAD-1": {"id":"ROAD-1","name":"Test Road","stations":["ST-001","ST-002"],"depots":[],"legs":[],"region_multiplier":1.0}
        }
    }"#;
    let network = Network::from_json(json);
    assert!(network.is_ok());
    let net = network.unwrap();
    assert!(net.stations.contains_key("ST-001"));
}

#[test]
fn test_network_from_json_invalid() {
    let json = r#"{"stations":{},"segments":{},"legs":{},"roads":{}}"#;
    let network = Network::from_json(json);
    assert!(network.is_ok());
}

#[test]
fn test_shortest_path_bfs() {
    let net = make_test_network();
    let path = net.shortest_path(&"ST-001".to_string(), &"ST-005".to_string());
    assert!(path.is_some());
    let path = path.unwrap();
    assert_eq!(path.len(), 4); // 4 segments: 001->002, 002->003, 003->004, 004->005
}

#[test]
fn test_shortest_path_same_station() {
    let net = make_test_network();
    let path = net.shortest_path(&"ST-001".to_string(), &"ST-001".to_string());
    assert!(path.is_some());
    assert!(path.unwrap().is_empty());
}

#[test]
fn test_shortest_path_no_route() {
    let net = make_test_network();
    let path = net.shortest_path(&"ST-001".to_string(), &"ST-999".to_string());
    assert!(path.is_none());
}

#[test]
fn test_shortest_path_dijkstra() {
    let net = make_test_network();
    let result = net.shortest_path_dijkstra(&"ST-001".to_string(), &"ST-005".to_string());
    assert!(result.is_some());
    let (path, time) = result.unwrap();
    assert_eq!(path.len(), 4);
    assert!(time > 0.0);
}

#[test]
fn test_shortest_path_dijkstra_prefers_lower_cost_over_direct_route() {
    let mut net = make_test_network();
    net.segments.insert(
        "SEG-DIRECT".to_string(),
        Segment {
            id: "SEG-DIRECT".to_string(),
            from: "ST-001".to_string(),
            to: "ST-005".to_string(),
            distance_km: 1_000.0,
            base_speed_kmh: 60.0,
            gradient_factor: 1.0,
            failure_base_rate: 0.001,
            voltage_sag_prob: 0.01,
            single_track: false,
        },
    );
    net.refresh_derived_state();

    let (path, time) = net
        .shortest_path_dijkstra(&"ST-001".to_string(), &"ST-005".to_string())
        .expect("connected network should have a route");

    assert_eq!(path.len(), 4);
    assert!(!path.contains(&"SEG-DIRECT".to_string()));
    assert!(time < 1_000.0);
}

#[test]
fn test_segment_travel_time() {
    let net = make_test_network();
    let time = net.segment_travel_time_min(&"SEG-001".to_string());
    assert!(time.is_some());
    let t = time.unwrap();
    assert!(t > 0.0);
    // SEG-001: 70km / 80kmh * 60 = 52.5min * 1.1 gradient = 57.75
    assert!((55.0..62.0).contains(&t));
}

#[test]
fn test_validate_valid() {
    let net = make_test_network();
    let errors = net.validate();
    assert!(errors.is_empty());
}

#[test]
fn test_get_neighbors() {
    let net = make_test_network();
    let neighbors = net.get_neighbors(&"ST-001".to_string());
    assert_eq!(neighbors.len(), 1);
    assert_eq!(neighbors[0].0, &"ST-002".to_string());
}

// === PRNG TESTS ===

#[test]
fn test_prng_deterministic() {
    let mut a = Mulberry32::new(42);
    let mut b = Mulberry32::new(42);
    for _ in 0..100 {
        assert_eq!(a.next_u32(), b.next_u32());
    }
}

#[test]
fn test_prng_range() {
    let mut prng = Mulberry32::new(1);
    for _ in 0..1000 {
        let v = prng.range(10.0, 20.0);
        assert!((10.0..=20.0).contains(&v));
    }
}

#[test]
fn test_prng_chance() {
    let mut prng = Mulberry32::new(1);
    let mut true_count = 0;
    for _ in 0..10000 {
        if prng.chance(0.5) {
            true_count += 1;
        }
    }
    assert!((4000..6000).contains(&true_count));
}

// === ENTITIES TESTS ===

#[test]
fn test_crew_new() {
    let crew = Crew::new("C1".into(), "D1".into(), "ST-001".into());
    assert_eq!(crew.state, CrewState::AtHome);
    assert_eq!(crew.shift_work_min, 0);
    assert!(crew.route_segments.is_empty());
}

#[test]
fn test_crew_night_shift() {
    let mut crew = Crew::new("C1".into(), "D1".into(), "ST-001".into());
    crew.shift_start = Some(1440 + 22 * 60); // 22:00 on day 1
    assert!(crew.is_night_shift());
}

#[test]
fn test_crew_day_shift() {
    let mut crew = Crew::new("C1".into(), "D1".into(), "ST-001".into());
    crew.shift_start = Some(1440 + 10 * 60); // 10:00 on day 1
    assert!(!crew.is_night_shift());
}

// === VIOLATIONS TESTS ===

#[test]
fn test_violations_analytics() {
    let mut va = ViolationAnalytics::new();
    assert!(va.violations.is_empty());

    va.add_violation(Violation {
        id: "V1".into(),
        violation_type: ViolationType::Overtime8h,
        severity: Severity::Warning,
        crew_id: "C1".into(),
        loco_id: None,
        segment_id: None,
        detected_at: 100,
        description: "Test".into(),
        norm_reference: "§4.1".into(),
    });

    assert_eq!(va.violations.len(), 1);
    assert_eq!(va.get_by_crew(&"C1".into()).len(), 1);
    assert!(va.get_by_crew(&"C2".into()).is_empty());

    let summary = va.summary();
    assert_eq!(summary.total, 1);
}

// === TU3 TESTS ===

#[test]
fn test_tu3_row_empty() {
    let row = Tu3Row::empty();
    assert_eq!(row.violations_count, 0);
    assert_eq!(row.total_distance_km, 0.0);
}

#[test]
fn test_tu3_tsv_header() {
    let header = Tu3Row::to_tsv_header();
    let fields: Vec<&str> = header.split('\t').collect();
    assert_eq!(fields.len(), 73); // 73 columns
}

#[test]
fn test_tu3_tsv_row() {
    let mut row = Tu3Row::empty();
    row.crew_id = "C1".into();
    row.loco_id = "L1".into();
    row.work_duration_min = 480;
    row.total_distance_km = 191.0;

    let tsv = row.to_tsv_row();
    let fields: Vec<&str> = tsv.split('\t').collect();
    assert_eq!(fields.len(), 73);
    assert_eq!(fields[4], "C1"); // crew_id
    assert_eq!(fields[10], "480"); // work_duration_min
}

// === EXPORT TESTS ===

#[test]
fn test_export_tu3_tsv() {
    let rows = vec![Tu3Row::empty()];
    let tsv = Export::tu3_to_tsv(&rows);
    assert!(tsv.starts_with('\u{FEFF}')); // BOM
    let lines: Vec<&str> = tsv.lines().collect();
    assert_eq!(lines.len(), 2); // header + 1 row
}

#[test]
fn test_export_violations_json() {
    let va = ViolationAnalytics::new();
    let json = Export::violations_to_json(&va).unwrap();
    assert_eq!(json, "[]");
}

// === ENGINE TESTS ===

#[test]
fn test_engine_new() {
    let net = make_test_network();
    let engine = Engine::new(net, 42);
    assert_eq!(engine.crew_count(), 0);
    assert_eq!(engine.loco_count(), 0);
}

#[test]
fn test_engine_add_crew_and_loco() {
    let net = make_test_network();
    let mut engine = Engine::new(net, 42);

    let mut crew = Crew::new("C1".into(), "DEP-01".into(), "ST-001".into());
    crew.route_segments = vec![
        "SEG-001".into(),
        "SEG-002".into(),
        "SEG-003".into(),
        "SEG-004".into(),
    ];
    engine.add_crew(crew);

    engine.add_loco(Locomotive {
        id: "L1".into(),
        series: "EL-2K".into(),
        loco_kind: LocoKind::Electric,
        sections: 2,
        depot_id: "DEP-01".into(),
        state: LocoState::Available,
        position: "ST-001".into(),
        mileage_km: 0.0,
        hours_since_to: 0.0,
        failures_total: 0,
        inventory_number: 500,
        manufacture_year: 2020,
    });

    assert_eq!(engine.crew_count(), 1);
    assert_eq!(engine.loco_count(), 1);
}

#[test]
fn test_engine_full_simulation() {
    let net = make_test_network();
    let mut engine = Engine::new(net, 42);

    for i in 1..=10 {
        let crew_id = format!("C-{:03}", i);
        let mut crew = Crew::new(crew_id.clone(), "DEP-01".into(), "ST-001".into());
        crew.route_segments = vec![
            "SEG-001".into(),
            "SEG-002".into(),
            "SEG-003".into(),
            "SEG-004".into(),
        ];
        crew.route_index = 0;
        engine.add_crew(crew);

        engine.queue.push(SimEvent {
            time: (i * 30) as u64,
            kind: EventKind::ShiftAppear,
            crew_id: Some(crew_id),
            loco_id: None,
            segment_id: None,
            priority: 10,
            id: 0,
        });
    }

    for i in 1..=5 {
        let loco_id = format!("L-{:03}", i);
        engine.add_loco(Locomotive {
            id: loco_id,
            series: "EL-2K".into(),
            loco_kind: LocoKind::Electric,
            sections: 2,
            depot_id: "DEP-01".into(),
            state: LocoState::Available,
            position: "ST-001".into(),
            mileage_km: 0.0,
            hours_since_to: 0.0,
            failures_total: 0,
            inventory_number: 500 + i as u32 * 10,
            manufacture_year: 2020,
        });
    }

    engine.run(1440); // 24 hours

    assert!(!engine.tu3_rows.is_empty());
    println!("TU-3 rows: {}", engine.tu3_rows.len());
    println!("Violations: {}", engine.violations.violations.len());
}

// === INTEGRATION: JSON NETWORK LOADING ===

#[test]
fn test_load_northern_json() {
    let json_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("data")
        .join("networks")
        .join("northern.json");

    if json_path.exists() {
        let json = std::fs::read_to_string(&json_path).unwrap();
        let network = Network::from_json(&json);
        assert!(
            network.is_ok(),
            "Failed to load northern.json: {:?}",
            network.err()
        );
        let net = network.unwrap();
        assert!(net.stations.len() >= 3);
        assert!(net.segments.len() >= 2);
    }
}

// === MULTI-REGION NETWORK TESTS ===

use rzd_core::norms::{RegionNorms, RegionNormsConfig};

fn make_multi_region_network() -> Network {
    let mut stations = HashMap::new();
    let mut segments = HashMap::new();
    let mut legs = HashMap::new();
    let mut roads = HashMap::new();

    // Region NORTH: stations ST-001..ST-003
    for i in 1..=3 {
        let id = format!("ST-{:03}", i);
        stations.insert(
            id.clone(),
            Station {
                id: id.clone(),
                name: format!("North Station {}", i),
                kind: if i == 1 {
                    StationKind::MainDepot
                } else {
                    StationKind::Waypoint
                },
                region: "NORTH".into(),
                coords: None,
                depot_id: if i == 1 { Some("DEP-N1".into()) } else { None },
                voltage_kv: VoltageKind::DC3000,
                electrification: true,
            },
        );
    }
    // Region SOUTH: stations ST-004..ST-005
    for i in 4..=5 {
        let id = format!("ST-{:03}", i);
        stations.insert(
            id.clone(),
            Station {
                id: id.clone(),
                name: format!("South Station {}", i),
                kind: if i == 5 {
                    StationKind::Terminal
                } else {
                    StationKind::Waypoint
                },
                region: "SOUTH".into(),
                coords: None,
                depot_id: None,
                voltage_kv: VoltageKind::AC25000,
                electrification: true,
            },
        );
    }

    // Segments within NORTH
    segments.insert(
        "SEG-001".into(),
        Segment {
            id: "SEG-001".into(),
            from: "ST-001".into(),
            to: "ST-002".into(),
            distance_km: 50.0,
            base_speed_kmh: 120.0,
            gradient_factor: 1.0,
            failure_base_rate: 0.001,
            voltage_sag_prob: 0.0005,
            single_track: false,
        },
    );
    segments.insert(
        "SEG-002".into(),
        Segment {
            id: "SEG-002".into(),
            from: "ST-002".into(),
            to: "ST-003".into(),
            distance_km: 40.0,
            base_speed_kmh: 120.0,
            gradient_factor: 1.0,
            failure_base_rate: 0.001,
            voltage_sag_prob: 0.0005,
            single_track: false,
        },
    );
    // Segment crossing NORTH→SOUTH
    segments.insert(
        "SEG-003".into(),
        Segment {
            id: "SEG-003".into(),
            from: "ST-003".into(),
            to: "ST-004".into(),
            distance_km: 60.0,
            base_speed_kmh: 100.0,
            gradient_factor: 1.2,
            failure_base_rate: 0.002,
            voltage_sag_prob: 0.001,
            single_track: true,
        },
    );
    // Segment within SOUTH
    segments.insert(
        "SEG-004".into(),
        Segment {
            id: "SEG-004".into(),
            from: "ST-004".into(),
            to: "ST-005".into(),
            distance_km: 35.0,
            base_speed_kmh: 140.0,
            gradient_factor: 1.0,
            failure_base_rate: 0.001,
            voltage_sag_prob: 0.0003,
            single_track: false,
        },
    );

    let seg_ids: Vec<String> = vec![
        "SEG-001".into(),
        "SEG-002".into(),
        "SEG-003".into(),
        "SEG-004".into(),
    ];
    legs.insert(
        "LEG-01".into(),
        rzd_core::network::Leg {
            id: "LEG-01".into(),
            name: "North-South".into(),
            segments: seg_ids,
            from_depot: "DEP-N1".into(),
            to_depot: "DEP-N1".into(),
            total_km: 185.0,
            road_id: "RD-01".into(),
        },
    );
    roads.insert(
        "RD-01".into(),
        rzd_core::network::Road {
            id: "RD-01".into(),
            name: "Main".into(),
            stations: vec![
                "ST-001".into(),
                "ST-002".into(),
                "ST-003".into(),
                "ST-004".into(),
                "ST-005".into(),
            ],
            depots: vec!["DEP-N1".into()],
            legs: vec!["LEG-01".into()],
            region_multiplier: 1.0,
        },
    );

    let mut region_norms = RegionNormsConfig::new();
    region_norms.add(
        "NORTH".into(),
        RegionNorms {
            region_id: "NORTH".into(),
            failure_rate_multiplier: 1.5,
            voltage_sag_multiplier: 2.0,
            speed_multiplier: 0.9,
            ..Default::default()
        },
    );
    region_norms.add(
        "SOUTH".into(),
        RegionNorms {
            region_id: "SOUTH".into(),
            failure_rate_multiplier: 0.8,
            voltage_sag_multiplier: 0.5,
            speed_multiplier: 1.1,
            ..Default::default()
        },
    );

    Network::new(stations, segments, legs, roads).with_region_norms(region_norms)
}

#[test]
fn test_region_for_segment() {
    let net = make_multi_region_network();
    assert_eq!(net.region_for_segment("SEG-001"), Some("NORTH"));
    assert_eq!(net.region_for_segment("SEG-004"), Some("SOUTH"));
    assert_eq!(net.region_for_segment("SEG-003"), Some("NORTH"));
    assert_eq!(net.region_for_segment("NONEXISTENT"), None);
}

#[test]
fn test_region_norms_failure_multiplier() {
    let net = make_multi_region_network();
    let eng = Engine::new(net, 42);

    let seg_north = eng.network.segments.get("SEG-001").unwrap();
    assert!((eng.region_multiplier_for_segment(seg_north) - 1.5).abs() < 0.01);

    let seg_south = eng.network.segments.get("SEG-004").unwrap();
    assert!((eng.region_multiplier_for_segment(seg_south) - 0.8).abs() < 0.01);
}

#[test]
fn test_region_norms_apply_overrides() {
    let mut norms = rzd_core::norms::RzdNorms::default();
    assert_eq!(norms.max_shift_8h_min, 480);

    let overrides = RegionNorms {
        region_id: "NORTH".into(),
        max_shift_8h_min: Some(420),
        min_home_rest_min: Some(720),
        ..Default::default()
    };
    norms.apply_region_overrides(&overrides);

    assert_eq!(norms.max_shift_8h_min, 420);
    assert_eq!(norms.min_home_rest_min, 720);
    assert_eq!(norms.max_shift_12h_min, 720); // unchanged
}

#[test]
fn test_region_norms_config_get_default() {
    let mut config = RegionNormsConfig::new();
    config.add(
        "NORTH".into(),
        RegionNorms {
            region_id: "NORTH".into(),
            failure_rate_multiplier: 2.0,
            ..Default::default()
        },
    );

    let north = config.get_default("NORTH");
    assert_eq!(north.failure_rate_multiplier, 2.0);

    let unknown = config.get_default("UNKNOWN");
    assert_eq!(unknown.failure_rate_multiplier, 1.0);
    assert_eq!(unknown.region_id, "UNKNOWN");
}

#[test]
fn test_network_serializes_region_norms() {
    let net = make_multi_region_network();
    let json = serde_json::to_string(&net).unwrap();
    let parsed: Network = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed.region_norms.regions.len(), 2);
    let north = parsed.region_norms.get_default("NORTH");
    assert!((north.failure_rate_multiplier - 1.5).abs() < 0.01);
}
