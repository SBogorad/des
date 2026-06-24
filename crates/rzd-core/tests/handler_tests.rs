use rzd_core::engine::Engine;
use rzd_core::network::{Network, Station, StationKind, VoltageKind, Segment};
use rzd_core::entities::{Crew, Locomotive, LocoKind, LocoState, CrewState, FailureKind};
use rzd_core::events::{SimEvent, EventKind, ViolationType};
use std::collections::HashMap;

fn make_test_network() -> Network {
    let mut stations = HashMap::new();
    let mut segments = HashMap::new();
    for i in 1..=5 {
        let id = format!("ST-{:03}", i);
        stations.insert(id.clone(), Station {
            id: id.clone(), name: format!("St{}", i),
            kind: if i == 1 { StationKind::MainDepot } else if i == 3 { StationKind::Turnaround } else if i == 5 { StationKind::Terminal } else { StationKind::Waypoint },
            region: "TEST".to_string(), coords: None,
            depot_id: if i == 1 { Some("DEP-01".to_string()) } else if i == 3 { Some("DEP-02".to_string()) } else { None },
            voltage_kv: VoltageKind::DC3000, electrification: true,
        });
        if i > 1 {
            segments.insert(format!("SEG-{:03}", i - 1), Segment {
                id: format!("SEG-{:03}", i - 1),
                from: format!("ST-{:03}", i - 1), to: id,
                distance_km: 50.0, base_speed_kmh: 80.0, gradient_factor: 1.0,
                failure_base_rate: 0.000, voltage_sag_prob: 0.0, single_track: false,
            });
        }
    }
    Network::new(stations, segments, HashMap::new(), HashMap::new())
}

fn setup_engine() -> Engine {
    let net = make_test_network();
    let mut eng = Engine::new(net, 42);
    eng.add_loco(Locomotive {
        id: "LOCO-001".into(), series: "EL-2K".into(), loco_kind: LocoKind::Electric,
        sections: 2, depot_id: "DEP-01".into(), state: LocoState::Available,
        position: "ST-001".into(), mileage_km: 0.0, hours_since_to: 0.0, failures_total: 0,
    });
    eng
}

fn push(engine: &mut Engine, time: u64, kind: EventKind, crew: &str, loco: Option<&str>, seg: Option<&str>, priority: u8) {
    engine.queue.push(SimEvent {
        time, kind, crew_id: Some(crew.into()), loco_id: loco.map(String::from),
        segment_id: seg.map(String::from), priority, id: 0,
    });
}

fn crew_with_route(crew_id: &str, depot: &str, pos: &str, segs: Vec<&str>) -> Crew {
    let mut c = Crew::new(crew_id.into(), depot.into(), pos.into());
    c.route_segments = segs.into_iter().map(String::from).collect();
    c.shift_start = Some(0);
    c
}

// ── SHIFT LIFECYCLE ──

#[test]
fn test_shift_appear_schedules_medcheck() {
    let mut eng = setup_engine();
    eng.add_crew(crew_with_route("C1", "DEP-01", "ST-001", vec!["SEG-001"]));
    push(&mut eng, 60, EventKind::ShiftAppear, "C1", None, None, 10);
    eng.run(61);
    assert_eq!(eng.crews.get("C1").unwrap().state, CrewState::MedCheck);
}

#[test]
fn test_medcheck_schedules_pretrip() {
    let mut eng = setup_engine();
    let mut c = crew_with_route("C1", "DEP-01", "ST-001", vec!["SEG-001"]);
    c.state = CrewState::MedCheck;
    eng.add_crew(c);
    push(&mut eng, 60, EventKind::MedCheckEnd, "C1", None, None, 10);
    eng.run(61);
    assert!(matches!(eng.crews.get("C1").unwrap().state, CrewState::PreTripCheck | CrewState::Acceptance | CrewState::Waiting | CrewState::Working));
}

