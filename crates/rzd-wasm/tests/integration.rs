use rzd_core::engine::Engine;
use rzd_core::entities::{Crew, CrewState, LocoKind, LocoState, Locomotive};
use rzd_core::events::EventKind;
use rzd_core::network::Network;
use rzd_core::prng::Mulberry32;
use rzd_core::scenario::{generate_scenario, ScenarioConfig, ScenarioDocument};
use rzd_wasm::RzdSimulatorWasm;

fn minimal_network_json() -> &'static str {
    r#"{
        "stations": {
            "ST-A": {"id":"ST-A","name":"А","kind":"MainDepot","region":"R1","depot_id":"DEP-1","voltage_kv":"AC25000","electrification":true},
            "ST-B": {"id":"ST-B","name":"Б","kind":"Turnaround","region":"R1","voltage_kv":"AC25000","electrification":true},
            "ST-C": {"id":"ST-C","name":"В","kind":"MainDepot","region":"R1","depot_id":"DEP-2","voltage_kv":"AC25000","electrification":true}
        },
        "segments": {
            "SEG-1": {"id":"SEG-1","from":"ST-A","to":"ST-B","distance_km":100.0,"base_speed_kmh":80.0,"gradient_factor":1.0,"failure_base_rate":0.001,"voltage_sag_prob":0.01,"single_track":false},
            "SEG-2": {"id":"SEG-2","from":"ST-B","to":"ST-C","distance_km":80.0,"base_speed_kmh":80.0,"gradient_factor":1.0,"failure_base_rate":0.001,"voltage_sag_prob":0.01,"single_track":false}
        },
        "legs": {
            "LEG-1": {"id":"LEG-1","name":"A-C","from_depot":"DEP-1","to_depot":"DEP-2","segments":["SEG-1","SEG-2"],"total_km":180.0,"road_id":"ROAD-1"}
        },
        "roads": {
            "ROAD-1": {"id":"ROAD-1","name":"Road 1","stations":["ST-A","ST-B","ST-C"],"depots":["DEP-1","DEP-2"],"legs":["LEG-1"],"region_multiplier":1.0}
        }
    }"#
}

fn minimal_network() -> Network {
    Network::from_json(minimal_network_json()).unwrap()
}

fn make_crew(id: &str, depot: &str) -> Crew {
    Crew::new(id.to_string(), depot.to_string(), "Engineer".to_string())
}

fn make_loco(id: &str, depot: &str) -> Locomotive {
    Locomotive {
        id: id.to_string(),
        series: "2TE25A".to_string(),
        loco_kind: LocoKind::Diesel,
        sections: 2,
        depot_id: depot.to_string(),
        state: LocoState::Available,
        position: depot.to_string(),
        mileage_km: 0.0,
        hours_since_to: 0.0,
        failures_total: 0,
        inventory_number: 0,
        manufacture_year: 2015,
    }
}

fn small_scenario_config() -> ScenarioConfig {
    ScenarioConfig {
        crew_count: 3,
        loco_count: 2,
        seed: 42,
        duration_days: 1,
        shift_spread_min: 5,
        night_shift_ratio: 0.0,
        allowed_depot_ids: vec![],
        allowed_leg_ids: vec![],
    }
}

fn run_small_simulation(seed: u32) -> Engine {
    let net = minimal_network();
    let config = ScenarioConfig {
        crew_count: 3,
        loco_count: 2,
        seed,
        duration_days: 1,
        shift_spread_min: 5,
        night_shift_ratio: 0.0,
        allowed_depot_ids: vec![],
        allowed_leg_ids: vec![],
    };
    let scenario = generate_scenario(&net, config);
    let mut engine = Engine::new(net, seed);
    for crew in scenario.crews {
        engine.add_crew(crew);
    }
    for loco in scenario.locos {
        engine.add_loco(loco);
    }
    engine.schedule_all_shifts(0, 60);
    engine.run(12 * 60);
    engine
}

// ====== Network tests ======

