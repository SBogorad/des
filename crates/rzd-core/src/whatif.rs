use crate::engine::Engine;
use crate::network::Network;
use crate::norms::RegionNorms;
use crate::scenario::{generate_scenario, ScenarioConfig};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WhatIfParams {
    #[serde(default)]
    pub crew_count_delta: i32,
    #[serde(default)]
    pub loco_count_delta: i32,
    #[serde(default)]
    pub failure_rate_multiplier: Option<f32>,
    #[serde(default)]
    pub voltage_sag_multiplier: Option<f32>,
    #[serde(default)]
    pub overtime_8h_override: Option<u32>,
    #[serde(default)]
    pub overtime_12h_override: Option<u32>,
    #[serde(default)]
    pub min_home_rest_override: Option<u32>,
    #[serde(default)]
    pub region_overrides: Option<Vec<RegionNorms>>,
    #[serde(default)]
    pub seed_override: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WhatIfResult {
    pub baseline: WhatIfMetrics,
    pub modified: WhatIfMetrics,
    pub changes: WhatIfChanges,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WhatIfMetrics {
    pub crews: usize,
    pub locos: usize,
    pub total_violations: usize,
    pub overtime_8h_count: usize,
    pub overtime_12h_count: usize,
    pub short_rest_count: usize,
    pub tu3_rows: usize,
    pub total_shifts: u32,
    pub avg_shift_min: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WhatIfChanges {
    pub violation_delta: i32,
    pub overtime_8h_delta: i32,
    pub overtime_12h_delta: i32,
    pub short_rest_delta: i32,
    pub shift_change_pct: f32,
}

pub fn run_what_if(
    network: &Network,
    base_config: ScenarioConfig,
    params: WhatIfParams,
    duration_min: u64,
) -> WhatIfResult {
    let baseline = run_simulation(network, &base_config, duration_min);

    let mut modified_config = base_config.clone();
    let new_crew = (modified_config.crew_count as i32 + params.crew_count_delta).max(1) as usize;
    let new_loco = (modified_config.loco_count as i32 + params.loco_count_delta).max(1) as usize;
    modified_config.crew_count = new_crew;
    modified_config.loco_count = new_loco;
    if let Some(seed) = params.seed_override {
        modified_config.seed = seed;
    }

    let modified = run_simulation_with_params(network, &modified_config, &params, duration_min);

    let changes = WhatIfChanges {
        violation_delta: modified.total_violations as i32 - baseline.total_violations as i32,
        overtime_8h_delta: modified.overtime_8h_count as i32 - baseline.overtime_8h_count as i32,
        overtime_12h_delta: modified.overtime_12h_count as i32 - baseline.overtime_12h_count as i32,
        short_rest_delta: modified.short_rest_count as i32 - baseline.short_rest_count as i32,
        shift_change_pct: if baseline.total_shifts > 0 {
            ((modified.total_shifts as f32 - baseline.total_shifts as f32)
                / baseline.total_shifts as f32)
                * 100.0
        } else {
            0.0
        },
    };

    WhatIfResult {
        baseline,
        modified,
        changes,
    }
}

fn run_simulation(network: &Network, config: &ScenarioConfig, duration_min: u64) -> WhatIfMetrics {
    let params = WhatIfParams::default();
    run_simulation_with_params(network, config, &params, duration_min)
}

fn run_simulation_with_params(
    network: &Network,
    config: &ScenarioConfig,
    params: &WhatIfParams,
    duration_min: u64,
) -> WhatIfMetrics {
    let scenario = generate_scenario(network, config.clone());
    let mut eng = Engine::new(network.clone(), config.seed);
    eng.set_warmup(60);

    for c in &scenario.crews {
        eng.add_crew(c.clone());
    }
    for l in &scenario.locos {
        eng.add_loco(l.clone());
    }

    if let Some(mult) = params.failure_rate_multiplier {
        for seg in eng.network.segments.values_mut() {
            seg.failure_base_rate *= mult;
        }
    }
    if let Some(mult) = params.voltage_sag_multiplier {
        for seg in eng.network.segments.values_mut() {
            seg.voltage_sag_prob *= mult;
        }
    }
    if let Some(v) = params.overtime_8h_override {
        eng.norms.max_shift_8h_min = v;
    }
    if let Some(v) = params.overtime_12h_override {
        eng.norms.max_shift_12h_min = v;
    }
    if let Some(v) = params.min_home_rest_override {
        eng.norms.min_home_rest_min = v;
    }
    if let Some(ref overrides) = params.region_overrides {
        for override_norms in overrides {
            eng.network
                .region_norms
                .add(override_norms.region_id.clone(), override_norms.clone());
        }
    }

    eng.schedule_all_shifts(0, 30);
    eng.run(duration_min);

    let vsummary = eng.violations.summary();
    let violations = vsummary.total;

    WhatIfMetrics {
        crews: scenario.crews.len(),
        locos: scenario.locos.len(),
        total_violations: violations,
        overtime_8h_count: *vsummary
            .by_type
            .get(&crate::events::ViolationType::Overtime8h)
            .unwrap_or(&0) as usize,
        overtime_12h_count: *vsummary
            .by_type
            .get(&crate::events::ViolationType::Overtime12h)
            .unwrap_or(&0) as usize,
        short_rest_count: *vsummary
            .by_type
            .get(&crate::events::ViolationType::ShortRest)
            .unwrap_or(&0) as usize,
        tu3_rows: eng.tu3_rows.len(),
        total_shifts: eng.stats.total_shifts_completed as u32,
        avg_shift_min: 0.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::network::{Leg, Road, Segment, Station, StationKind, VoltageKind};
    use std::collections::HashMap;

    fn test_network() -> Network {
        let mut stations = HashMap::new();
        let mut segments = HashMap::new();
        let mut legs = HashMap::new();
        let mut roads = HashMap::new();

        for i in 1..=5 {
            let id = format!("ST-{:03}", i);
            stations.insert(
                id.clone(),
                Station {
                    id: id.clone(),
                    name: format!("Station {}", i),
                    kind: if i == 1 {
                        StationKind::MainDepot
                    } else if i == 5 {
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
                segments.insert(
                    format!("SEG-{:03}", i - 1),
                    Segment {
                        id: format!("SEG-{:03}", i - 1),
                        from: format!("ST-{:03}", i - 1),
                        to: id,
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

        legs.insert(
            "LEG-01".to_string(),
            Leg {
                id: "LEG-01".into(),
                name: "Test".into(),
                from_depot: "DEP-01".into(),
                to_depot: "DEP-01".into(),
                segments: vec![
                    "SEG-001".into(),
                    "SEG-002".into(),
                    "SEG-003".into(),
                    "SEG-004".into(),
                ],
                total_km: 200.0,
                road_id: "RD-01".into(),
            },
        );

        roads.insert(
            "RD-01".to_string(),
            Road {
                id: "RD-01".into(),
                name: "Test Road".into(),
                stations: (1..=5).map(|i| format!("ST-{:03}", i)).collect(),
                depots: vec!["DEP-01".into()],
                legs: vec!["LEG-01".into()],
                region_multiplier: 1.0,
            },
        );

        Network::new(stations, segments, legs, roads)
    }

    #[test]
    fn test_what_if_basic() {
        let net = test_network();
        let config = ScenarioConfig {
            crew_count: 5,
            loco_count: 3,
            seed: 42,
            duration_days: 1,
            ..Default::default()
        };
        let params = WhatIfParams::default();

        let result = run_what_if(&net, config, params, 60 * 24);
        assert_eq!(result.baseline.crews, 5);
        assert_eq!(result.modified.crews, 5);
        assert_eq!(result.changes.violation_delta, 0);
    }

    #[test]
    fn test_what_if_more_crews() {
        let net = test_network();
        let config = ScenarioConfig {
            crew_count: 5,
            loco_count: 3,
            seed: 42,
            duration_days: 1,
            ..Default::default()
        };
        let params = WhatIfParams {
            crew_count_delta: 5,
            ..Default::default()
        };

        let result = run_what_if(&net, config, params, 60 * 24);
        assert_eq!(result.baseline.crews, 5);
        assert_eq!(result.modified.crews, 10);
    }

    #[test]
    fn test_what_if_custom_norms() {
        let net = test_network();
        let config = ScenarioConfig {
            crew_count: 5,
            loco_count: 3,
            seed: 42,
            duration_days: 1,
            ..Default::default()
        };
        let params = WhatIfParams {
            overtime_8h_override: Some(420),
            ..Default::default()
        };

        let result = run_what_if(&net, config, params, 60 * 24);
        assert_eq!(result.baseline.crews, 5);
    }
}