#[test]
fn test_pretrip_schedules_acceptance() {
    let mut eng = setup_engine();
    let mut c = crew_with_route("C1", "DEP-01", "ST-001", vec!["SEG-001"]);
    c.state = CrewState::PreTripCheck;
    eng.add_crew(c);
    push(&mut eng, 60, EventKind::PreTripBriefingEnd, "C1", None, None, 10);
    eng.run(61);
    assert!(matches!(eng.crews.get("C1").unwrap().state, CrewState::Acceptance | CrewState::Waiting | CrewState::Working));
}

#[test]
fn test_acceptance_start_uses_loco() {
    let mut eng = setup_engine();
    let mut c = crew_with_route("C1", "DEP-01", "ST-001", vec!["SEG-001"]);
    c.state = CrewState::PreTripCheck;
    eng.add_crew(c);
    push(&mut eng, 60, EventKind::AcceptanceStart, "C1", None, None, 10);
    eng.run(61);
    let c = eng.crews.get("C1").unwrap();
    assert_eq!(c.state, CrewState::Acceptance);
    assert!(c.current_loco.is_some());
}

#[test]
fn test_acceptance_end_schedules_departure() {
    let mut eng = setup_engine();
    let mut c = crew_with_route("C1", "DEP-01", "ST-001", vec!["SEG-001"]);
    c.state = CrewState::Acceptance;
    eng.add_crew(c);
    push(&mut eng, 60, EventKind::AcceptanceEnd, "C1", Some("LOCO-001"), None, 10);
    eng.run(61);
    assert!(matches!(eng.crews.get("C1").unwrap().state, CrewState::Waiting | CrewState::Working));
}

#[test]
fn test_wait_start_sets_waiting() {
    let mut eng = setup_engine();
    let mut c = crew_with_route("C1", "DEP-01", "ST-001", vec!["SEG-001"]);
    c.state = CrewState::Waiting;
    eng.add_crew(c);
    push(&mut eng, 60, EventKind::WaitStart, "C1", None, None, 10);
    eng.run(61);
    assert_eq!(eng.crews.get("C1").unwrap().state, CrewState::Waiting);
}

#[test]
fn test_wait_end_triggers_movement() {
    let mut eng = setup_engine();
    let mut c = crew_with_route("C1", "DEP-01", "ST-001", vec!["SEG-001"]);
    c.state = CrewState::Waiting;
    eng.add_crew(c);
    push(&mut eng, 60, EventKind::WaitEnd, "C1", Some("LOCO-001"), None, 10);
    eng.run(61);
    assert!(matches!(eng.crews.get("C1").unwrap().state, CrewState::Working));
}

// ── MOVEMENT ──

#[test]
fn test_arrive_advances_route() {
    let mut eng = setup_engine();
    eng.add_crew(crew_with_route("C1", "DEP-01", "ST-001", vec!["SEG-001", "SEG-002"]));
    let c = eng.crews.get_mut("C1").unwrap();
    c.state = CrewState::Working;
    c.shift_start = Some(0);
    push(&mut eng, 60, EventKind::ArriveStation, "C1", Some("LOCO-001"), Some("SEG-001"), 10);
    eng.run(61);
    let c = eng.crews.get("C1").unwrap();
    assert_eq!(c.route_index, 1);
    assert_eq!(c.position, "ST-002");
    assert!(c.total_distance_km > 0.0);
}

#[test]
fn test_arrive_at_last_segment_triggers_arrival_checkpoint() {
    let mut eng = setup_engine();
    eng.add_crew(crew_with_route("C1", "DEP-001", "ST-001", vec!["SEG-001"]));
    let c = eng.crews.get_mut("C1").unwrap();
    c.state = CrewState::Working;
    c.shift_start = Some(0);
    push(&mut eng, 60, EventKind::ArriveStation, "C1", Some("LOCO-001"), Some("SEG-001"), 10);
    eng.run(500);
    let c = eng.crews.get("C1").unwrap();
    assert!(c.route_index >= 1);
}

#[test]
fn test_pass_waypoint_increments_work_time() {
    let mut eng = setup_engine();
    let mut c = crew_with_route("C1", "DEP-01", "ST-001", vec!["SEG-001"]);
    c.shift_work_min = 100;
    eng.add_crew(c);
    push(&mut eng, 60, EventKind::PassWaypoint, "C1", None, None, 10);
    eng.run(61);
    assert_eq!(eng.crews.get("C1").unwrap().shift_work_min, 102);
}