#[test]
fn network_from_json_parses_correctly() {
    let net = minimal_network();
    assert_eq!(net.stations.len(), 3);
    assert_eq!(net.segments.len(), 2);
    assert_eq!(net.legs.len(), 1);
    assert_eq!(net.roads.len(), 1);
}

#[test]
fn network_from_json_orphan_station_rejected() {
    let json = r#"{
        "stations": {
            "ST-A": {"id":"ST-A","name":"A","kind":"MainDepot","region":"R1","depot_id":"DEP-1","voltage_kv":"AC25000","electrification":true},
            "ST-ORPHAN": {"id":"ST-ORPHAN","name":"X","kind":"Waypoint","region":"R1","voltage_kv":"AC25000","electrification":true}
        },
        "segments": {
            "SEG-1": {"id":"SEG-1","from":"ST-A","to":"ST-A","distance_km":10.0,"base_speed_kmh":80.0,"gradient_factor":1.0,"failure_base_rate":0.001,"voltage_sag_prob":0.01,"single_track":false}
        },
        "legs": {},
        "roads": {
            "ROAD-1": {"id":"ROAD-1","name":"R","stations":["ST-A"],"depots":["DEP-1"],"legs":[],"region_multiplier":1.0}
        }
    }"#;
    let result = Network::from_json(json);
    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(err_msg.contains("Orphan station"));
}

#[test]
fn network_adjacency_built() {
    let net = minimal_network();
    let neighbors_a = net.get_neighbors(&"ST-A".to_string());
    assert_eq!(neighbors_a.len(), 1);
    assert_eq!(neighbors_a[0].0, "ST-B");
}

#[test]
fn network_segment_travel_time() {
    let net = minimal_network();
    let t = net.segment_travel_time_min(&"SEG-1".to_string()).unwrap();
    // 100km / 80kmh * 60 = 75 min
    assert!((t - 75.0).abs() < 0.01);
}

#[test]
fn network_shortest_path_a_to_c() {
    let net = minimal_network();
    let path = net
        .shortest_path(&"ST-A".to_string(), &"ST-C".to_string())
        .unwrap();
    assert_eq!(path.len(), 2);
    assert_eq!(path[0], "SEG-1");
    assert_eq!(path[1], "SEG-2");
}

#[test]
fn network_shortest_path_same_station() {
    let net = minimal_network();
    let path = net
        .shortest_path(&"ST-A".to_string(), &"ST-A".to_string())
        .unwrap();
    assert!(path.is_empty());
}

// ====== Engine basic tests ======

#[test]
fn engine_new_creates_empty_engine() {
    let engine = Engine::new(minimal_network(), 42);
    assert_eq!(engine.crew_count(), 0);
    assert_eq!(engine.loco_count(), 0);
    assert_eq!(engine.current_time, 0);
}

#[test]
fn engine_add_crew_and_loco() {
    let mut engine = Engine::new(minimal_network(), 42);
    engine.add_crew(make_crew("C1", "DEP-1"));
    engine.add_loco(make_loco("L1", "DEP-1"));
    assert_eq!(engine.crew_count(), 1);
    assert_eq!(engine.loco_count(), 1);
}

#[test]
fn engine_run_empty_produces_no_violations() {
    let mut engine = Engine::new(minimal_network(), 42);
    engine.run(1000);
    let summary = engine.violations_summary();
    assert_eq!(summary.total, 0);
}

#[test]
fn engine_warmup_suppresses_violations() {
    let mut engine = Engine::new(minimal_network(), 42);
    engine.add_crew(make_crew("C1", "DEP-1"));
    engine.add_loco(make_loco("L1", "DEP-1"));
    engine.set_warmup(60);
    engine.schedule_all_shifts(0, 10);
    engine.run(30);
    let summary = engine.violations_summary();
    assert_eq!(summary.total, 0);
}

// ====== Scenario generation tests ======

#[test]
fn generate_scenario_produces_crews_and_locos() {
    let scenario = generate_scenario(&minimal_network(), small_scenario_config());
    assert_eq!(scenario.crews.len(), 3);
    assert_eq!(scenario.locos.len(), 2);
}

