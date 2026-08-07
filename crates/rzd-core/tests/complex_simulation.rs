use rzd_core::engine::Engine;
use rzd_core::entities::{Crew, LocoKind, LocoState, Locomotive};
use rzd_core::events::{EventKind, SimEvent};
use rzd_core::network::{Network, Segment, Station, StationKind, VoltageKind};
use std::collections::HashMap;

#[test]
fn test_complex_synthetic_run() {
    let mut stations = HashMap::new();
    let mut segments = HashMap::new();

    // Generate 10 stations
    for i in 1..=10 {
        let id = format!("ST-{:03}", i);
        stations.insert(
            id.clone(),
            Station {
                id: id.clone(),
                name: format!("Station {}", i),
                kind: if i == 1 {
                    StationKind::MainDepot
                } else if i == 10 {
                    StationKind::Terminal
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
            let from = format!("ST-{:03}", i - 1);
            let to = format!("ST-{:03}", i);
            let seg_id = format!("SEG-{:03}", i - 1);
            segments.insert(
                seg_id.clone(),
                Segment {
                    id: seg_id,
                    from,
                    to,
                    distance_km: 50.0,
                    base_speed_kmh: 80.0,
                    gradient_factor: 1.0,
                    failure_base_rate: 0.001,
                    voltage_sag_prob: 0.01,
                    single_track: false,
                },
            );
        }
    }

    let network = Network::new(stations, segments, HashMap::new(), HashMap::new());

    let mut engine = Engine::new(network, 42);

    // Add 50 crews using Crew::new
    for i in 1..=50 {
        let crew_id = format!("CREW-{:03}", i);
        let mut crew = Crew::new(crew_id.clone(), "DEP-01".to_string(), "ST-001".to_string());

        // Assign route: ST-001 -> ST-010 via SEG-001..SEG-009
        crew.route_segments = (1..10).map(|j| format!("SEG-{:03}", j)).collect();
        crew.route_index = 0;

        engine.add_crew(crew);

        // Schedule appearance for each crew at different times
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

    // Add 30 locomotives
    for i in 1..=30 {
        let loco_id = format!("LOCO-{:03}", i);
        engine.add_loco(Locomotive {
            id: loco_id,
            series: "EL-2K".to_string(),
            loco_kind: LocoKind::Electric,
            sections: 2,
            depot_id: "DEP-01".to_string(),
            state: LocoState::Available,
            position: "ST-001".to_string(),
            mileage_km: 0.0,
            hours_since_to: 0.0,
            failures_total: 0,
            inventory_number: 500 + i as u32 * 10,
            manufacture_year: 2020,
        });
    }

    // Run for 24 hours (1440 min)
    engine.run(1440);

    println!(
        "Simulation finished. TU-3 Rows generated: {}",
        engine.tu3_rows.len()
    );
    assert!(!engine.tu3_rows.is_empty());
}