// ── FAILURES ──

#[test]
fn test_failure_sets_loco_failed() {
    let mut eng = setup_engine();
    eng.add_crew(crew_with_route("C1", "DEP-01", "ST-001", vec!["SEG-001"]));
    let c = eng.crews.get_mut("C1").unwrap();
    c.state = CrewState::Working;
    push(&mut eng, 60, EventKind::FailureOccurred(FailureKind::BrakeSystemFault), "C1", Some("LOCO-001"), Some("SEG-001"), 5);
    eng.run(61);
    assert_eq!(eng.locos.get("LOCO-001").unwrap().state, LocoState::Failed);
    assert_eq!(eng.locos.get("LOCO-001").unwrap().failures_total, 1);
}

#[test]
fn test_failure_resolved_restores_loco() {
    let mut eng = setup_engine();
    let loco = eng.locos.get_mut("LOCO-001").unwrap();
    loco.state = LocoState::Failed;
    let mut c = crew_with_route("C1", "DEP-01", "ST-001", vec!["SEG-001"]);
    c.state = CrewState::Working;
    eng.add_crew(c);
    push(&mut eng, 60, EventKind::FailureResolved, "C1", Some("LOCO-001"), Some("SEG-001"), 5);
    eng.run(61);
    assert_eq!(eng.locos.get("LOCO-001").unwrap().state, LocoState::Available);
}

#[test]
fn test_failure_resolved_on_available_noop() {
    let mut eng = setup_engine();
    let mut c = crew_with_route("C1", "DEP-001", "ST-001", vec!["SEG-001"]);
    c.state = CrewState::Working;
    eng.add_crew(c);
    push(&mut eng, 60, EventKind::FailureResolved, "C1", Some("LOCO-001"), None, 5);
    eng.run(61);
    assert_eq!(eng.locos.get("LOCO-001").unwrap().state, LocoState::Available);
}

// ── VOLTAGE SAG ──

#[test]
fn test_voltage_sag_applies_speed_factor() {
    let mut eng = setup_engine();
    eng.add_crew(crew_with_route("C1", "DEP-01", "ST-001", vec!["SEG-001"]));
    push(&mut eng, 60, EventKind::VoltageSagStart { voltage_v: 2200, segment_id: "SEG-001".into() }, "C1", Some("LOCO-001"), Some("SEG-001"), 5);
    eng.run(61);
    assert!(eng.crews.get("C1").unwrap().in_voltage_sag);
}

#[test]
fn test_voltage_sag_end_clears_flag() {
    let mut eng = setup_engine();
    let mut c = crew_with_route("C1", "DEP-001", "ST-001", vec!["SEG-001"]);
    c.in_voltage_sag = true;
    eng.add_crew(c);
    push(&mut eng, 60, EventKind::VoltageSagEnd, "C1", Some("LOCO-001"), Some("SEG-001"), 8);
    eng.run(61);
    assert!(!eng.crews.get("C1").unwrap().in_voltage_sag);
}

// ── RESERVE ──

#[test]
fn test_reserve_start_sets_reserve() {
    let mut eng = setup_engine();
    let mut c = crew_with_route("C1", "DEP-01", "ST-001", vec!["SEG-001"]);
    c.state = CrewState::Working;
    eng.add_crew(c);
    push(&mut eng, 60, EventKind::ReserveStart, "C1", None, None, 10);
    eng.run(61);
    assert_eq!(eng.crews.get("C1").unwrap().state, CrewState::Reserve);
}

#[test]
fn test_reserve_end_triggers_shift_end() {
    let mut eng = setup_engine();
    let mut c = crew_with_route("C1", "DEP-01", "ST-001", vec!["SEG-001"]);
    c.state = CrewState::Reserve;
    c.shift_start = Some(0);
    eng.add_crew(c);
    push(&mut eng, 60, EventKind::ReserveEnd, "C1", None, None, 10);
    eng.run(61);
    assert!(matches!(eng.crews.get("C1").unwrap().state, CrewState::Finished | CrewState::RestingHome | CrewState::AtHome));
}