#[test]
fn generate_scenario_crews_have_valid_depot() {
    let net = minimal_network();
    let scenario = generate_scenario(&net, ScenarioConfig::default());
    let depots: Vec<&str> = net
        .stations
        .values()
        .filter_map(|s| s.depot_id.as_deref())
        .collect();
    for crew in &scenario.crews {
        assert!(
            depots.contains(&crew.depot_id.as_str()),
            "Crew {} depot {} not in network depots",
            crew.id,
            crew.depot_id
        );
    }
}

#[test]
fn generate_scenario_locos_have_valid_depot() {
    let net = minimal_network();
    let scenario = generate_scenario(&net, ScenarioConfig::default());
    let depots: Vec<&str> = net
        .stations
        .values()
        .filter_map(|s| s.depot_id.as_deref())
        .collect();
    for loco in &scenario.locos {
        assert!(
            depots.contains(&loco.depot_id.as_str()),
            "Loco {} depot {} not in network depots",
            loco.id,
            loco.depot_id
        );
    }
}

#[test]
fn generate_scenario_deterministic_with_same_seed() {
    let net = minimal_network();
    let config = small_scenario_config();
    let s1 = generate_scenario(&net, config.clone());
    let s2 = generate_scenario(&net, config);
    assert_eq!(s1.crews.len(), s2.crews.len());
    assert_eq!(s1.locos.len(), s2.locos.len());
    assert_eq!(s1.crews[0].id, s2.crews[0].id);
}

// ====== Full simulation tests ======

#[test]
fn full_simulation_completes() {
    let engine = run_small_simulation(42);
    assert!(engine.current_time > 0);
}

#[test]
fn simulation_produces_events() {
    let engine = run_small_simulation(42);
    assert!(engine.stats.total_events_processed > 0);
}

#[test]
fn simulation_violations_nonnegative() {
    let engine = run_small_simulation(42);
    let summary = engine.violations_summary();
    assert!(summary.total <= 100);
}

// ====== Stats and export tests ======

#[test]
fn engine_stats_returns_reference() {
    let mut engine = Engine::new(minimal_network(), 42);
    engine.add_crew(make_crew("C1", "DEP-1"));
    engine.add_loco(make_loco("L1", "DEP-1"));
    let stats = engine.stats();
    assert_eq!(stats.total_events_processed, 0);
}

#[test]
fn engine_violations_summary_returns_data() {
    let engine = Engine::new(minimal_network(), 42);
    let summary = engine.violations_summary();
    assert_eq!(summary.total, 0);
}

#[test]
fn engine_export_ced_no_panic() {
    let mut engine = Engine::new(minimal_network(), 42);
    engine.add_crew(make_crew("C1", "DEP-1"));
    engine.add_loco(make_loco("L1", "DEP-1"));
    let ced = engine.export_ced();
    assert!(
        ced.trips.is_empty(),
        "CED with no run should have empty trips"
    );
}

// ====== Crew state machine tests ======

#[test]
fn crew_state_athome_to_appearing() {
    assert!(CrewState::AtHome.can_transition_to(&CrewState::Appearing));
}

#[test]
fn crew_state_athome_not_to_working() {
    assert!(!CrewState::AtHome.can_transition_to(&CrewState::Working));
}

#[test]
fn crew_state_medcheck_to_pretrip() {
    assert!(CrewState::MedCheck.can_transition_to(&CrewState::PreTripCheck));
}

#[test]
fn crew_state_pretrip_to_acceptance() {
    assert!(CrewState::PreTripCheck.can_transition_to(&CrewState::Acceptance));
}

#[test]
fn crew_state_acceptance_to_waiting() {
    assert!(CrewState::Acceptance.can_transition_to(&CrewState::Waiting));
}

#[test]
fn crew_state_waiting_to_working() {
    assert!(CrewState::Waiting.can_transition_to(&CrewState::Working));
}

#[test]
fn crew_state_working_to_waiting() {
    assert!(CrewState::Working.can_transition_to(&CrewState::Waiting));
}

#[test]
fn crew_state_working_to_reserve() {
    assert!(CrewState::Working.can_transition_to(&CrewState::Reserve));
}

