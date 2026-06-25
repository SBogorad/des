use serde::{Deserialize, Serialize};
use crate::network::Network;
use crate::entities::{Crew, Locomotive, LocoKind, LocoState};
use crate::prng::Mulberry32;
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScenarioConfig {
    pub crew_count: usize,
    pub loco_count: usize,
    pub seed: u32,
    pub duration_days: u32,
    #[serde(default = "default_shift_spread_min")]
    pub shift_spread_min: u32,
    #[serde(default)]
    pub night_shift_ratio: f32,
}

fn default_shift_spread_min() -> u32 { 30 }

impl Default for ScenarioConfig {
    fn default() -> Self {
        Self {
            crew_count: 50,
            loco_count: 20,
            seed: 42,
            duration_days: 30,
            shift_spread_min: 30,
            night_shift_ratio: 0.3,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocoSeriesDef {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub sections: u8,
    pub depot_ids: Vec<String>,
    pub max_shift_hours: f32,
    pub acceleration_kmh_per_s: f32,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum CrewExperience {
    Junior,
    Standard,
    Senior,
    Master,
}

impl CrewExperience {
    pub fn shift_length_range(self) -> (u32, u32) {
        match self {
            Self::Junior => (360, 420),
            Self::Standard => (420, 480),
            Self::Senior => (480, 540),
            Self::Master => (480, 600),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Scenario {
    pub crews: Vec<Crew>,
    pub locos: Vec<Locomotive>,
    pub config: ScenarioConfig,
    pub metadata: ScenarioMetadata,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScenarioMetadata {
    pub total_crews: usize,
    pub total_locos: usize,
    pub depots_used: Vec<String>,
    pub regions_covered: Vec<String>,
    pub legs_assigned: usize,
}

fn experience_distribution(prng: &mut Mulberry32, total: usize) -> Vec<CrewExperience> {
    let junior_pct = 0.15;
    let senior_pct = 0.20;
    let master_pct = 0.05;

    let n_junior = (total as f32 * junior_pct) as usize;
    let n_master = (total as f32 * master_pct) as usize;
    let n_senior = (total as f32 * senior_pct) as usize;
    let n_standard = total.saturating_sub(n_junior + n_senior + n_master);

    let mut exps: Vec<CrewExperience> = Vec::with_capacity(total);
    exps.extend(std::iter::repeat_n(CrewExperience::Junior, n_junior));
    exps.extend(std::iter::repeat_n(CrewExperience::Standard, n_standard));
    exps.extend(std::iter::repeat_n(CrewExperience::Senior, n_senior));
    exps.extend(std::iter::repeat_n(CrewExperience::Master, n_master));

    while exps.len() < total {
        exps.push(CrewExperience::Standard);
    }

    let mut rng = Mulberry32::new(prng.range(0.0, u32::MAX as f32) as u32);
    for i in (1..exps.len()).rev() {
        let j = rng.range(0.0, (i + 1) as f32) as usize;
        exps.swap(i, j.min(i));
    }
    exps
}

pub fn generate_scenario(network: &Network, config: ScenarioConfig) -> Scenario {
    let mut prng = Mulberry32::new(config.seed);

    let mut depot_ids: Vec<String> = network.roads.values()
        .flat_map(|r| r.depots.iter().cloned())
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .collect();
    depot_ids.sort();

    if depot_ids.is_empty() {
        return Scenario {
            crews: Vec::new(),
            locos: Vec::new(),
            config,
            metadata: ScenarioMetadata {
                total_crews: 0,
                total_locos: 0,
                depots_used: Vec::new(),
                regions_covered: Vec::new(),
                legs_assigned: network.legs.len(),
            },
        };
    }

    let mut depot_station_map: HashMap<String, Vec<String>> = HashMap::new();
    for station in network.stations.values() {
        if let Some(ref depot_id) = station.depot_id {
            depot_station_map
                .entry(depot_id.clone())
                .or_default()
                .push(station.id.clone());
        }
    }

    let depot_legs: HashMap<String, Vec<String>> = {
        let mut map: HashMap<String, Vec<String>> = HashMap::new();
        for leg in network.legs.values() {
            map.entry(leg.from_depot.clone())
                .or_default()
                .push(leg.id.clone());
        }
        map
    };

    let mut regions_covered: Vec<String> = network.stations.values()
        .map(|s| s.region.clone())
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .collect();
    regions_covered.sort();

    // Generate crews
    let crews_per_depot = (config.crew_count as f32 / depot_ids.len() as f32).ceil() as usize;
    let mut crews = Vec::new();
    let mut crew_idx = 0;
    let experiences = experience_distribution(&mut prng, config.crew_count);
    let is_night = |shift_idx: usize, ratio: f32| {
        let threshold = (1.0 - ratio) * 100.0;
        ((shift_idx as f32 * 37.0) % 100.0) > threshold
    };

    for depot_id in &depot_ids {
        let depot_stations = depot_station_map.get(depot_id)
            .cloned()
            .unwrap_or_default();
        let start_station = depot_stations.first()
            .cloned()
            .unwrap_or_else(|| network.stations.keys().min().cloned().unwrap_or_default());

        let available_legs = depot_legs.get(depot_id)
            .cloned()
            .unwrap_or_default();

        let n = crews_per_depot.min(config.crew_count - crews.len());
        for i in 0..n {
            if crew_idx >= config.crew_count { break; }
            let experience = experiences.get(crew_idx).copied().unwrap_or(CrewExperience::Standard);
            let (min_shift, max_shift) = experience.shift_length_range();

            crew_idx += 1;
            let crew_id = format!("CREW-{:04}", crew_idx);

            let mut crew = Crew::new(
                crew_id,
                depot_id.clone(),
                start_station.clone(),
            );

            if !available_legs.is_empty() {
                let leg_idx = prng.range(0.0, available_legs.len() as f32) as usize;
                let leg_id = &available_legs[leg_idx.min(available_legs.len() - 1)];
                if let Some(leg) = network.legs.get(leg_id) {
                    crew.current_leg = Some(leg_id.clone());
                    crew.route_segments = leg.segments.clone();
                }
            }

            let shift_len = prng.range(min_shift as f32, max_shift as f32) as u32;
            crew.shift_work_min = shift_len;

            let night = is_night(i, config.night_shift_ratio);
            if night {
                crew.night_shifts_in_row = 1;
            }

            crews.push(crew);
        }
    }

    // Generate locomotives
    let series_defs = default_loco_series();
    let locos_per_depot = (config.loco_count as f32 / depot_ids.len() as f32).ceil() as usize;
    let mut locos = Vec::new();
    let mut loco_idx = 0;

    for depot_id in &depot_ids {
        let depot_stations = depot_station_map.get(depot_id)
            .cloned()
            .unwrap_or_default();
        let start_station = depot_stations.first()
            .cloned()
            .unwrap_or_else(|| network.stations.keys().min().cloned().unwrap_or_default());

        let matching_series: Vec<&LocoSeriesDef> = series_defs.iter()
            .filter(|s| s.depot_ids.contains(depot_id))
            .collect();

        let n = locos_per_depot.min(config.loco_count - locos.len());
        for _ in 0..n {
            loco_idx += 1;
            let loco_id = format!("LOCO-{:04}", loco_idx);

            let series = if matching_series.is_empty() {
                &series_defs[0]
            } else {
                let idx = prng.range(0.0, matching_series.len() as f32) as usize;
                matching_series[idx.min(matching_series.len() - 1)]
            };

            let loco_kind = match series.kind.as_str() {
                "Electric" => LocoKind::Electric,
                "Diesel" => LocoKind::Diesel,
                "Dual" => LocoKind::Dual,
                _ => LocoKind::Electric,
            };

            let mileage = prng.range(10000.0, 200000.0);
            let hours_since = prng.range(0.0, 200.0);

            locos.push(Locomotive {
                id: loco_id,
                series: series.id.clone(),
                loco_kind,
                sections: series.sections,
                depot_id: depot_id.clone(),
                state: LocoState::Available,
                position: start_station.clone(),
                mileage_km: mileage,
                hours_since_to: hours_since,
                failures_total: 0,
            });
        }
    }

    Scenario {
        crews,
        locos,
        config,
        metadata: ScenarioMetadata {
            total_crews: crew_idx,
            total_locos: loco_idx,
            depots_used: depot_ids,
            regions_covered,
            legs_assigned: network.legs.len(),
        },
    }
}

fn default_loco_series() -> Vec<LocoSeriesDef> {
    vec![
        LocoSeriesDef {
            id: "EL-2K".into(),
            name: "Электровоз 2-секционный".into(),
            kind: "Electric".into(),
            sections: 2,
            depot_ids: vec!["DEP-01".into(), "DEP-03".into(), "DEP-04".into(), "DEP-10".into()],
            max_shift_hours: 12.0,
            acceleration_kmh_per_s: 0.6,
        },
        LocoSeriesDef {
            id: "EL-4K".into(),
            name: "Электровоз 4-секционный".into(),
            kind: "Electric".into(),
            sections: 4,
            depot_ids: vec!["DEP-01".into(), "DEP-07".into()],
            max_shift_hours: 12.0,
            acceleration_kmh_per_s: 0.5,
        },
        LocoSeriesDef {
            id: "EP-2K".into(),
            name: "Пассажирский электровоз".into(),
            kind: "Electric".into(),
            sections: 2,
            depot_ids: vec!["DEP-01".into(), "DEP-04".into()],
            max_shift_hours: 10.0,
            acceleration_kmh_per_s: 0.7,
        },
        LocoSeriesDef {
            id: "DL-7H".into(),
            name: "Тепловоз 7-осный".into(),
            kind: "Diesel".into(),
            sections: 1,
            depot_ids: vec!["DEP-02".into(), "DEP-05".into(), "DEP-06".into(), "DEP-08".into()],
            max_shift_hours: 10.0,
            acceleration_kmh_per_s: 0.4,
        },
        LocoSeriesDef {
            id: "DL-9H".into(),
            name: "Тепловоз 9-осный".into(),
            kind: "Diesel".into(),
            sections: 1,
            depot_ids: vec!["DEP-05".into(), "DEP-06".into(), "DEP-08".into()],
            max_shift_hours: 10.0,
            acceleration_kmh_per_s: 0.35,
        },
        LocoSeriesDef {
            id: "TEP-70".into(),
            name: "Тепловоз пассажирский".into(),
            kind: "Diesel".into(),
            sections: 1,
            depot_ids: vec!["DEP-02".into(), "DEP-04".into()],
            max_shift_hours: 10.0,
            acceleration_kmh_per_s: 0.45,
        },
        LocoSeriesDef {
            id: "2TE25A".into(),
            name: "Двухсекционный тепловоз".into(),
            kind: "Diesel".into(),
            sections: 2,
            depot_ids: vec!["DEP-05".into(), "DEP-06".into()],
            max_shift_hours: 12.0,
            acceleration_kmh_per_s: 0.3,
        },
        LocoSeriesDef {
            id: "DUAL-2K".into(),
            name: "Двухmodes электровоз".into(),
            kind: "Dual".into(),
            sections: 2,
            depot_ids: vec!["DEP-03".into()],
            max_shift_hours: 11.0,
            acceleration_kmh_per_s: 0.5,
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_experience_distribution() {
        let mut prng = Mulberry32::new(42);
        let exps = experience_distribution(&mut prng, 100);
        assert_eq!(exps.len(), 100);
        let juniors = exps.iter().filter(|e| **e == CrewExperience::Junior).count();
        let masters = exps.iter().filter(|e| **e == CrewExperience::Master).count();
        assert!(juniors > 10, "Expected ~15 juniors, got {}", juniors);
        assert!(masters > 0, "Expected ~5 masters, got {}", masters);
    }

    #[test]
    fn test_experience_shift_ranges() {
        let (min, max) = CrewExperience::Junior.shift_length_range();
        assert!(min < max);
        let (min_m, _max_m) = CrewExperience::Master.shift_length_range();
        assert!(min_m >= min, "Master min should be >= Junior min");
    }
}
