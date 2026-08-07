use std::collections::HashMap;

use rzd_core::engine::Engine;
use rzd_core::entities::{Crew, LocoKind, LocoState, Locomotive};
use rzd_core::events::ViolationType;
use rzd_core::injection::{InjectedEvent, InjectionPlan};
use rzd_core::network::{Network, Segment, Station, StationKind, VoltageKind};
use rzd_core::violations::Severity;

fn minimal_network() -> Network {
    let mut stations = HashMap::new();
    stations.insert(
        "ST-A".into(),
        Station {
            id: "ST-A".into(),
            name: "Station A".into(),
            kind: StationKind::MainDepot,
            region: "R1".into(),
            coords: None,
            depot_id: Some("DEP-1".into()),
            voltage_kv: VoltageKind::DC3000,
            electrification: true,
        },
    );
    stations.insert(
        "ST-B".into(),
        Station {
            id: "ST-B".into(),
            name: "Station B".into(),
            kind: StationKind::Waypoint,
            region: "R1".into(),
            coords: None,
            depot_id: None,
            voltage_kv: VoltageKind::DC3000,
            electrification: true,
        },
    );

    let mut segments = HashMap::new();
    segments.insert(
        "SEG-AB".into(),
        Segment {
            id: "SEG-AB".into(),
            from: "ST-A".into(),
            to: "ST-B".into(),
            distance_km: 50.0,
            base_speed_kmh: 80.0,
            gradient_factor: 1.0,
            failure_base_rate: 0.0,
            voltage_sag_prob: 0.0,
            single_track: false,
        },
    );

    Network::new(stations, segments, HashMap::new(), HashMap::new())
}

#[test]
fn injection_plan_can_schedule_manual_violation() {
    let network = minimal_network();
    let mut engine = Engine::new(network, 42);
    engine.add_crew(Crew::new(
        "CREW-1".to_string(),
        "DEP-1".to_string(),
        "ST-A".to_string(),
    ));
    engine.add_loco(Locomotive {
        id: "LOCO-1".to_string(),
        series: "EL-2K".to_string(),
        loco_kind: LocoKind::Electric,
        sections: 2,
        depot_id: "DEP-1".to_string(),
        state: LocoState::Available,
        position: "ST-A".to_string(),
        mileage_km: 0.0,
        hours_since_to: 0.0,
        failures_total: 0,
        inventory_number: 500,
        manufacture_year: 2020,
    });

    let plan = InjectionPlan {
        schema: "rzd-des-injection-plan/v1".to_string(),
        events: vec![InjectedEvent::ManualViolation {
            time: 5,
            crew_id: "CREW-1".to_string(),
            loco_id: Some("LOCO-1".to_string()),
            segment_id: Some("SEG-AB".to_string()),
            violation_type: ViolationType::AcceptanceDelay,
            severity: Severity::Warning,
            description: "Injected acceptance delay".to_string(),
            norm_reference: "TEST-NORM".to_string(),
        }],
    };

    engine
        .apply_injection_plan(&plan)
        .expect("apply injection plan");
    engine.run(10);

    assert_eq!(engine.violations.violations.len(), 1);
    assert_eq!(
        engine.violations.violations[0].description,
        "Injected acceptance delay"
    );
}
