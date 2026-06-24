use rzd_core::network::Network;
use rzd_core::scenario::{generate_scenario, ScenarioConfig};
use std::collections::HashMap;

fn load_test_network() -> Network {
    // Try to load northern.json first, fall back to synthetic
    let northern_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent().unwrap()
        .join("data").join("networks").join("northern.json");

    if let Ok(json) = std::fs::read_to_string(&northern_path) {
        if let Ok(network) = Network::from_json(&json) {
            return network;
        }
    }

    let mut stations = HashMap::new();
    let mut segments = HashMap::new();

    for i in 1..=5 {
        let id = format!("ST-{:03}", i);
        stations.insert(id.clone(), rzd_core::network::Station {
            id: id.clone(),
            name: format!("Station {}", i),
            kind: if i == 1 { rzd_core::network::StationKind::MainDepot }
                   else if i == 5 { rzd_core::network::StationKind::Terminal }
                   else if i == 3 { rzd_core::network::StationKind::Turnaround }
                   else { rzd_core::network::StationKind::Waypoint },
            region: "NORTH".to_string(),
            coords: None,
            depot_id: if i == 1 { Some("DEP-01".to_string()) }
                      else if i == 3 { Some("DEP-02".to_string()) }
                      else { None },
            voltage_kv: rzd_core::network::VoltageKind::DC3000,
            electrification: true,
        });
        if i > 1 {
            let seg_id = format!("SEG-{:03}", i - 1);
            segments.insert(seg_id.clone(), rzd_core::network::Segment {
                id: seg_id,
                from: format!("ST-{:03}", i - 1),
                to: id,
                distance_km: 50.0,
                base_speed_kmh: 80.0,
                gradient_factor: 1.0,
                failure_base_rate: 0.001,
                voltage_sag_prob: 0.01,
                single_track: false,
            });
        }
    }

    let mut legs = HashMap::new();
    legs.insert("LEG-01".to_string(), rzd_core::network::Leg {
        id: "LEG-01".into(),
        name: "Test Route".into(),
        from_depot: "DEP-01".into(),
        to_depot: "DEP-02".into(),
        segments: vec!["SEG-001".into(), "SEG-002".into(), "SEG-003".into(), "SEG-004".into()],
        total_km: 200.0,
        road_id: "ROAD-1".into(),
    });

    let mut roads = HashMap::new();
    roads.insert("ROAD-1".to_string(), rzd_core::network::Road {
        id: "ROAD-1".into(),
        name: "Test Road".into(),
        stations: (1..=5).map(|i| format!("ST-{:03}", i)).collect(),
        depots: vec!["DEP-01".into(), "DEP-02".into()],
        legs: vec!["LEG-01".into()],
        region_multiplier: 1.0,
    });

    Network::new(stations, segments, legs, roads)
}

#[test]
fn test_generate_scenario_basic() {
    let network = load_test_network();
    let config = ScenarioConfig {
        crew_count: 10,
        loco_count: 5,
        seed: 42,
        duration_days: 30,
        ..Default::default()
    };

    let scenario = generate_scenario(&network, config);

    assert_eq!(scenario.crews.len(), 10);
    assert_eq!(scenario.locos.len(), 5);
}

#[test]
fn test_generate_scenario_deterministic() {
    let network = load_test_network();
    let config = ScenarioConfig {
        crew_count: 20,
        loco_count: 8,
        seed: 42,
        duration_days: 7,
        ..Default::default()
    };

    let s1 = generate_scenario(&network, config.clone());
    let s2 = generate_scenario(&network, config);

    assert_eq!(s1.crews.len(), s2.crews.len());
    assert_eq!(s1.locos.len(), s2.locos.len());

    let mut crews1: Vec<_> = s1.crews.iter().map(|c| (&c.id, &c.depot_id, &c.route_segments)).collect();
    let mut crews2: Vec<_> = s2.crews.iter().map(|c| (&c.id, &c.depot_id, &c.route_segments)).collect();
    crews1.sort();
    crews2.sort();

    for (c1, c2) in crews1.iter().zip(crews2.iter()) {
        assert_eq!(c1, c2);
    }
}

#[test]
fn test_generate_scenario_crews_have_routes() {
    let network = load_test_network();
    let config = ScenarioConfig {
        crew_count: 10,
        loco_count: 5,
        seed: 42,
        duration_days: 30,
        ..Default::default()
    };

    let scenario = generate_scenario(&network, config);

    let crews_with_routes = scenario.crews.iter()
        .filter(|c| !c.route_segments.is_empty())
        .count();

    assert!(crews_with_routes > 0, "At least some crews should have routes assigned");
}

#[test]
fn test_generate_scenario_locos_at_depots() {
    let network = load_test_network();
    let config = ScenarioConfig {
        crew_count: 10,
        loco_count: 5,
        seed: 42,
        duration_days: 30,
        ..Default::default()
    };

    let scenario = generate_scenario(&network, config);

    for loco in &scenario.locos {
        let station = network.stations.get(&loco.position);
        assert!(station.is_some(), "Loco position should be a valid station");
    }
}

#[test]
fn test_generate_scenario_large() {
    let network = load_test_network();
    let config = ScenarioConfig {
        crew_count: 100,
        loco_count: 30,
        seed: 42,
        duration_days: 30,
        ..Default::default()
    };

    let scenario = generate_scenario(&network, config);

    assert_eq!(scenario.crews.len(), 100);
    assert_eq!(scenario.locos.len(), 30);

    let crews_with_routes = scenario.crews.iter()
        .filter(|c| !c.route_segments.is_empty())
        .count();
    assert!(crews_with_routes > 10, "Some crews should have routes assigned");
}
