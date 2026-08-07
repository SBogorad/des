use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use rzd_core::engine::Engine;
use rzd_core::entities::Crew;
use rzd_core::events::{EventKind, SimEvent};
use rzd_core::network::{Leg, Network, Road, Segment, Station, StationKind, VoltageKind};
use rzd_core::scenario::{generate_scenario, ScenarioConfig};
use std::collections::HashMap;

fn build_small_network() -> Network {
    let mut stations = HashMap::new();
    let mut segments = HashMap::new();
    let mut legs = HashMap::new();
    let mut roads = HashMap::new();

    for i in 0..20 {
        let id = format!("ST-{:02}", i);
        let kind = if i == 0 || i == 19 {
            StationKind::Turnaround
        } else if i % 5 == 0 {
            StationKind::Junction
        } else {
            StationKind::Waypoint
        };
        stations.insert(
            id.clone(),
            Station {
                id: id.clone(),
                name: format!("Станция {}", i),
                kind,
                region: "R1".into(),
                coords: None,
                depot_id: Some(format!("DEP-{:02}", i / 4)),
                voltage_kv: VoltageKind::DC3000,
                electrification: true,
            },
        );
    }

    for i in 0..19 {
        let id = format!("SEG-{:02}", i);
        segments.insert(
            id.clone(),
            Segment {
                id: id.clone(),
                from: format!("ST-{:02}", i),
                to: format!("ST-{:02}", i + 1),
                distance_km: 25.0,
                base_speed_kmh: 120.0,
                gradient_factor: 1.0,
                failure_base_rate: 0.001,
                voltage_sag_prob: 0.0005,
                single_track: false,
            },
        );
    }

    let segment_ids: Vec<String> = (0..19).map(|i| format!("SEG-{:02}", i)).collect();
    legs.insert(
        "LEG-01".into(),
        Leg {
            id: "LEG-01".into(),
            name: "Тестовое направление".into(),
            segments: segment_ids.clone(),
            from_depot: "DEP-00".into(),
            to_depot: "DEP-04".into(),
            total_km: 475.0,
            road_id: "RD-01".into(),
        },
    );

    let station_ids: Vec<String> = (0..20).map(|i| format!("ST-{:02}", i)).collect();
    roads.insert(
        "RD-01".into(),
        Road {
            id: "RD-01".into(),
            name: "Тестовая магистраль".into(),
            stations: station_ids,
            legs: vec!["LEG-01".into()],
            depots: vec!["DEP-00".into(), "DEP-04".into()],
            region_multiplier: 1.0,
        },
    );

    Network::new(stations, segments, legs, roads)
}

fn build_medium_network() -> Network {
    let mut stations = HashMap::new();
    let mut segments = HashMap::new();
    let mut legs = HashMap::new();
    let mut roads = HashMap::new();

    let n_stations = 50;
    let n_depots = 10;

    for i in 0..n_stations {
        let id = format!("ST-{:03}", i);
        let kind = if i == 0 || i == n_stations - 1 {
            StationKind::Turnaround
        } else if i % 10 == 0 {
            StationKind::Junction
        } else {
            StationKind::Waypoint
        };
        stations.insert(
            id.clone(),
            Station {
                id: id.clone(),
                name: format!("Станция {}", i),
                kind,
                region: format!("R{}", i / 25 + 1),
                coords: None,
                depot_id: Some(format!("DEP-{:02}", i % n_depots)),
                voltage_kv: VoltageKind::DC3000,
                electrification: true,
            },
        );
    }

    for i in 0..(n_stations - 1) {
        let id = format!("SEG-{:03}", i);
        segments.insert(
            id.clone(),
            Segment {
                id: id.clone(),
                from: format!("ST-{:03}", i),
                to: format!("ST-{:03}", i + 1),
                distance_km: 15.0,
                base_speed_kmh: 120.0,
                gradient_factor: 1.0,
                failure_base_rate: 0.001,
                voltage_sag_prob: 0.0005,
                single_track: false,
            },
        );
    }

    let segs: Vec<String> = (0..(n_stations - 1))
        .map(|i| format!("SEG-{:03}", i))
        .collect();
    legs.insert(
        "LEG-01".into(),
        Leg {
            id: "LEG-01".into(),
            name: "Основное направление".into(),
            segments: segs,
            from_depot: "DEP-00".into(),
            to_depot: "DEP-09".into(),
            total_km: 735.0,
            road_id: "RD-01".into(),
        },
    );

    let station_ids: Vec<String> = (0..n_stations).map(|i| format!("ST-{:03}", i)).collect();
    roads.insert(
        "RD-01".into(),
        Road {
            id: "RD-01".into(),
            name: "Магистраль".into(),
            stations: station_ids,
            legs: vec!["LEG-01".into()],
            depots: (0..n_depots).map(|i| format!("DEP-{:02}", i)).collect(),
            region_multiplier: 1.0,
        },
    );

    Network::new(stations, segments, legs, roads)
}

