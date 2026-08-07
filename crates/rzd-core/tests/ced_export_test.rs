use rzd_core::events::ViolationType;
use rzd_core::export::build_ced_export;
use rzd_core::tu3::Tu3Row;
use rzd_core::violations::{Severity, Violation};

fn make_test_tu3_rows() -> Vec<Tu3Row> {
    vec![
        Tu3Row {
            crew_id: "CREW-0001".into(),
            loco_id: "LOCO-0001".into(),
            work_duration_min: 540,
            total_distance_km: 191.0,
            failure_count: 1,
            voltage_sag_count: 2,
            wait_count: 1,
            wait_total_min: 45,
            violations_count: 1,
            from_station: "Ярославль".into(),
            to_station: "Вологда".into(),
            depot_name: "Ярославль".into(),
            route_segments: "SEG-01,SEG-02,SEG-03".into(),
            ..Tu3Row::empty()
        },
        Tu3Row {
            crew_id: "CREW-0002".into(),
            loco_id: "LOCO-0002".into(),
            work_duration_min: 420,
            total_distance_km: 85.0,
            failure_count: 0,
            voltage_sag_count: 0,
            wait_count: 2,
            wait_total_min: 60,
            violations_count: 0,
            from_station: "Вологда".into(),
            to_station: "Сыктывкар".into(),
            depot_name: "Вологда".into(),
            route_segments: "SEG-10,SEG-11,SEG-12".into(),
            ..Tu3Row::empty()
        },
    ]
}

fn make_test_violations() -> Vec<Violation> {
    vec![
        Violation {
            id: "V-001".into(),
            violation_type: ViolationType::Overtime8h,
            severity: Severity::Warning,
            crew_id: "CREW-0001".into(),
            loco_id: Some("LOCO-0001".into()),
            segment_id: Some("SEG-02".into()),
            detected_at: 600,
            description: "Смена > 480 мин (8ч)".into(),
            norm_reference: "Приказ Минтранса №339, п.4.1".into(),
        },
        Violation {
            id: "V-002".into(),
            violation_type: ViolationType::VoltageSagShift,
            severity: Severity::Warning,
            crew_id: "CREW-0001".into(),
            loco_id: Some("LOCO-0001".into()),
            segment_id: Some("SEG-02".into()),
            detected_at: 450,
            description: "Просадка напряжения 2300 В".into(),
            norm_reference: "Технические условия КС".into(),
        },
    ]
}

#[test]
fn test_build_ced_export() {
    let rows = make_test_tu3_rows();
    let violations = make_test_violations();

    let ced = build_ced_export(&rows, &violations);

    assert_eq!(ced.trips.len(), 2);
    assert_eq!(ced.violations.len(), 2);
}

#[test]
fn test_ced_trips_have_ids() {
    let rows = make_test_tu3_rows();
    let violations = make_test_violations();

    let ced = build_ced_export(&rows, &violations);

    for (i, trip) in ced.trips.iter().enumerate() {
        assert_eq!(trip.id, format!("TRIP-{:04}", i + 1));
        assert!(!trip.crew_id.is_empty());
    }
}

#[test]
fn test_ced_events_generated() {
    let rows = make_test_tu3_rows();
    let violations = make_test_violations();

    let ced = build_ced_export(&rows, &violations);

    let fail_events = ced
        .events
        .iter()
        .filter(|e| e.factor == "Отказ ТМЦ")
        .count();
    let volt_events = ced
        .events
        .iter()
        .filter(|e| e.factor == "Просадка КС")
        .count();
    let wait_events = ced
        .events
        .iter()
        .filter(|e| e.factor == "Ожидание поезда")
        .count();

    assert_eq!(fail_events, 1);
    assert_eq!(volt_events, 1);
    assert_eq!(wait_events, 2); // 1 per row with wait > 0
}

#[test]
fn test_ced_factors() {
    let rows = make_test_tu3_rows();
    let violations = make_test_violations();

    let ced = build_ced_export(&rows, &violations);

    assert!(!ced.factors.is_empty());
    let total_percent: f32 = ced.factors.iter().map(|f| f.percent).sum();
    assert!((99.0..101.0).contains(&total_percent));
}

#[test]
fn test_ced_serializes_to_json() {
    let rows = make_test_tu3_rows();
    let violations = make_test_violations();

    let ced = build_ced_export(&rows, &violations);
    let json = serde_json::to_string_pretty(&ced).unwrap();

    assert!(json.contains("CREW-0001"));
    assert!(json.contains("TRIP-0001"));
    assert!(json.contains("Отказ ТМЦ"));
}

#[test]
fn test_ced_empty_data() {
    let ced = build_ced_export(&[], &[]);

    assert!(ced.trips.is_empty());
    assert!(ced.events.is_empty());
    assert!(ced.violations.is_empty());
    assert!(ced.factors.is_empty());
}
