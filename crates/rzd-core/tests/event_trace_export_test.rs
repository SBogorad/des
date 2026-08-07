use rzd_core::events::{EventKind, SimEvent};
use rzd_core::export::{build_event_trace, Export};

fn event(id: u64, time: u64, kind: EventKind) -> SimEvent {
    SimEvent {
        id,
        time,
        kind,
        crew_id: Some(format!("CREW-{id}")),
        loco_id: None,
        segment_id: None,
        priority: 10,
    }
}

#[test]
fn direct_ndjson_matches_event_trace_wrapper_byte_for_byte() {
    let events = vec![
        event(1, 60, EventKind::ShiftAppear),
        event(2, 75, EventKind::MedCheckStart),
    ];
    let trace = build_event_trace(&events, 90);

    let direct = Export::events_to_ndjson(&events).expect("direct serialization should succeed");
    let wrapped =
        Export::event_trace_to_ndjson(&trace).expect("wrapped serialization should succeed");

    assert_eq!(direct, wrapped);
    assert_eq!(direct.lines().count(), events.len());
}