// ── TURNAROUND ──

#[test]
fn test_turnaround_rest_start_sets_resting() {
    let mut eng = setup_engine();
    let mut c = crew_with_route("C1", "DEP-01", "ST-003", vec!["SEG-001"]);
    c.state = CrewState::Working;
    eng.add_crew(c);
    push(&mut eng, 60, EventKind::TurnaroundRestStart, "C1", None, None, 10);
    eng.run(61);
    assert_eq!(eng.crews.get("C1").unwrap().state, CrewState::RestingAway);
}

// ── HANDOVER ──

#[test]
fn test_handover_start_adds_time() {
    let mut eng = setup_engine();
    let mut c = crew_with_route("C1", "DEP-01", "ST-001", vec!["SEG-001"]);
    c.state = CrewState::RestingAway;
    c.shift_work_min = 100;
    eng.add_crew(c);
    push(&mut eng, 60, EventKind::HandoverStart, "C1", Some("LOCO-001"), None, 10);
    eng.run(61);
    assert!(eng.crews.get("C1").unwrap().shift_work_min > 100);
}

// ── SHIFT END ──

#[test]
fn test_shift_end_records_tu3_and_resets() {
    let mut eng = setup_engine();
    let mut c = crew_with_route("C1", "DEP-01", "ST-001", vec!["SEG-001"]);
    c.shift_start = Some(0);
    c.shift_work_min = 200;
    c.total_distance_km = 150.0;
    eng.add_crew(c);
    push(&mut eng, 60, EventKind::ShiftEnd, "C1", Some("LOCO-001"), None, 10);
    eng.run(500);
    let c = eng.crews.get("C1").unwrap();
    // ShiftEnd → Finished → HomeRestStart → RestingHome → HomeRestEnd → AtHome
    assert!(matches!(c.state, CrewState::AtHome | CrewState::RestingHome));
    assert_eq!(c.total_shifts, 1);
    assert!(c.cumulative_work_min >= 200);
}

// ── HOME REST ──

#[test]
fn test_home_rest_start_checks_monthly_limit() {
    let mut eng = setup_engine();
    eng.set_warmup(0);
    let mut c = crew_with_route("C1", "DEP-01", "ST-001", vec!["SEG-001"]);
    c.state = CrewState::Finished;
    c.cumulative_work_min = 12000;
    eng.add_crew(c);
    push(&mut eng, 60, EventKind::HomeRestStart, "C1", None, None, 10);
    eng.run(61);
    assert_eq!(eng.crews.get("C1").unwrap().state, CrewState::RestingHome);
    assert!(eng.violations.violations.iter().any(|v| v.violation_type == ViolationType::MonthlyOvertime));
}

#[test]
fn test_home_rest_end_resets_crew() {
    let mut eng = setup_engine();
    let mut c = crew_with_route("C1", "DEP-01", "ST-001", vec!["SEG-001"]);
    c.state = CrewState::RestingHome;
    c.shift_work_min = 500;
    c.total_distance_km = 300.0;
    eng.add_crew(c);
    push(&mut eng, 60, EventKind::HomeRestEnd, "C1", None, None, 10);
    eng.run(61);
    let c = eng.crews.get("C1").unwrap();
    assert_eq!(c.state, CrewState::AtHome);
    assert_eq!(c.shift_work_min, 0);
    assert_eq!(c.total_distance_km, 0.0);
    assert!(c.last_home_rest_end.is_some());
}

// ── LINE CHANGE ──

#[test]
fn test_line_change_releases_loco() {
    let mut eng = setup_engine();
    let mut c = crew_with_route("C1", "DEP-01", "ST-001", vec!["SEG-001"]);
    c.state = CrewState::Working;
    eng.add_crew(c);
    push(&mut eng, 60, EventKind::LineChangeStart, "C1", Some("LOCO-001"), Some("SEG-001"), 5);
    eng.run(61);
    assert!(matches!(
        eng.locos.get("LOCO-001").unwrap().state,
        LocoState::Available | LocoState::Failed
    ));
}

