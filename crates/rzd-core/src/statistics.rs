use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunningStats {
    count: u64,
    mean: f64,
    m2: f64,
    min: f64,
    max: f64,
}

impl RunningStats {
    pub fn new() -> Self {
        Self { count: 0, mean: 0.0, m2: 0.0, min: f64::INFINITY, max: f64::NEG_INFINITY }
    }

    pub fn update(&mut self, value: f64) {
        self.count += 1;
        let delta = value - self.mean;
        self.mean += delta / self.count as f64;
        let delta2 = value - self.mean;
        self.m2 += delta * delta2;
        if value < self.min { self.min = value; }
        if value > self.max { self.max = value; }
    }

    pub fn count(&self) -> u64 { self.count }
    pub fn mean(&self) -> f64 { self.mean }
    pub fn min(&self) -> f64 { self.min }
    pub fn max(&self) -> f64 { self.max }

    pub fn variance(&self) -> f64 {
        if self.count < 2 { 0.0 } else { self.m2 / (self.count - 1) as f64 }
    }

    pub fn std_dev(&self) -> f64 {
        self.variance().sqrt()
    }
}

impl Default for RunningStats {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeWeightedAverage {
    area: f64,
    last_change: f64,
    last_value: f64,
    total_time: f64,
    initialized: bool,
}

impl TimeWeightedAverage {
    pub fn new() -> Self {
        Self { area: 0.0, last_change: 0.0, last_value: 0.0, total_time: 0.0, initialized: false }
    }

    pub fn update(&mut self, current_time: f64, new_value: f64) {
        if self.initialized {
            let dt = current_time - self.last_change;
            self.area += self.last_value * dt;
        }
        self.initialized = true;
        self.last_value = new_value;
        self.last_change = current_time;
        self.total_time = current_time;
    }

    pub fn average(&self) -> f64 {
        if self.total_time <= 0.0 { 0.0 } else { self.area / self.total_time }
    }

    pub fn current(&self) -> f64 {
        self.last_value
    }
}

impl Default for TimeWeightedAverage {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Histogram {
    bins: Vec<HistogramBin>,
    total_count: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistogramBin {
    pub lower: f64,
    pub upper: f64,
    pub count: u64,
}

impl Histogram {
    pub fn new(min: f64, max: f64, num_bins: usize) -> Self {
        let bin_width = (max - min) / num_bins as f64;
        let bins = (0..num_bins).map(|i| HistogramBin {
            lower: min + i as f64 * bin_width,
            upper: min + (i + 1) as f64 * bin_width,
            count: 0,
        }).collect();
        Self { bins, total_count: 0 }
    }

    pub fn update(&mut self, value: f64) {
        self.total_count += 1;
        for bin in &mut self.bins {
            if value >= bin.lower && value < bin.upper {
                bin.count += 1;
                return;
            }
        }
        if let Some(last) = self.bins.last_mut() {
            if value >= last.lower {
                last.count += 1;
            }
        }
    }

    pub fn percentile(&self, p: f64) -> f64 {
        let target = (self.total_count as f64 * p / 100.0) as u64;
        let mut cumulative = 0u64;
        for bin in &self.bins {
            cumulative += bin.count;
            if cumulative >= target {
                return bin.lower;
            }
        }
        self.bins.last().map_or(0.0, |b| b.upper)
    }

    pub fn p50(&self) -> f64 { self.percentile(50.0) }
    pub fn p90(&self) -> f64 { self.percentile(90.0) }
    pub fn p99(&self) -> f64 { self.percentile(99.0) }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimulationStats {
    pub total_events_processed: u64,
    pub total_violations: u64,
    pub total_shifts_completed: u64,
    pub total_distance_km: f64,
    pub work_time: RunningStats,
    pub acceptance_time: RunningStats,
    pub repair_time: RunningStats,
    pub wait_time: RunningStats,
    pub loco_utilization: TimeWeightedAverage,
    pub crew_utilization: TimeWeightedAverage,
    pub shift_duration: Histogram,
    pub acceptance_duration: Histogram,
    pub repair_duration: Histogram,
}

impl SimulationStats {
    pub fn new() -> Self {
        Self {
            total_events_processed: 0,
            total_violations: 0,
            total_shifts_completed: 0,
            total_distance_km: 0.0,
            work_time: RunningStats::new(),
            acceptance_time: RunningStats::new(),
            repair_time: RunningStats::new(),
            wait_time: RunningStats::new(),
            loco_utilization: TimeWeightedAverage::new(),
            crew_utilization: TimeWeightedAverage::new(),
            shift_duration: Histogram::new(0.0, 1440.0, 24),
            acceptance_duration: Histogram::new(0.0, 300.0, 15),
            repair_duration: Histogram::new(0.0, 300.0, 15),
        }
    }

    pub fn record_shift(&mut self, work_min: u32, distance_km: f32) {
        self.total_shifts_completed += 1;
        self.work_time.update(work_min as f64);
        self.shift_duration.update(work_min as f64);
        self.total_distance_km += distance_km as f64;
    }

    pub fn record_acceptance(&mut self, duration_min: u32) {
        self.acceptance_time.update(duration_min as f64);
        self.acceptance_duration.update(duration_min as f64);
    }

    pub fn record_repair(&mut self, duration_min: u64) {
        self.repair_time.update(duration_min as f64);
        self.repair_duration.update(duration_min as f64);
    }

    pub fn record_wait(&mut self, duration_min: u64) {
        self.wait_time.update(duration_min as f64);
    }

    pub fn record_event(&mut self) {
        self.total_events_processed += 1;
    }

    pub fn record_violation(&mut self) {
        self.total_violations += 1;
    }
}

impl Default for SimulationStats {
    fn default() -> Self {
        Self::new()
    }
}
