use proptest::prelude::*;
use rzd_core::engine::Engine;
use rzd_core::events::{EventKind, EventQueue, SimEvent};
use rzd_core::network::{Network, Segment, Station, StationKind, VoltageKind};
use rzd_core::prng::Mulberry32;
use std::collections::HashMap;

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
    stations.insert(
        "ST-C".into(),
        Station {
            id: "ST-C".into(),
            name: "Station C".into(),
            kind: StationKind::Turnaround,
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
            failure_base_rate: 0.001,
            voltage_sag_prob: 0.0,
            single_track: false,
        },
    );
    segments.insert(
        "SEG-BC".into(),
        Segment {
            id: "SEG-BC".into(),
            from: "ST-B".into(),
            to: "ST-C".into(),
            distance_km: 30.0,
            base_speed_kmh: 90.0,
            gradient_factor: 1.0,
            failure_base_rate: 0.001,
            voltage_sag_prob: 0.0,
            single_track: false,
        },
    );

    Network::new(stations, segments, HashMap::new(), HashMap::new())
}

// ── EventQueue property: push/pop preserves ordering ──

proptest! {
    #[test]
    fn event_queue_ordered_by_time(times in prop::collection::vec(0u64..10000, 1..50)) {
        let mut q = EventQueue::new();
        for (i, &t) in times.iter().enumerate() {
            q.push(SimEvent {
                id: 0, time: t, kind: EventKind::ShiftAppear,
                crew_id: Some(format!("C{}", i)), loco_id: None, segment_id: None,
                priority: 10,
            });
        }
        let mut prev_time = 0u64;
        while let Some(ev) = q.pop() {
            assert!(ev.time >= prev_time, "events must be in non-decreasing time order");
            prev_time = ev.time;
        }
    }

    #[test]
    fn event_queue_priority_within_same_time(
        priorities in prop::collection::vec(1u8..20, 1..30),
    ) {
        let mut q = EventQueue::new();
        for (i, &p) in priorities.iter().enumerate() {
            q.push(SimEvent {
                id: 0, time: 100, kind: EventKind::ShiftAppear,
                crew_id: Some(format!("C{}", i)), loco_id: None, segment_id: None,
                priority: p,
            });
        }
        let mut prev_priority = u8::MAX;
        let mut prev_id = 0u64;
        while let Some(ev) = q.pop() {
            assert!(
                ev.priority <= prev_priority,
                "within same time, higher priority must be popped first"
            );
            if ev.priority == prev_priority {
                assert!(
                    ev.id > prev_id,
                    "within same priority bucket, events must preserve FIFO by id"
                );
            }
            prev_priority = ev.priority;
            prev_id = ev.id;
        }
    }

    #[test]
    fn event_queue_cancel_removes_event(ids in prop::collection::vec(1u64..1000, 2..20)) {
        let mut q = EventQueue::new();
        let pushed: Vec<u64> = ids.iter().map(|&id| {
            q.push(SimEvent {
                id: 0, time: id, kind: EventKind::ShiftAppear,
                crew_id: Some(format!("C{}", id)), loco_id: None, segment_id: None,
                priority: 10,
            })
        }).collect();

        // Cancel half the events
        let cancel_count = pushed.len() / 2;
        for &eid in pushed.iter().take(cancel_count) {
            q.cancel(eid);
        }

        let remaining = pushed.len() - cancel_count;
        let mut popped = 0;
        while q.pop().is_some() {
            popped += 1;
        }
        assert_eq!(popped, remaining, "cancelled events should not be popped");
    }

    // ── Mulberry32 determinism ──

    #[test]
    fn prng_deterministic_across_seeds(seeds in prop::collection::vec(0u32..10000, 1..20)) {
        for &seed in &seeds {
            let mut a = Mulberry32::new(seed);
            let mut b = Mulberry32::new(seed);
            for _ in 0..100 {
                assert_eq!(a.next_f32(), b.next_f32(), "PRNG must be deterministic for seed {}", seed);
            }
        }
    }

    // ── Network adjacency property: shortest path exists for valid routes ──

    #[test]
    fn network_shortest_path_returns_valid_path(
        stations in prop::collection::vec("[A-Z]{2}", 2..6),
        segments_data in prop::collection::vec(
            (0usize..5, 0usize..5, 10.0f64..500.0, 40.0f64..120.0),
            1..10,
        ),
    ) {
        // Build a random connected graph from station pairs
        let unique_stations: Vec<String> = {
            let mut s: Vec<String> = stations.iter().take(5).cloned().collect();
            s.sort();
            s.dedup();
            s
        };
        if unique_stations.len() < 2 { return Ok(()); }

        let mut network_stations = HashMap::new();
        for (i, name) in unique_stations.iter().enumerate() {
            network_stations.insert(name.clone(), Station {
                id: name.clone(), name: name.clone(),
                kind: if i == 0 { StationKind::MainDepot } else { StationKind::Waypoint },
                region: "R1".into(), coords: None, depot_id: None,
                voltage_kv: VoltageKind::DC3000, electrification: true,
            });
        }

        let mut network_segments = HashMap::new();
        for (i, &(from_idx, to_idx, dist, speed)) in segments_data.iter().enumerate() {
            let fi = from_idx % unique_stations.len();
            let ti = to_idx % unique_stations.len();
            if fi == ti { continue; }
            let seg_id = format!("SEG-{}", i);
            network_segments.insert(seg_id.clone(), Segment {
                id: seg_id, from: unique_stations[fi].clone(), to: unique_stations[ti].clone(),
                distance_km: dist as f32, base_speed_kmh: speed as f32, gradient_factor: 1.0,
                failure_base_rate: 0.0, voltage_sag_prob: 0.0, single_track: false,
            });
        }

        if network_segments.is_empty() { return Ok(()); }

        let net = Network::new(network_stations, network_segments, HashMap::new(), HashMap::new());

        // For every pair of distinct stations, shortest path either finds a route or returns None
        let station_ids: Vec<String> = net.stations.keys().cloned().collect();
        for from in &station_ids {
            for to in &station_ids {
                if from == to { continue; }
                let path = net.shortest_path(from, to);
                if let Some(segments) = path {
                    assert!(!segments.is_empty(), "path must have at least one segment");
                    for seg_id in &segments {
                        assert!(net.segments.contains_key(seg_id), "segment {} in path must exist", seg_id);
                    }
                }
            }
        }
    }

    // ── Engine creation ──

    #[test]
    fn engine_new_always_consistent(seed in 0u32..10000) {
        let net = minimal_network();
        let eng = Engine::new(net, seed);
        assert_eq!(eng.crew_count(), 0, "new engine has no crews");
        assert_eq!(eng.loco_count(), 0, "new engine has no locos");
        assert_eq!(eng.current_time, 0, "new engine starts at time 0");
        assert_eq!(eng.event_errors, 0, "new engine has no errors");
    }
}
