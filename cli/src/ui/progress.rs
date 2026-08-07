use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

/// Interactive progress tracker with ETA, throughput, and animation
pub struct ProgressTracker {
    current: AtomicU64,
    total: u64,
    start: Instant,
    label: String,
}

impl ProgressTracker {
    pub fn new(label: &str, total: u64) -> Self {
        let tracker = ProgressTracker {
            current: AtomicU64::new(0),
            total,
            start: Instant::now(),
            label: label.to_string(),
        };
        tracker.print_initial();
        tracker
    }

    pub fn update(&self, completed: u64) {
        self.current.store(completed, Ordering::Relaxed);
        self.print_progress();
    }

    fn print_initial(&self) {
        eprint!("\r\x1b[K[{}] Starting...", self.label);
    }

    fn print_progress(&self) {
        let current = self.current.load(Ordering::Relaxed);
        let elapsed = self.start.elapsed();
        let percent = (current as f64 / self.total as f64 * 100.0).min(100.0);

        let eps = if elapsed.as_secs_f64() > 0.0 {
            current as f64 / elapsed.as_secs_f64()
        } else {
            0.0
        };

        let eta = if eps > 0.0 && current < self.total {
            let remaining = self.total - current;
            Duration::from_secs_f64(remaining as f64 / eps)
        } else {
            Duration::from_secs(0)
        };

        let bar_width = 30;
        let filled = (percent / 100.0 * bar_width as f64) as usize;
        let empty = bar_width - filled;

        let bar: String = "█".repeat(filled) + &"░".repeat(empty);

        eprint!(
            "\r\x1b[K[{}] |{}| {:.1}% | {} events | {:.0} eps | ETA: {}",
            self.label,
            bar,
            percent,
            format_number(current),
            eps,
            format_duration(eta)
        );
    }

    pub fn finish(&self) {
        let elapsed = self.start.elapsed();
        let total = self.current.load(Ordering::Relaxed);
        let eps = if elapsed.as_secs_f64() > 0.0 {
            total as f64 / elapsed.as_secs_f64()
        } else {
            0.0
        };

        eprintln!(
            "\r\x1b[K[{}] ✓ Complete: {} events in {} ({:.0} eps)",
            self.label,
            format_number(total),
            format_duration(elapsed),
            eps
        );
    }
}

impl Drop for ProgressTracker {
    fn drop(&mut self) {
        if self.current.load(Ordering::Relaxed) < self.total {
            eprintln!("\r\x1b[K[{}] ✗ Interrupted", self.label);
        }
    }
}

/// Format large numbers with commas
fn format_number(n: u64) -> String {
    let s = n.to_string();
    let mut result = String::new();
    for (i, c) in s.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            result.push(',');
        }
        result.push(c);
    }
    result.chars().rev().collect()
}

/// Format duration to human readable
fn format_duration(d: Duration) -> String {
    let secs = d.as_secs();
    if secs < 60 {
        format!("{}s", secs)
    } else if secs < 3600 {
        format!("{}m {}s", secs / 60, secs % 60)
    } else {
        format!("{}h {}m", secs / 3600, (secs % 3600) / 60)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_number() {
        assert_eq!(format_number(0), "0");
        assert_eq!(format_number(123), "123");
        assert_eq!(format_number(1234), "1,234");
        assert_eq!(format_number(1234567), "1,234,567");
    }

    #[test]
    fn test_format_duration() {
        assert_eq!(format_duration(Duration::from_secs(30)), "30s");
        assert_eq!(format_duration(Duration::from_secs(90)), "1m 30s");
        assert_eq!(format_duration(Duration::from_secs(3661)), "1h 1m");
    }
}