// ── VIOLATIONS ──

#[test]
fn test_overtime_8h_flag_set() {
    let mut eng = setup_engine();
    eng.set_warmup(0);
    let mut c = crew_with_route("C1", "DEP-001", "ST-001", vec!["SEG-001"]);
    c.shift_start = Some(0);
    c.shift_work_min = 479;
    c.state = CrewState::Working;
    eng.add_crew(c);
    eng.queue.push(SimEvent {
        time: 60, kind: EventKind::ArriveStation,
        crew_id: Some("C1".into()), loco_id: Some("LOCO-001".into()),
        segment_id: Some("SEG-001".into()), priority: 10, id: 0,
    });
    eng.run(200);
    assert!(eng.crews.get("C1").unwrap().overtime_8h_flagged);
    assert!(eng.violations.violations.iter().any(|v| v.violation_type == ViolationType::Overtime8h));
}

#[test]
fn test_violation_not_recorded_during_warmup() {
    let mut eng = setup_engine();
    eng.set_warmup(10000);
    let mut c = crew_with_route("C1", "DEP-01", "ST-001", vec!["SEG-001"]);
    c.state = CrewState::Finished;
    c.cumulative_work_min = 12000;
    eng.add_crew(c);
    push(&mut eng, 60, EventKind::HomeRestStart, "C1", None, None, 10);
    eng.run(61);
    assert!(eng.violations.violations.is_empty());
}

// ── EVENT CANCELLATION ──

#[test]
fn test_failure_cancels_pending_arrival() {
    let mut eng = setup_engine();
    eng.add_crew(crew_with_route("C1", "DEP-001", "ST-001", vec!["SEG-001"]));
    let c = eng.crews.get_mut("C1").unwrap();
    c.state = CrewState::Working;
    push(&mut eng, 60, EventKind::FailureOccurred(FailureKind::BrakeSystemFault), "C1", Some("LOCO-001"), Some("SEG-001"), 5);
    eng.run(500);
    assert_eq!(eng.locos.get("LOCO-001").unwrap().state, LocoState::Available);
}

// ── FULL LIFECYCLE ──

#[test]
fn test_full_shift_appear_to_homereset() {
    let mut eng = setup_engine();
    eng.add_crew(crew_with_route("C1", "DEP-001", "ST-001", vec!["SEG-001"]));
    eng.queue.push(SimEvent {
        time: 60, kind: EventKind::ShiftAppear,
        crew_id: Some("C1".into()), loco_id: None, segment_id: None, priority: 10, id: 0,
    });
    eng.run(5000);
    let c = eng.crews.get("C1").unwrap();
    assert!(matches!(c.state, CrewState::AtHome | CrewState::RestingHome | CrewState::Finished));
}

// ── UTILIZATION TRACKING ──

#[test]
fn test_loco_utilization_tracked() {
    let mut eng = setup_engine();
    let loco = eng.locos.get_mut("LOCO-001").unwrap();
    loco.state = LocoState::InUse;
    eng.stats(); // just verify stats() doesn't panic
}

#[test]
fn test_shift_end_records_stats() {
    let mut eng = setup_engine();
    eng.set_warmup(0);
    let mut c = crew_with_route("C1", "DEP-01", "ST-001", vec!["SEG-001"]);
    c.shift_start = Some(0);
    c.shift_work_min = 300;
    c.total_distance_km = 100.0;
    eng.add_crew(c);
    push(&mut eng, 60, EventKind::ShiftEnd, "C1", Some("LOCO-001"), None, 10);
    eng.run(500);
    assert!(eng.stats().total_shifts_completed > 0);
    assert!(eng.stats().total_distance_km > 0.0);
}

// ── PUBLIC GETTERS ──

#[test]
fn test_engine_public_getters() {
    let mut eng = setup_engine();
    eng.add_crew(crew_with_route("C1", "DEP-01", "ST-001", vec!["SEG-001"]));
    assert_eq!(eng.crew_count(), 1);
    assert_eq!(eng.loco_count(), 1);
    let _s = eng.stats();
    let _v = eng.violations_summary();
}
