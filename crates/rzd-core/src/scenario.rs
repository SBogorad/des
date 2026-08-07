use crate::entities::{Crew, CrewQualification, LocoKind, LocoState, Locomotive};
use crate::network::Network;
use crate::prng::Mulberry32;
use crate::synthetic_person::SyntheticPersonGenerator;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub const SCENARIO_SCHEMA_V1: &str = "rzd-des-scenario/v1";

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
    #[serde(default)]
    pub allowed_depot_ids: Vec<String>,
    #[serde(default)]
    pub allowed_leg_ids: Vec<String>,
}

fn default_shift_spread_min() -> u32 {
    30
}

impl Default for ScenarioConfig {
    fn default() -> Self {
        Self {
            crew_count: 50,
            loco_count: 20,
            seed: 42,
            duration_days: 30,
            shift_spread_min: 30,
            night_shift_ratio: 0.3,
            allowed_depot_ids: Vec::new(),
            allowed_leg_ids: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScenarioDocument {
    pub schema: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    pub config: ScenarioConfig,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
enum ScenarioDocumentInput {
    Versioned(ScenarioDocument),
    Bare(ScenarioConfig),
}

impl ScenarioDocument {
    pub fn new(config: ScenarioConfig) -> Self {
        Self {
            schema: SCENARIO_SCHEMA_V1.to_string(),
            name: None,
            description: None,
            config,
        }
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.schema != SCENARIO_SCHEMA_V1 {
            return Err(format!(
                "unsupported scenario schema '{}', expected {}",
                self.schema, SCENARIO_SCHEMA_V1
            ));
        }
        Ok(())
    }

    pub fn from_json(json: &str) -> Result<Self, String> {
        let parsed: ScenarioDocumentInput =
            serde_json::from_str(json).map_err(|e| format!("scenario JSON parse error: {e}"))?;
        match parsed {
            ScenarioDocumentInput::Versioned(doc) => {
                doc.validate()?;
                Ok(doc)
            }
            ScenarioDocumentInput::Bare(config) => Ok(Self::new(config)),
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
    let mut person_gen = SyntheticPersonGenerator::new(config.seed.wrapping_add(0xDEAD));
    let allowed_depots: std::collections::HashSet<&str> = config
        .allowed_depot_ids
        .iter()
        .map(String::as_str)
        .collect();
    let allowed_legs: std::collections::HashSet<&str> =
        config.allowed_leg_ids.iter().map(String::as_str).collect();
    let depot_allowed =
        |depot_id: &str| allowed_depots.is_empty() || allowed_depots.contains(depot_id);
    let leg_allowed = |leg_id: &str| allowed_legs.is_empty() || allowed_legs.contains(leg_id);

    let mut depot_ids: Vec<String> = network
        .roads
        .values()
        .flat_map(|r| r.depots.iter().cloned())
        .filter(|depot_id| depot_allowed(depot_id))
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
    for stations in depot_station_map.values_mut() {
        stations.sort();
    }

    let depot_legs: HashMap<String, Vec<String>> = {
        let mut map: HashMap<String, Vec<String>> = HashMap::new();
        for leg in network.legs.values() {
            if !leg_allowed(&leg.id) || !depot_allowed(&leg.from_depot) {
                continue;
            }
            map.entry(leg.from_depot.clone())
                .or_default()
                .push(leg.id.clone());
        }
        for legs in map.values_mut() {
            legs.sort();
        }
        map
    };

    let selected_leg_ids: std::collections::HashSet<&str> = if allowed_legs.is_empty() {
        network
            .legs
            .values()
            .filter(|leg| depot_allowed(&leg.from_depot))
            .map(|leg| leg.id.as_str())
            .collect()
    } else {
        network
            .legs
            .values()
            .filter(|leg| leg_allowed(&leg.id) && depot_allowed(&leg.from_depot))
            .map(|leg| leg.id.as_str())
            .collect()
    };
    let station_ids_in_scope: std::collections::HashSet<&str> = selected_leg_ids
        .iter()
        .filter_map(|leg_id| network.legs.get(*leg_id))
        .flat_map(|leg| leg.segments.iter())
        .filter_map(|segment_id| network.segments.get(segment_id))
        .flat_map(|segment| [segment.from.as_str(), segment.to.as_str()])
        .collect();
    let mut regions_covered: Vec<String> = network
        .stations
        .values()
        .filter(|station| {
            station_ids_in_scope.is_empty() || station_ids_in_scope.contains(station.id.as_str())
        })
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
        let depot_stations = depot_station_map.get(depot_id).cloned().unwrap_or_default();
        let start_station = depot_stations
            .first()
            .cloned()
            .unwrap_or_else(|| network.stations.keys().min().cloned().unwrap_or_default());

        let available_legs = depot_legs.get(depot_id).cloned().unwrap_or_default();

        let n = crews_per_depot.min(config.crew_count - crews.len());
        for i in 0..n {
            if crew_idx >= config.crew_count {
                break;
            }
            let experience = experiences
                .get(crew_idx)
                .copied()
                .unwrap_or(CrewExperience::Standard);
            let (min_shift, max_shift) = experience.shift_length_range();

            crew_idx += 1;
            let crew_id = format!("CREW-{:04}", crew_idx);

            let mut crew = Crew::new(crew_id, depot_id.clone(), start_station.clone());

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

            crew.personnel_number = person_gen.generate_personnel_number();
            crew.full_name = person_gen.generate_driver_name();
            crew.assistant_personnel_number = person_gen.generate_personnel_number();
            crew.assistant_full_name = person_gen.generate_assistant_name();
            crew.qualification = Some(match experience {
                CrewExperience::Junior => CrewQualification::Junior,
                CrewExperience::Standard => CrewQualification::Assistant,
                CrewExperience::Senior => CrewQualification::Senior,
                CrewExperience::Master => CrewQualification::Instructor,
            });

            crews.push(crew);
        }
    }

    // Generate locomotives
    let series_defs = default_loco_series();
    let locos_per_depot = (config.loco_count as f32 / depot_ids.len() as f32).ceil() as usize;
    let mut locos = Vec::new();
    let mut loco_idx = 0;

    for depot_id in &depot_ids {
        let depot_stations = depot_station_map.get(depot_id).cloned().unwrap_or_default();
        let start_station = depot_stations
            .first()
            .cloned()
            .unwrap_or_else(|| network.stations.keys().min().cloned().unwrap_or_default());

        let matching_series: Vec<&LocoSeriesDef> = series_defs
            .iter()
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
                inventory_number: person_gen.generate_loco_number(500, 9999),
                manufacture_year: 2000 + (person_gen.next_u32() % 25) as u16,
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
            legs_assigned: selected_leg_ids.len(),
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
            depot_ids: vec![
                "DEP-01".into(),
                "DEP-03".into(),
                "DEP-04".into(),
                "DEP-10".into(),
                "SD-1".into(),
                "SD-2".into(),
                "TD-72".into(),
                "TD-74".into(),
            ],
            max_shift_hours: 12.0,
            acceleration_kmh_per_s: 0.6,
        },
        LocoSeriesDef {
            id: "EL-4K".into(),
            name: "Электровоз 4-секционный".into(),
            kind: "Electric".into(),
            sections: 4,
            depot_ids: vec![
                "DEP-01".into(),
                "DEP-07".into(),
                "SD-1".into(),
                "SD-5".into(),
            ],
            max_shift_hours: 12.0,
            acceleration_kmh_per_s: 0.5,
        },
        LocoSeriesDef {
            id: "EP-2K".into(),
            name: "Пассажирский электровоз".into(),
            kind: "Electric".into(),
            sections: 2,
            depot_ids: vec![
                "DEP-01".into(),
                "DEP-04".into(),
                "SD-2".into(),
                "TD-72".into(),
                "TD-74".into(),
            ],
            max_shift_hours: 10.0,
            acceleration_kmh_per_s: 0.7,
        },
        LocoSeriesDef {
            id: "DL-7H".into(),
            name: "Тепловоз 7-осный".into(),
            kind: "Diesel".into(),
            sections: 1,
            depot_ids: vec![
                "DEP-02".into(),
                "DEP-05".into(),
                "DEP-06".into(),
                "DEP-08".into(),
                "SD-7".into(),
                "TD-71".into(),
                "TD-75".into(),
                "TD-77".into(),
            ],
            max_shift_hours: 10.0,
            acceleration_kmh_per_s: 0.4,
        },
        LocoSeriesDef {
            id: "DL-9H".into(),
            name: "Тепловоз 9-осный".into(),
            kind: "Diesel".into(),
            sections: 1,
            depot_ids: vec![
                "DEP-05".into(),
                "DEP-06".into(),
                "DEP-08".into(),
                "TD-71".into(),
                "TD-75".into(),
                "TD-77".into(),
            ],
            max_shift_hours: 10.0,
            acceleration_kmh_per_s: 0.35,
        },
        LocoSeriesDef {
            id: "TEP-70".into(),
            name: "Тепловоз пассажирский".into(),
            kind: "Diesel".into(),
            sections: 1,
            depot_ids: vec![
                "DEP-02".into(),
                "DEP-04".into(),
                "SD-7".into(),
                "TD-71".into(),
            ],
            max_shift_hours: 10.0,
            acceleration_kmh_per_s: 0.45,
        },
        LocoSeriesDef {
            id: "2TE25A".into(),
            name: "Двухсекционный тепловоз".into(),
            kind: "Diesel".into(),
            sections: 2,
            depot_ids: vec![
                "DEP-05".into(),
                "DEP-06".into(),
                "SD-7".into(),
                "TD-71".into(),
                "TD-75".into(),
            ],
            max_shift_hours: 12.0,
            acceleration_kmh_per_s: 0.3,
        },
        LocoSeriesDef {
            id: "DUAL-2K".into(),
            name: "Двухmodes электровоз".into(),
            kind: "Dual".into(),
            sections: 2,
            depot_ids: vec![
                "DEP-03".into(),
                "SD-3".into(),
                "SD-5".into(),
                "TD-72".into(),
                "TD-74".into(),
            ],
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
        let juniors = exps
            .iter()
            .filter(|e| **e == CrewExperience::Junior)
            .count();
        let masters = exps
            .iter()
            .filter(|e| **e == CrewExperience::Master)
            .count();
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

    #[test]
    fn synthetic_one_road_depots_have_matching_series() {
        let network_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("data")
            .join("networks")
            .join("synthetic_one_road_88_05.json");
        let json = std::fs::read_to_string(network_path).expect("synthetic network should exist");
        let network = Network::from_json(&json).expect("synthetic network should parse");
        let series_defs = default_loco_series();

        for depot_id in network.roads.values().flat_map(|road| road.depots.iter()) {
            assert!(
                series_defs
                    .iter()
                    .any(|series| series.depot_ids.contains(depot_id)),
                "Depot {depot_id} must have at least one matching loco series"
            );
        }
    }

    #[test]
    fn scenario_document_accepts_bare_config_json() {
        let json = r#"{
            "crew_count": 12,
            "loco_count": 6,
            "seed": 42,
            "duration_days": 7,
            "shift_spread_min": 30,
            "night_shift_ratio": 0.3,
            "allowed_depot_ids": ["SD-1"],
            "allowed_leg_ids": ["LEG-01"]
        }"#;

        let doc = ScenarioDocument::from_json(json).expect("bare config should parse");
        assert_eq!(doc.schema, SCENARIO_SCHEMA_V1);
        assert_eq!(doc.config.crew_count, 12);
        assert_eq!(doc.config.allowed_depot_ids, vec!["SD-1"]);
    }

    #[test]
    fn scenario_document_accepts_versioned_json() {
        let json = r#"{
            "schema": "rzd-des-scenario/v1",
            "name": "one-road-shoulder-a",
            "description": "Synthetic western shoulder month",
            "config": {
                "crew_count": 60,
                "loco_count": 30,
                "seed": 20260708,
                "duration_days": 30,
                "shift_spread_min": 30,
                "night_shift_ratio": 0.3,
                "allowed_depot_ids": ["SD-1", "SD-2", "TD-72"],
                "allowed_leg_ids": ["LEG-01", "LEG-02", "LEG-03"]
            }
        }"#;

        let doc = ScenarioDocument::from_json(json).expect("versioned contract should parse");
        assert_eq!(doc.schema, SCENARIO_SCHEMA_V1);
        assert_eq!(doc.name.as_deref(), Some("one-road-shoulder-a"));
        assert_eq!(doc.config.loco_count, 30);
    }

    #[test]
    fn scenario_document_rejects_unknown_schema() {
        let json = r#"{
            "schema": "rzd-des-scenario/v99",
            "config": {
                "crew_count": 1,
                "loco_count": 1,
                "seed": 1,
                "duration_days": 1
            }
        }"#;

        let err = ScenarioDocument::from_json(json).expect_err("unknown schema must fail");
        assert!(err.contains("unsupported scenario schema"));
    }

    #[test]
    fn scenario_document_new_sets_expected_schema() {
        let doc = ScenarioDocument::new(ScenarioConfig::default());
        assert_eq!(doc.schema, SCENARIO_SCHEMA_V1);
    }

    #[test]
    fn scenario_document_round_trips_shared_contract_fixture() {
        let fixture = include_str!("../../../contracts/scenario-v1.fixture.json");
        let document = ScenarioDocument::from_json(fixture).expect("shared fixture should parse");
        let encoded = serde_json::to_string(&document).expect("document should serialize");
        let reparsed =
            ScenarioDocument::from_json(&encoded).expect("serialized document should parse");
        assert_eq!(reparsed.schema, document.schema);
        assert_eq!(reparsed.name, document.name);
        assert_eq!(reparsed.description, document.description);
        assert_eq!(reparsed.config.crew_count, document.config.crew_count);
        assert_eq!(reparsed.config.loco_count, document.config.loco_count);
        assert_eq!(reparsed.config.seed, document.config.seed);
        assert_eq!(reparsed.config.duration_days, document.config.duration_days);
        assert_eq!(
            reparsed.config.allowed_depot_ids,
            document.config.allowed_depot_ids
        );
        assert_eq!(
            reparsed.config.allowed_leg_ids,
            document.config.allowed_leg_ids
        );
        assert_eq!(
            reparsed.config.night_shift_ratio,
            document.config.night_shift_ratio
        );
    }
}
