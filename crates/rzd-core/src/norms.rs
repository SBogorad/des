use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RzdNorms {
    // Work time
    pub max_shift_8h_min: u32,     // 480 — warning
    pub max_shift_12h_min: u32,    // 720 — critical
    pub max_monthly_work_min: u32, // 10 800 (180h)

    // Rest
    pub min_home_rest_min: u32,       // 960 (16h)
    pub min_turnaround_rest_min: u32, // 180 (3h)
    pub turnaround_rest_ratio: f32,   // 0.5 (50% of work time)

    // Night shifts
    pub max_night_shifts_in_row: u8, // 2

    // Handover/Acceptance
    pub acceptance_per_section_min: u32, // 20 min/section
    pub handover_min_min: u32,           // 15
    pub handover_max_min: u32,           // 30

    // Probabilities (calibrated)
    pub waiting_probability: f32,       // 0.18
    pub waiting_min_min: u32,           // 20
    pub waiting_max_min: u32,           // 90
    pub line_change_on_critical: f32,   // 0.60
    pub reserve_after_line_change: f32, // 0.70

    // Voltage
    pub voltage_sag_threshold_dc: f32, // 2400 V (norm 3000 V)
    pub voltage_sag_threshold_ac: f32, // 21000 V (norm 25000 V)
    pub voltage_sag_speed_factor: f32, // 0.7
}

impl RzdNorms {
    pub fn apply_region_overrides(&mut self, overrides: &RegionNorms) {
        if let Some(v) = overrides.max_shift_8h_min {
            self.max_shift_8h_min = v;
        }
        if let Some(v) = overrides.max_shift_12h_min {
            self.max_shift_12h_min = v;
        }
        if let Some(v) = overrides.max_monthly_work_min {
            self.max_monthly_work_min = v;
        }
        if let Some(v) = overrides.min_home_rest_min {
            self.min_home_rest_min = v;
        }
        if let Some(v) = overrides.min_turnaround_rest_min {
            self.min_turnaround_rest_min = v;
        }
        if let Some(v) = overrides.turnaround_rest_ratio {
            self.turnaround_rest_ratio = v;
        }
        if let Some(v) = overrides.max_night_shifts_in_row {
            self.max_night_shifts_in_row = v;
        }
        if let Some(v) = overrides.acceptance_per_section_min {
            self.acceptance_per_section_min = v;
        }
        if let Some(v) = overrides.handover_min_min {
            self.handover_min_min = v;
        }
        if let Some(v) = overrides.handover_max_min {
            self.handover_max_min = v;
        }
        if let Some(v) = overrides.waiting_probability {
            self.waiting_probability = v;
        }
        if let Some(v) = overrides.waiting_min_min {
            self.waiting_min_min = v;
        }
        if let Some(v) = overrides.waiting_max_min {
            self.waiting_max_min = v;
        }
        if let Some(v) = overrides.line_change_on_critical {
            self.line_change_on_critical = v;
        }
        if let Some(v) = overrides.reserve_after_line_change {
            self.reserve_after_line_change = v;
        }
        if let Some(v) = overrides.voltage_sag_threshold_dc {
            self.voltage_sag_threshold_dc = v;
        }
        if let Some(v) = overrides.voltage_sag_threshold_ac {
            self.voltage_sag_threshold_ac = v;
        }
        if let Some(v) = overrides.voltage_sag_speed_factor {
            self.voltage_sag_speed_factor = v;
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegionNorms {
    pub region_id: String,
    pub failure_rate_multiplier: f32,
    pub voltage_sag_multiplier: f32,
    pub speed_multiplier: f32,

    pub max_shift_8h_min: Option<u32>,
    pub max_shift_12h_min: Option<u32>,
    pub max_monthly_work_min: Option<u32>,
    pub min_home_rest_min: Option<u32>,
    pub min_turnaround_rest_min: Option<u32>,
    pub turnaround_rest_ratio: Option<f32>,
    pub max_night_shifts_in_row: Option<u8>,
    pub acceptance_per_section_min: Option<u32>,
    pub handover_min_min: Option<u32>,
    pub handover_max_min: Option<u32>,
    pub waiting_probability: Option<f32>,
    pub waiting_min_min: Option<u32>,
    pub waiting_max_min: Option<u32>,
    pub line_change_on_critical: Option<f32>,
    pub reserve_after_line_change: Option<f32>,
    pub voltage_sag_threshold_dc: Option<f32>,
    pub voltage_sag_threshold_ac: Option<f32>,
    pub voltage_sag_speed_factor: Option<f32>,
}

impl Default for RegionNorms {
    fn default() -> Self {
        Self {
            region_id: String::new(),
            failure_rate_multiplier: 1.0,
            voltage_sag_multiplier: 1.0,
            speed_multiplier: 1.0,
            max_shift_8h_min: None,
            max_shift_12h_min: None,
            max_monthly_work_min: None,
            min_home_rest_min: None,
            min_turnaround_rest_min: None,
            turnaround_rest_ratio: None,
            max_night_shifts_in_row: None,
            acceptance_per_section_min: None,
            handover_min_min: None,
            handover_max_min: None,
            waiting_probability: None,
            waiting_min_min: None,
            waiting_max_min: None,
            line_change_on_critical: None,
            reserve_after_line_change: None,
            voltage_sag_threshold_dc: None,
            voltage_sag_threshold_ac: None,
            voltage_sag_speed_factor: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegionNormsConfig {
    pub regions: HashMap<String, RegionNorms>,
}

impl RegionNormsConfig {
    pub fn new() -> Self {
        Self {
            regions: HashMap::new(),
        }
    }

    pub fn add(&mut self, region_id: String, norms: RegionNorms) {
        self.regions.insert(region_id, norms);
    }

    pub fn get(&self, region_id: &str) -> Option<&RegionNorms> {
        self.regions.get(region_id)
    }

    pub fn get_default(&self, region_id: &str) -> RegionNorms {
        self.regions
            .get(region_id)
            .cloned()
            .unwrap_or_else(|| RegionNorms {
                region_id: region_id.to_string(),
                failure_rate_multiplier: 1.0,
                voltage_sag_multiplier: 1.0,
                speed_multiplier: 1.0,
                ..Default::default()
            })
    }
}

impl Default for RegionNormsConfig {
    fn default() -> Self {
        Self::new()
    }
}

impl Default for RzdNorms {
    fn default() -> Self {
        Self {
            max_shift_8h_min: 480,
            max_shift_12h_min: 720,
            max_monthly_work_min: 10800,
            min_home_rest_min: 960,
            min_turnaround_rest_min: 180,
            turnaround_rest_ratio: 0.5,
            max_night_shifts_in_row: 2,
            acceptance_per_section_min: 20,
            handover_min_min: 15,
            handover_max_min: 30,
            waiting_probability: 0.18,
            waiting_min_min: 20,
            waiting_max_min: 90,
            line_change_on_critical: 0.60,
            reserve_after_line_change: 0.70,
            voltage_sag_threshold_dc: 2400.0,
            voltage_sag_threshold_ac: 21000.0,
            voltage_sag_speed_factor: 0.7,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub enum NormsError {
    InvalidFormat,
    ValueOutOfRange,
}
