use rzd_core::engine::{Engine, EngineSnapshot};
use rzd_core::network::Network;
use rzd_core::scenario::{generate_scenario, ScenarioConfig};

fn load_synthetic_network() -> Network {
    let network_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("data")
        .join("networks")
        .join("synthetic_one_road_88_05.json");
    let json = std::fs::read_to_string(network_path).expect("synthetic network should exist");
    Network::from_json(&json).expect("synthetic network should parse")
}

fn build_engine(network: &Network, config: ScenarioConfig) -> Engine {
    let scenario = generate_scenario(network, config.clone());
    let mut engine = Engine::new(network.clone(), config.seed);
    for crew in scenario.crews {
        engine.add_crew(crew);
    }
    for loco in scenario.locos {
        engine.add_loco(loco);
    }
    engine.schedule_all_shifts(60, 10);
    engine
}

#[test]
fn snapshot_roundtrip_preserves_serialized_state() {
    let network = load_synthetic_network();
    let config = ScenarioConfig {
        crew_count: 24,
        loco_count: 12,
        seed: 20260708,
        duration_days: 5,
        ..Default::default()
    };
    let checkpoint = 2 * 1440;

    let mut split = build_engine(&network, config);
    split.run(checkpoint);
    let snapshot = split.snapshot();
    let snapshot_json =
        serde_json::to_string(&snapshot).expect("snapshot should serialize to JSON");
    let restored_snapshot: EngineSnapshot =
        serde_json::from_str(&snapshot_json).expect("snapshot should deserialize from JSON");
    assert_eq!(snapshot.schema, restored_snapshot.schema);
    assert_eq!(snapshot.current_time, restored_snapshot.current_time);
    assert_eq!(snapshot.crews.len(), restored_snapshot.crews.len());
    assert_eq!(snapshot.locos.len(), restored_snapshot.locos.len());
    assert_eq!(snapshot.tu3_rows.len(), restored_snapshot.tu3_rows.len());
    assert_eq!(
        snapshot.queue.pending_events.len(),
        restored_snapshot.queue.pending_events.len()
    );
    assert_eq!(
        snapshot.violations.violations.len(),
        restored_snapshot.violations.violations.len()
    );
    assert_eq!(
        snapshot.processed_events.len(),
        restored_snapshot.processed_events.len()
    );
}

#[test]
fn restored_snapshot_can_continue_running() {
    let network = load_synthetic_network();
    let config = ScenarioConfig {
        crew_count: 24,
        loco_count: 12,
        seed: 20260708,
        duration_days: 5,
        ..Default::default()
    };
    let checkpoint = 2 * 1440;
    let until = (config.duration_days as u64) * 1440;

    let mut engine = build_engine(&network, config);
    engine.run(checkpoint);
    let snapshot = engine.snapshot();
    let mut restored = Engine::from_snapshot(snapshot).expect("snapshot should restore");
    let initial_events = restored.stats().total_events_processed;
    restored.run(until);

    assert!(
        restored.stats().total_events_processed >= initial_events,
        "restored engine should continue processing events"
    );
    assert!(
        restored.stats().total_shifts_completed > 0,
        "restored engine should complete shifts after resume"
    );
}

fn business_signature(engine: &Engine) -> String {
    let summary = engine.violations_summary();
    let mut by_type: Vec<(String, u32)> = summary
        .by_type
        .into_iter()
        .map(|(kind, count)| (format!("{kind:?}"), count))
        .collect();
    by_type.sort();

    let mut by_severity: Vec<(String, u32)> = summary.by_severity.into_iter().collect();
    by_severity.sort();

    format!(
        "events={}|violations={}|types={:?}|severity={:?}|shifts={}|distance={:.3}|tu3={}|errors={}",
        engine.stats().total_events_processed,
        summary.total,
        by_type,
        by_severity,
        engine.stats().total_shifts_completed,
        engine.stats().total_distance_km,
        engine.tu3_rows.len(),
        engine.event_errors
    )
}

#[test]
fn json_roundtrip_snapshot_matches_full_run_business_results() {
    let network = load_synthetic_network();
    let config = ScenarioConfig {
        crew_count: 24,
        loco_count: 12,
        seed: 20260708,
        duration_days: 5,
        ..Default::default()
    };
    let checkpoint = 300;
    let until = (config.duration_days as u64) * 1440;

    let mut full = build_engine(&network, config.clone());
    full.run(until);

    let mut split = build_engine(&network, config);
    split.run(checkpoint);
    let snapshot_json =
        serde_json::to_string(&split.snapshot()).expect("snapshot should serialize to JSON");
    let restored_snapshot: EngineSnapshot =
        serde_json::from_str(&snapshot_json).expect("snapshot should deserialize from JSON");
    let mut resumed = Engine::from_snapshot(restored_snapshot).expect("snapshot should restore");
    resumed.run(until);

    assert_eq!(
        business_signature(&full),
        business_signature(&resumed),
        "resume after JSON snapshot roundtrip must preserve business results"
    );
}

#[test]
fn snapshot_resume_preserves_event_trace_continuity() {
    let network = load_synthetic_network();
    let config = ScenarioConfig {
        crew_count: 24,
        loco_count: 12,
        seed: 20260708,
        duration_days: 5,
        ..Default::default()
    };
    let checkpoint = 300;
    let until = (config.duration_days as u64) * 1440;

    let mut full = build_engine(&network, config.clone());
    full.run(until);
    let full_trace = full.export_event_trace();

    let mut split = build_engine(&network, config);
    split.run(checkpoint);
    let snapshot_json =
        serde_json::to_string(&split.snapshot()).expect("snapshot should serialize to JSON");
    let restored_snapshot: EngineSnapshot =
        serde_json::from_str(&snapshot_json).expect("snapshot should deserialize from JSON");
    let mut resumed = Engine::from_snapshot(restored_snapshot).expect("snapshot should restore");
    resumed.run(until);
    let resumed_trace = resumed.export_event_trace();

    assert_eq!(full_trace.schema, resumed_trace.schema);
    assert_eq!(full_trace.until_minutes, resumed_trace.until_minutes);
    assert_eq!(full_trace.events.len(), resumed_trace.events.len());
    assert_eq!(full_trace.events, resumed_trace.events);
}