fn build_large_network() -> Network {
    let mut stations = HashMap::new();
    let mut segments = HashMap::new();
    let mut legs = HashMap::new();
    let mut roads = HashMap::new();

    let n_stations = 120;
    let n_depots = 20;

    for i in 0..n_stations {
        let id = format!("ST-{:03}", i);
        let kind = if i == 0 || i == n_stations - 1 {
            StationKind::Turnaround
        } else if i % 12 == 0 {
            StationKind::Junction
        } else {
            StationKind::Waypoint
        };
        stations.insert(
            id.clone(),
            Station {
                id: id.clone(),
                name: format!("Станция {}", i),
                kind,
                region: format!("R{}", i / 30 + 1),
                coords: None,
                depot_id: Some(format!("DEP-{:02}", i % n_depots)),
                voltage_kv: if i % 3 == 0 {
                    VoltageKind::AC25000
                } else {
                    VoltageKind::DC3000
                },
                electrification: true,
            },
        );
    }

    for i in 0..(n_stations - 1) {
        let id = format!("SEG-{:03}", i);
        segments.insert(
            id.clone(),
            Segment {
                id: id.clone(),
                from: format!("ST-{:03}", i),
                to: format!("ST-{:03}", i + 1),
                distance_km: 12.0,
                base_speed_kmh: 140.0,
                gradient_factor: 1.0,
                failure_base_rate: 0.001,
                voltage_sag_prob: 0.0005,
                single_track: false,
            },
        );
    }

    let segs: Vec<String> = (0..(n_stations - 1))
        .map(|i| format!("SEG-{:03}", i))
        .collect();
    legs.insert(
        "LEG-01".into(),
        Leg {
            id: "LEG-01".into(),
            name: "Длинное направление".into(),
            segments: segs,
            from_depot: "DEP-00".into(),
            to_depot: "DEP-19".into(),
            total_km: 1428.0,
            road_id: "RD-01".into(),
        },
    );

    let station_ids: Vec<String> = (0..n_stations).map(|i| format!("ST-{:03}", i)).collect();
    roads.insert(
        "RD-01".into(),
        Road {
            id: "RD-01".into(),
            name: "Магистраль".into(),
            stations: station_ids,
            legs: vec!["LEG-01".into()],
            depots: (0..n_depots).map(|i| format!("DEP-{:02}", i)).collect(),
            region_multiplier: 1.0,
        },
    );

    Network::new(stations, segments, legs, roads)
}

fn build_engine(network: &Network, crews: usize, locos: usize, seed: u32) -> Engine {
    let config = ScenarioConfig {
        crew_count: crews,
        loco_count: locos,
        seed,
        duration_days: 7,
        ..Default::default()
    };
    let scenario = generate_scenario(network, config);
    let mut eng = Engine::new(network.clone(), seed);
    eng.set_warmup(60);
    for c in &scenario.crews {
        eng.add_crew(c.clone());
    }
    for l in &scenario.locos {
        eng.add_loco(l.clone());
    }
    eng.schedule_all_shifts(0, 30);
    eng
}

fn bench_event_queue(c: &mut Criterion) {
    let mut group = c.benchmark_group("event_queue");

    for size in [100, 1_000, 10_000] {
        group.bench_with_input(BenchmarkId::new("push", size), &size, |b, &size| {
            b.iter(|| {
                let mut eng = Engine::new(build_small_network(), 42);
                for i in 0..size {
                    eng.queue.push(SimEvent {
                        id: 0,
                        time: i,
                        kind: EventKind::ShiftAppear,
                        crew_id: Some(format!("C{}", i)),
                        loco_id: None,
                        segment_id: None,
                        priority: 10,
                    });
                }
                eng
            });
        });

        group.bench_with_input(BenchmarkId::new("pop", size), &size, |b, &size| {
            b.iter(|| {
                let mut eng = Engine::new(build_small_network(), 42);
                for i in 0..size {
                    eng.queue.push(SimEvent {
                        id: 0,
                        time: i,
                        kind: EventKind::ShiftAppear,
                        crew_id: Some(format!("C{}", i)),
                        loco_id: None,
                        segment_id: None,
                        priority: 10,
                    });
                }
                while eng.queue.pop().is_some() {}
            });
        });
    }
    group.finish();
}

fn bench_full_simulation(c: &mut Criterion) {
    let mut group = c.benchmark_group("full_simulation");

    let small = build_small_network();
    let medium = build_medium_network();
    let large = build_large_network();

    let configs: &[(&str, usize, usize, &Network)] = &[
        ("small_10x5", 10, 5, &small),
        ("medium_50x20", 50, 20, &medium),
        ("large_200x80", 200, 80, &large),
        ("xlarge_500x200", 500, 200, &large),
    ];

    for &(name, crews, locos, network) in configs {
        group.bench_function(name, |b| {
            b.iter(|| {
                let mut eng = build_engine(network, crews, locos, 42);
                eng.run(60 * 24 * 7);
                eng
            });
        });
    }
    group.finish();
}

fn bench_scenarios_generation(c: &mut Criterion) {
    let mut group = c.benchmark_group("scenario_generation");
    let network = build_medium_network();

    for (crews, locos) in [(50, 20), (100, 40), (200, 80)] {
        let label = format!("{}x{}", crews, locos);
        group.bench_function(&label, |b| {
            b.iter(|| {
                let config = ScenarioConfig {
                    crew_count: crews,
                    loco_count: locos,
                    seed: 42,
                    duration_days: 30,
                    ..Default::default()
                };
                generate_scenario(&network, config)
            });
        });
    }
    group.finish();
}

fn bench_engine_construction(c: &mut Criterion) {
    let mut group = c.benchmark_group("engine_construction");

    let small = build_small_network();
    let medium = build_medium_network();
    let large = build_large_network();

    for (name, network) in [
        ("small_20_stations", &small),
        ("medium_50_stations", &medium),
        ("large_120_stations", &large),
    ] {
        group.bench_function(name, |b| {
            b.iter(|| {
                let mut eng = Engine::new(network.clone(), 42);
                for i in 0..100 {
                    eng.add_crew(Crew::new(
                        format!("C{:04}", i),
                        "DEP-00".into(),
                        "ST-00".into(),
                    ));
                }
                eng
            });
        });
    }
    group.finish();
}

criterion_group!(
    benches,
    bench_event_queue,
    bench_full_simulation,
    bench_scenarios_generation,
    bench_engine_construction,
);
criterion_main!(benches);