#[test]
fn crew_state_working_to_resting_away() {
    assert!(CrewState::Working.can_transition_to(&CrewState::RestingAway));
}

#[test]
fn crew_state_working_to_finished() {
    assert!(CrewState::Working.can_transition_to(&CrewState::Finished));
}

#[test]
fn crew_state_resting_away_to_working() {
    assert!(CrewState::RestingAway.can_transition_to(&CrewState::Working));
}

#[test]
fn crew_state_finished_to_resting_home() {
    assert!(CrewState::Finished.can_transition_to(&CrewState::RestingHome));
}

#[test]
fn crew_state_resting_home_to_athome() {
    assert!(CrewState::RestingHome.can_transition_to(&CrewState::AtHome));
}

#[test]
fn crew_state_athome_to_finished() {
    assert!(!CrewState::AtHome.can_transition_to(&CrewState::Finished));
}

#[test]
fn crew_state_invalid_athome_to_resting_away() {
    assert!(!CrewState::AtHome.can_transition_to(&CrewState::RestingAway));
}

#[test]
fn crew_state_invalid_working_to_athome() {
    assert!(!CrewState::Working.can_transition_to(&CrewState::AtHome));
}

#[test]
fn crew_state_invalid_medcheck_to_working() {
    assert!(!CrewState::MedCheck.can_transition_to(&CrewState::Working));
}

// ====== Event queue tests ======

#[test]
fn event_queue_push_returns_increasing_ids() {
    let mut queue = rzd_core::events::EventQueue::new();
    let id1 = queue.push(rzd_core::events::SimEvent {
        time: 10,
        kind: EventKind::ShiftAppear,
        crew_id: None,
        loco_id: None,
        segment_id: None,
        priority: 1,
        id: 0,
    });
    let id2 = queue.push(rzd_core::events::SimEvent {
        time: 20,
        kind: EventKind::ShiftAppear,
        crew_id: None,
        loco_id: None,
        segment_id: None,
        priority: 1,
        id: 0,
    });
    assert_eq!(id1, 1);
    assert_eq!(id2, 2);
}

#[test]
fn event_queue_pop_returns_earliest_first() {
    let mut queue = rzd_core::events::EventQueue::new();
    queue.push(rzd_core::events::SimEvent {
        time: 100,
        kind: EventKind::ShiftAppear,
        crew_id: None,
        loco_id: None,
        segment_id: None,
        priority: 1,
        id: 0,
    });
    queue.push(rzd_core::events::SimEvent {
        time: 10,
        kind: EventKind::ShiftAppear,
        crew_id: None,
        loco_id: None,
        segment_id: None,
        priority: 1,
        id: 0,
    });
    let first = queue.pop().unwrap();
    assert_eq!(first.time, 10);
}

#[test]
fn event_queue_cancel_removes_event() {
    let mut queue = rzd_core::events::EventQueue::new();
    let id = queue.push(rzd_core::events::SimEvent {
        time: 10,
        kind: EventKind::ShiftAppear,
        crew_id: None,
        loco_id: None,
        segment_id: None,
        priority: 1,
        id: 0,
    });
    queue.cancel(id);
    assert!(queue.pop().is_none());
}

#[test]
fn event_queue_cancel_only_removes_target() {
    let mut queue = rzd_core::events::EventQueue::new();
    let id1 = queue.push(rzd_core::events::SimEvent {
        time: 10,
        kind: EventKind::ShiftAppear,
        crew_id: None,
        loco_id: None,
        segment_id: None,
        priority: 1,
        id: 0,
    });
    let _id2 = queue.push(rzd_core::events::SimEvent {
        time: 20,
        kind: EventKind::ShiftAppear,
        crew_id: None,
        loco_id: None,
        segment_id: None,
        priority: 1,
        id: 0,
    });
    queue.cancel(id1);
    let next = queue.pop().unwrap();
    assert_eq!(next.time, 20);
}

