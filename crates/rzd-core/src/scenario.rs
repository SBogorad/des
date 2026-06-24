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
}

impl Default for ScenarioConfig {
    fn default() -> Self {
        Self {
            crew_count: 50,
            loco_count: 20,
            seed: 42,
            duration_days: 30,
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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Scenario {
    pub crews: Vec<Crew>,
    pub locos: Vec<Locomotive>,
    pub config: ScenarioConfig,
}

pub fn generate_scenario(network: &Network, config: ScenarioConfig) -> Scenario {
    let mut prng = Mulberry32::new(config.seed);

    let mut depot_ids: Vec<String> = network.roads.values()
        .flat_map(|r| r.depots.iter().cloned())
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .collect();
    depot_ids.sort();

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

    // Generate crews
    let crews_per_depot = (config.crew_count as f32 / depot_ids.len() as f32).ceil() as usize;
    let mut crews = Vec::new();
    let mut crew_idx = 0;

    for depot_id in &depot_ids {
        let depot_stations = depot_station_map.get(depot_id)
            .cloned()
            .unwrap_or_default();
        let start_station = depot_stations.first()
            .cloned()
            .unwrap_or_else(|| network.stations.keys().next().unwrap().clone());

        let available_legs = depot_legs.get(depot_id)
            .cloned()
            .unwrap_or_default();

        let n = crews_per_depot.min(config.crew_count - crews.len());
        for _ in 0..n {
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
            .unwrap_or_else(|| network.stations.keys().next().unwrap().clone());

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

            locos.push(Locomotive {
                id: loco_id,
                series: series.id.clone(),
                loco_kind,
                sections: series.sections,
                depot_id: depot_id.clone(),
                state: LocoState::Available,
                position: start_station.clone(),
                mileage_km: 0.0,
                hours_since_to: 0.0,
                failures_total: 0,
            });
        }
    }

    Scenario {
        crews,
        locos,
        config,
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
        },
        LocoSeriesDef {
            id: "EL-4K".into(),
            name: "Электровоз 4-секционный".into(),
            kind: "Electric".into(),
            sections: 4,
            depot_ids: vec!["DEP-01".into(), "DEP-07".into()],
        },
        LocoSeriesDef {
            id: "EP-2K".into(),
            name: "Пассажирский электровоз".into(),
            kind: "Electric".into(),
            sections: 2,
            depot_ids: vec!["DEP-01".into(), "DEP-04".into()],
        },
        LocoSeriesDef {
            id: "DL-7H".into(),
            name: "Тепловоз 7-осный".into(),
            kind: "Diesel".into(),
            sections: 1,
            depot_ids: vec!["DEP-02".into(), "DEP-05".into(), "DEP-06".into(), "DEP-08".into()],
        },
        LocoSeriesDef {
            id: "DL-9H".into(),
            name: "Тепловоз 9-осный".into(),
            kind: "Diesel".into(),
            sections: 1,
            depot_ids: vec!["DEP-05".into(), "DEP-06".into(), "DEP-08".into()],
        },
        LocoSeriesDef {
            id: "TEP-70".into(),
            name: "Тепловоз пассажирский".into(),
            kind: "Diesel".into(),
            sections: 1,
            depot_ids: vec!["DEP-02".into(), "DEP-04".into()],
        },
        LocoSeriesDef {
            id: "2TE25A".into(),
            name: "Двухсекционный тепловоз".into(),
            kind: "Diesel".into(),
            sections: 2,
            depot_ids: vec!["DEP-05".into(), "DEP-06".into()],
        },
        LocoSeriesDef {
            id: "DUAL-2K".into(),
            name: "Двухmodes электровоз".into(),
            kind: "Dual".into(),
            sections: 2,
            depot_ids: vec!["DEP-03".into()],
        },
    ]
}
