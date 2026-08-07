use rzd_core::engine::Engine;
use rzd_core::entities::{Crew, LocoKind, LocoState, Locomotive};
use rzd_core::network::Network;
use rzd_core::scenario::{generate_scenario, ScenarioConfig};
use rzd_core::tu3::tsv_escape;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn init_panic_hook() {
    console_error_panic_hook::set_once();
}

#[wasm_bindgen]
pub struct RzdSimulatorWasm {
    engine: Option<Engine>,
    seed: u32,
}

impl Default for RzdSimulatorWasm {
    fn default() -> Self {
        Self::new()
    }
}

#[wasm_bindgen]
impl RzdSimulatorWasm {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self {
            engine: None,
            seed: 42,
        }
    }

    #[wasm_bindgen]
    pub fn with_seed(seed: u32) -> Self {
        Self { engine: None, seed }
    }

    pub fn status(&self) -> String {
        if self.engine.is_some() {
            "ready".to_string()
        } else {
            "empty".to_string()
        }
    }

    #[wasm_bindgen]
    pub fn load_network(&mut self, network_json: &str) -> Result<(), JsValue> {
        let network =
            Network::from_json(network_json).map_err(|e| JsValue::from_str(&e.to_string()))?;
        self.engine = Some(Engine::new(network, self.seed));
        Ok(())
    }

    #[wasm_bindgen]
    pub fn schedule_shifts(&mut self, start_offset: u64, interval: u64) -> Result<(), JsValue> {
        let engine = self
            .engine
            .as_mut()
            .ok_or_else(|| JsValue::from_str("Network not loaded"))?;
        engine.schedule_all_shifts(start_offset, interval);
        Ok(())
    }

    #[wasm_bindgen]
    pub fn set_warmup(&mut self, minutes: u64) -> Result<(), JsValue> {
        let engine = self
            .engine
            .as_mut()
            .ok_or_else(|| JsValue::from_str("Network not loaded"))?;
        engine.set_warmup(minutes);
        Ok(())
    }

    #[wasm_bindgen]
    pub fn load_scenario(&mut self, scenario_json: &str) -> Result<(), JsValue> {
        let scenario: ScenarioConfig =
            serde_json::from_str(scenario_json).map_err(|e| JsValue::from_str(&e.to_string()))?;

        let engine = self
            .engine
            .as_mut()
            .ok_or_else(|| JsValue::from_str("Network not loaded"))?;

        let network = engine.network.clone();
        let gen = generate_scenario(&network, scenario);

        for crew in gen.crews {
            engine.add_crew(crew);
        }
        for loco in gen.locos {
            engine.add_loco(loco);
        }

        Ok(())
    }

    #[wasm_bindgen]
    pub fn generate_and_load_scenario(
        &mut self,
        network_json: &str,
        config_json: &str,
    ) -> Result<(), JsValue> {
        let config: ScenarioConfig =
            serde_json::from_str(config_json).map_err(|e| JsValue::from_str(&e.to_string()))?;

        let seed = config.seed;
        let network =
            Network::from_json(network_json).map_err(|e| JsValue::from_str(&e.to_string()))?;

        let gen = generate_scenario(&network, config);
        let mut engine = Engine::new(network, seed);

        for crew in gen.crews {
            engine.add_crew(crew);
        }
        for loco in gen.locos {
            engine.add_loco(loco);
        }

        self.engine = Some(engine);
        Ok(())
    }

    #[wasm_bindgen]
    pub fn add_crew(
        &mut self,
        id: &str,
        depot_id: &str,
        position: &str,
        route_json: &str,
    ) -> Result<(), JsValue> {
        let engine = self
            .engine
            .as_mut()
            .ok_or_else(|| JsValue::from_str("Network not loaded"))?;

        let route_segments: Vec<String> =
            serde_json::from_str(route_json).map_err(|e| JsValue::from_str(&e.to_string()))?;

        let mut crew = Crew::new(id.to_string(), depot_id.to_string(), position.to_string());
        crew.route_segments = route_segments;

        engine.add_crew(crew);
        Ok(())
    }

    #[wasm_bindgen]
    pub fn add_loco(
        &mut self,
        id: &str,
        series: &str,
        sections: u8,
        depot_id: &str,
        position: &str,
    ) -> Result<(), JsValue> {
        let engine = self
            .engine
            .as_mut()
            .ok_or_else(|| JsValue::from_str("Network not loaded"))?;

        let loco = Locomotive {
            id: id.to_string(),
            series: series.to_string(),
            loco_kind: LocoKind::Electric,
            sections,
            depot_id: depot_id.to_string(),
            state: LocoState::Available,
            position: position.to_string(),
            mileage_km: 0.0,
            hours_since_to: 0.0,
            failures_total: 0,
            inventory_number: 0,
            manufacture_year: 2015,
        };

        engine.add_loco(loco);
        Ok(())
    }

    #[wasm_bindgen]
    pub fn run(&mut self, until_minutes: u64) -> Result<String, JsValue> {
        let engine = self
            .engine
            .as_mut()
            .ok_or_else(|| JsValue::from_str("Network not loaded"))?;

        engine.run(until_minutes);

        let result = serde_json::json!({
            "crew_count": engine.crew_count(),
            "loco_count": engine.loco_count(),
            "tu3_rows": engine.tu3_rows.len(),
            "violations": engine.violations.summary(),
            "current_time": engine.current_time,
        });

        serde_json::to_string(&result).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    #[wasm_bindgen]
    pub fn get_violations_json(&self) -> Result<String, JsValue> {
        let engine = self
            .engine
            .as_ref()
            .ok_or_else(|| JsValue::from_str("Network not loaded"))?;

        serde_json::to_string(&engine.violations.violations)
            .map_err(|e| JsValue::from_str(&e.to_string()))
    }

    #[wasm_bindgen]
    pub fn get_tu3_json(&self) -> Result<String, JsValue> {
        let engine = self
            .engine
            .as_ref()
            .ok_or_else(|| JsValue::from_str("Network not loaded"))?;

        serde_json::to_string(&engine.tu3_rows).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    #[wasm_bindgen]
    pub fn get_summary(&self) -> Result<String, JsValue> {
        let engine = self
            .engine
            .as_ref()
            .ok_or_else(|| JsValue::from_str("Network not loaded"))?;

        let summary = serde_json::json!({
            "crew_count": engine.crew_count(),
            "loco_count": engine.loco_count(),
            "tu3_rows": engine.tu3_rows.len(),
            "violations": engine.violations.summary(),
            "current_time": engine.current_time,
            "total_violations": engine.violations.violations.len(),
        });

        serde_json::to_string(&summary).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    #[wasm_bindgen]
    pub fn get_stats_json(&self) -> Result<String, JsValue> {
        let engine = self
            .engine
            .as_ref()
            .ok_or_else(|| JsValue::from_str("Network not loaded"))?;

        serde_json::to_string(engine.stats()).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    #[wasm_bindgen]
    pub fn get_crew_state(&self, crew_id: &str) -> Result<String, JsValue> {
        let engine = self
            .engine
            .as_ref()
            .ok_or_else(|| JsValue::from_str("Network not loaded"))?;

        let crew = engine
            .crews
            .get(crew_id)
            .ok_or_else(|| JsValue::from_str("Crew not found"))?;

        serde_json::to_string(crew).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    #[wasm_bindgen]
    pub fn export_tu3_tsv(&self) -> Result<String, JsValue> {
        let engine = self
            .engine
            .as_ref()
            .ok_or_else(|| JsValue::from_str("Network not loaded"))?;

        let mut tsv = String::new();
        tsv.push_str(
            "crew_id\tloco_id\tshift_start\tshift_end\twork_min\tdistance_km\tviolations\n",
        );
        for row in &engine.tu3_rows {
            tsv.push_str(&format!(
                "{}\t{}\t{}\t{}\t{}\t{}\t{}\n",
                tsv_escape(&row.crew_id),
                tsv_escape(&row.loco_id),
                row.shift_start,
                row.shift_end.map_or("".to_string(), |v| v.to_string()),
                row.work_duration_min,
                row.total_distance_km,
                row.violations_count,
            ));
        }
        Ok(tsv)
    }

    #[wasm_bindgen]
    pub fn export_ced_json(&self) -> Result<String, JsValue> {
        let engine = self
            .engine
            .as_ref()
            .ok_or_else(|| JsValue::from_str("Network not loaded"))?;

        let ced = engine.export_ced();
        serde_json::to_string(&ced).map_err(|e| JsValue::from_str(&e.to_string()))
    }
}