#[test]
fn event_queue_clear_resets() {
    let mut queue = rzd_core::events::EventQueue::new();
    queue.push(rzd_core::events::SimEvent {
        time: 10,
        kind: EventKind::ShiftAppear,
        crew_id: None,
        loco_id: None,
        segment_id: None,
        priority: 1,
        id: 0,
    });
    queue.clear();
    assert!(queue.is_empty());
    assert_eq!(queue.len(), 0);
}

#[test]
fn event_queue_priority_ordering() {
    let mut queue = rzd_core::events::EventQueue::new();
    queue.push(rzd_core::events::SimEvent {
        time: 10,
        kind: EventKind::ShiftAppear,
        crew_id: None,
        loco_id: None,
        segment_id: None,
        priority: 5,
        id: 0,
    });
    queue.push(rzd_core::events::SimEvent {
        time: 10,
        kind: EventKind::ShiftAppear,
        crew_id: None,
        loco_id: None,
        segment_id: None,
        priority: 1,
        id: 0,
    });
    let first = queue.pop().unwrap();
    assert_eq!(first.priority, 5);
}

// ====== Region norms tests ======

#[test]
fn network_region_for_segment() {
    let net = minimal_network();
    let region = net.region_for_segment("SEG-1").unwrap();
    assert_eq!(region, "R1");
}

#[test]
fn network_with_region_norms() {
    use rzd_core::norms::{RegionNorms, RegionNormsConfig};
    let mut config = RegionNormsConfig::new();
    config.regions.insert(
        "R1".to_string(),
        RegionNorms {
            region_id: "R1".to_string(),
            failure_rate_multiplier: 1.5,
            voltage_sag_multiplier: 2.0,
            speed_multiplier: 0.8,
            ..Default::default()
        },
    );
    let net = minimal_network().with_region_norms(config);
    let region = net.region_for_segment("SEG-1").unwrap();
    assert_eq!(region, "R1");
}

// ====== Locomotive tests ======

#[test]
fn loco_initial_state_is_available() {
    assert_eq!(make_loco("L1", "DEP-1").state, LocoState::Available);
}

#[test]
fn loco_mileage_initial_zero() {
    assert_eq!(make_loco("L1", "DEP-1").mileage_km, 0.0);
}

// ====== Scenario config tests ======

#[test]
fn scenario_config_default_values() {
    let config = ScenarioConfig::default();
    assert_eq!(config.crew_count, 50);
    assert_eq!(config.loco_count, 20);
    assert_eq!(config.seed, 42);
    assert_eq!(config.duration_days, 30);
    assert_eq!(config.shift_spread_min, 30);
    assert_eq!(config.night_shift_ratio, 0.3);
}

#[test]
fn scenario_config_serialization_roundtrip() {
    let config = ScenarioConfig::default();
    let json = serde_json::to_string(&config).unwrap();
    let parsed: ScenarioConfig = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed.crew_count, config.crew_count);
    assert_eq!(parsed.seed, config.seed);
}

#[test]
fn wasm_load_scenario_accepts_versioned_scenario_document() {
    let document = ScenarioDocument::new(small_scenario_config());
    let scenario_json =
        serde_json::to_string(&document).expect("scenario document should serialize");

    let mut simulator = RzdSimulatorWasm::new();
    simulator
        .load_network(minimal_network_json())
        .expect("network should load");
    simulator
        .load_scenario(&scenario_json)
        .expect("WASM should accept rzd-des-scenario/v1");

    let summary: serde_json::Value = serde_json::from_str(
        &simulator
            .get_summary()
            .expect("loaded scenario should have a summary"),
    )
    .expect("summary should be JSON");
    assert_eq!(summary["crew_count"], 3);
    assert_eq!(summary["loco_count"], 2);
}

#[test]
fn wasm_generate_and_load_accepts_versioned_scenario_document() {
    let document = ScenarioDocument::new(small_scenario_config());
    let scenario_json =
        serde_json::to_string(&document).expect("scenario document should serialize");

    let mut simulator = RzdSimulatorWasm::new();
    simulator
        .generate_and_load_scenario(minimal_network_json(), &scenario_json)
        .expect("WASM generation should accept rzd-des-scenario/v1");

    let summary: serde_json::Value = serde_json::from_str(
        &simulator
            .get_summary()
            .expect("generated scenario should have a summary"),
    )
    .expect("summary should be JSON");
    assert_eq!(summary["crew_count"], 3);
    assert_eq!(summary["loco_count"], 2);
}

