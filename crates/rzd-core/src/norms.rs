use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RzdNorms {
    // Work time
    pub max_shift_8h_min: u32,          // 480 — warning
    pub max_shift_12h_min: u32,         // 720 — critical
    pub max_monthly_work_min: u32,      // 10 800 (180h)

    // Rest
    pub min_home_rest_min: u32,         // 960 (16h)
    pub min_turnaround_rest_min: u32,   // 180 (3h)
    pub turnaround_rest_ratio: f32,     // 0.5 (50% of work time)

    // Night shifts
    pub max_night_shifts_in_row: u8,    // 2

    // Handover/Acceptance
    pub acceptance_per_section_min: u32, // 20 min/section
    pub handover_min_min: u32,           // 15
    pub handover_max_min: u32,           // 30

    // Probabilities (calibrated)
    pub waiting_probability: f32,        // 0.18
    pub waiting_min_min: u32,            // 20
    pub waiting_max_min: u32,            // 90
    pub line_change_on_critical: f32,    // 0.60
    pub reserve_after_line_change: f32,  // 0.70

    // Voltage
    pub voltage_sag_threshold_dc: f32,   // 2400 V (norm 3000 V)
    pub voltage_sag_threshold_ac: f32,   // 21000 V (norm 25000 V)
    pub voltage_sag_speed_factor: f32,   // 0.7
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