#[cfg(target_arch = "wasm32")]
mod browser_tests {
    use super::minimal_network_json;
    use rzd_wasm::RzdSimulatorWasm;
    use wasm_bindgen_test::*;

    wasm_bindgen_test_configure!(run_in_browser);

    #[wasm_bindgen_test]
    fn wasm_rejects_unknown_versioned_scenario_schema() {
        let mut simulator = RzdSimulatorWasm::new();
        simulator
            .load_network(minimal_network_json())
            .expect("network should load");

        let error = simulator
            .load_scenario(
                r#"{"schema":"rzd-des-scenario/v99","config":{"crew_count":1,"loco_count":1,"seed":42,"duration_days":1}}"#,
            )
            .expect_err("unknown scenario schema must be rejected");
        assert!(
            error
                .as_string()
                .is_some_and(|message| message.contains("unsupported scenario schema")),
            "unexpected WASM error: {error:?}"
        );
    }
}

// ====== PRNG tests ======

#[test]
fn mulberry32_deterministic() {
    let mut prng1 = Mulberry32::new(42);
    let mut prng2 = Mulberry32::new(42);
    let vals1: Vec<u32> = (0..10).map(|_| prng1.next_u32()).collect();
    let vals2: Vec<u32> = (0..10).map(|_| prng2.next_u32()).collect();
    assert_eq!(vals1, vals2);
}

#[test]
fn mulberry32_different_seeds_different_output() {
    let mut prng1 = Mulberry32::new(1);
    let mut prng2 = Mulberry32::new(2);
    let vals1: Vec<u32> = (0..5).map(|_| prng1.next_u32()).collect();
    let vals2: Vec<u32> = (0..5).map(|_| prng2.next_u32()).collect();
    assert_ne!(vals1, vals2);
}

// ====== Large simulation stress test ======

#[test]
fn large_simulation_100_crews_7_days() {
    let net = minimal_network();
    let config = ScenarioConfig {
        crew_count: 100,
        loco_count: 30,
        seed: 123,
        duration_days: 7,
        shift_spread_min: 30,
        night_shift_ratio: 0.3,
        allowed_depot_ids: vec![],
        allowed_leg_ids: vec![],
    };
    let scenario = generate_scenario(&net, config);
    let mut engine = Engine::new(net, 123);

    for crew in scenario.crews {
        engine.add_crew(crew);
    }
    for loco in scenario.locos {
        engine.add_loco(loco);
    }

    engine.schedule_all_shifts(0, 10);
    engine.run(7 * 24 * 60);

    assert!(engine.stats.total_events_processed > 0);
    let summary = engine.violations_summary();
    assert!(summary.total <= 10000);
}

// ====== Northern network CLI integration test ======

#[test]
fn northern_network_loads_and_simulates() {
    let json = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/networks/northern.json"),
    )
    .expect("Failed to read northern.json");
    let net = Network::from_json(&json).unwrap();
    assert_eq!(net.stations.len(), 35);
    assert_eq!(net.segments.len(), 38);

    let config = ScenarioConfig {
        crew_count: 50,
        loco_count: 20,
        seed: 42,
        duration_days: 7,
        shift_spread_min: 30,
        night_shift_ratio: 0.3,
        allowed_depot_ids: vec![],
        allowed_leg_ids: vec![],
    };
    let scenario = generate_scenario(&net, config);
    let mut engine = Engine::new(net, 42);

    for crew in scenario.crews {
        engine.add_crew(crew);
    }
    for loco in scenario.locos {
        engine.add_loco(loco);
    }

    engine.schedule_all_shifts(0, 10);
    engine.run(7 * 24 * 60);

    assert!(engine.stats.total_events_processed > 0);
    assert!(engine.crew_count() > 0);
    assert!(engine.loco_count() > 0);
}
