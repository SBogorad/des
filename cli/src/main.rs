use clap::Parser;
use rzd_core::engine::{Engine, EngineSnapshot};
use rzd_core::events::ViolationType;
use rzd_core::export::{build_replay_bundle, to_canonical_json, Export, ReplayBundleInputs};
use rzd_core::injection::{InjectedEvent, InjectionPlan};
use rzd_core::network::Network;
use rzd_core::scenario::{generate_scenario, ScenarioConfig, ScenarioDocument};
use rzd_core::violations::Severity;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::{Path, PathBuf};
use std::time::Instant;

// UI module
mod commands;
use commands::Commands;
mod ui;
use ui::ProgressTracker;

/// Sanitize a user-provided output path to prevent directory traversal attacks.
/// Returns a canonicalized absolute path.
fn sanitize_output_path(path: &Path) -> Result<PathBuf, Box<dyn std::error::Error>> {
    // Reject paths with suspicious components
    for component in path.components() {
        if let std::path::Component::ParentDir = component {
            return Err("Path traversal detected: '..' components are not allowed".into());
        }
    }
    
    // Convert to absolute path
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()?.join(path)
    };
    
    // Canonicalize to resolve any remaining relative components
    match absolute.canonicalize() {
        Ok(canonical) => Ok(canonical),
        Err(_) => {
            // Path doesn't exist yet; validate parent exists and is accessible
            if let Some(parent) = absolute.parent() {
                std::fs::create_dir_all(parent)?;
                Ok(absolute)
            } else {
                Err("Invalid output path: no parent directory".into())
            }
        }
    }
}

#[derive(Parser)]
#[command(name = "rzd-cli", about = "RZD-DES-ENGINE simulation runner")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Clone)]
struct BenchmarkProfile {
    name: &'static str,
    scenario: ScenarioConfig,
    warmup_minutes: u64,
    shift_start_offset_min: u64,
    shift_interval_min: u64,
    inject: Option<&'static str>,
    synthetic_manual_violations: Option<SyntheticManualViolationBurst>,
}

#[derive(Clone, Copy, Serialize)]
struct SyntheticManualViolationBurst {
    count: usize,
    start_minute: u64,
}

#[derive(Clone)]
struct BenchmarkRunResult {
    profile: &'static str,
    scenario: ScenarioConfig,
    warmup_minutes: u64,
    shift_start_offset_min: u64,
    shift_interval_min: u64,
    injection: Option<&'static str>,
    synthetic_manual_violations: Option<usize>,
    roads: usize,
    stations: usize,
    segments: usize,
    legs: usize,
    elapsed_ms: f64,
    events_processed: u64,
    events_per_second: f64,
    violations_total: usize,
    violations_by_type: BTreeMap<String, u32>,
    violations_by_severity: BTreeMap<String, u32>,
    shoulder_metrics: BTreeMap<String, ShoulderMetrics>,
    shifts_completed: u64,
    distance_km: f64,
    tu3_rows: usize,
    event_errors: u64,
    exports: BenchmarkExportMetrics,
    business_signature: BenchmarkBusinessSignature,
    business_signature_json: String,
    trace_signature: String,
}

struct BuildEngineInputs {
    network: PathBuf,
    scenario: Option<PathBuf>,
    crews: usize,
    locos: usize,
    days: u32,
    seed: u32,
    warmup: u64,
    inject: Option<PathBuf>,
}

struct BenchmarkExecutionArtifacts {
    result: BenchmarkRunResult,
    tu3_tsv: String,
    ced_json: String,
    violations_ndjson: String,
    event_trace_ndjson: String,
}

struct ScalePacketPaths {
    root: PathBuf,
    competition_bundle_dir: PathBuf,
    rzd_scale_bundle_dir: PathBuf,
    comparison_dir: PathBuf,
    leaderboard_dir: PathBuf,
}

#[derive(Serialize)]
struct ScalePacketManifest {
    schema: &'static str,
    packet_id: String,
    generated_at_unix_seconds: u64,
    network_path: String,
    packet_root: String,
    competition_bundle_dir: String,
    rzd_scale_bundle_dir: String,
    comparison_dir: String,
    leaderboard_dir: String,
    key_files: Vec<BenchmarkBundleFile>,
    competition: ScalePacketProfileSummary,
    rzd_scale: ScalePacketProfileSummary,
    comparison: ScalePacketComparisonSummary,
    leaderboard: ScalePacketLeaderboardSummary,
}

#[derive(Serialize)]
struct ScalePacketProfileSummary {
    profile: String,
    events_processed: u64,
    violations_total: usize,
    shifts_completed: u64,
    distance_km: f64,
    elapsed_ms: f64,
    events_per_second: f64,
    tu3_rows: usize,
    trace_signature: String,
}

#[derive(Serialize)]
struct ScalePacketComparisonSummary {
    comparison_class: String,
    trace_divergence_class: String,
    events_processed_delta: i64,
    violations_total_delta: i64,
    shifts_completed_delta: i64,
    distance_km_delta: f64,
    elapsed_ms_delta: f64,
    events_per_second_delta: f64,
    shoulder_drift_shoulders: usize,
}

#[derive(Serialize)]
struct ScalePacketLeaderboardSummary {
    comparisons_aggregated: usize,
    best_comparison_class: Option<String>,
}

#[derive(Serialize)]
struct ScalePacketSummaryRecord {
    schema: &'static str,
    packet_id: String,
    generated_at_unix_seconds: u64,
    network_path: String,
    competition_profile: String,
    competition_events_processed: u64,
    competition_violations_total: usize,
    competition_elapsed_ms: f64,
    rzd_scale_profile: String,
    rzd_scale_events_processed: u64,
    rzd_scale_violations_total: usize,
    rzd_scale_elapsed_ms: f64,
    comparison_class: String,
    trace_divergence_class: String,
    events_processed_delta: i64,
    violations_total_delta: i64,
    shoulder_drift_shoulders: usize,
}

#[derive(Debug, Clone, Serialize, serde::Deserialize)]
struct BenchmarkExportArtifact {
    bytes: usize,
    lines: usize,
    render_ms: f64,
}

impl Default for BenchmarkExportArtifact {
    fn default() -> Self {
        Self {
            bytes: 0,
            lines: 0,
            render_ms: 0.0,
        }
    }
}

#[derive(Debug, Clone, Serialize, serde::Deserialize)]
struct BenchmarkExportMetrics {
    tu3_tsv: BenchmarkExportArtifact,
    ced_json: BenchmarkExportArtifact,
    violations_ndjson: BenchmarkExportArtifact,
    #[serde(default)]
    event_trace_ndjson: BenchmarkExportArtifact,
}

#[derive(Debug, Serialize, serde::Deserialize)]
struct BenchmarkBundleManifest {
    schema: String,
    profile: String,
    network_path: String,
    files: Vec<BenchmarkBundleFile>,
}

struct BenchmarkBundleSummaryContext<'a> {
    manifest: &'a BenchmarkBundleManifest,
    result: &'a BenchmarkRunResult,
}

#[derive(Debug, Serialize, serde::Deserialize)]
struct BenchmarkBundleFile {
    name: String,
    sha256: String,
    bytes: usize,
    lines: usize,
}

#[derive(Debug, Clone, Serialize, serde::Deserialize)]
struct BenchmarkReport {
    profile: String,
    network: BenchmarkReportNetwork,
    scenario: BenchmarkReportScenario,
    results: BenchmarkReportResults,
}

#[derive(Debug, Clone, Serialize, serde::Deserialize)]
struct BenchmarkReportNetwork {
    path: String,
    roads: usize,
    stations: usize,
    segments: usize,
    legs: usize,
}

#[derive(Debug, Clone, Serialize, serde::Deserialize)]
struct BenchmarkReportScenario {
    crews: usize,
    locos: usize,
    days: u32,
    seed: u32,
    warmup_minutes: u64,
    shift_start_offset_min: u64,
    shift_interval_min: u64,
    injection: Option<String>,
    synthetic_manual_violations: Option<usize>,
}

#[derive(Debug, Clone, Serialize, serde::Deserialize)]
struct BenchmarkReportResults {
    elapsed_ms: f64,
    events_processed: u64,
    events_per_second: f64,
    violations_total: usize,
    violations_by_type: BTreeMap<String, u32>,
    violations_by_severity: BTreeMap<String, u32>,
    #[serde(default)]
    shoulder_metrics: BTreeMap<String, ShoulderMetrics>,
    shifts_completed: u64,
    distance_km: f64,
    tu3_rows: usize,
    event_errors: u64,
    #[serde(default)]
    trace_signature: String,
    exports: BenchmarkExportMetrics,
}

#[derive(Debug, Clone, Serialize, serde::Deserialize)]
struct BundleComparisonReport {
    schema: String,
    baseline_dir: String,
    current_dir: String,
    baseline_profile: String,
    current_profile: String,
    #[serde(default)]
    business_outputs_match: bool,
    #[serde(default)]
    file_hashes_match: bool,
    #[serde(default)]
    artifact_outputs_match: bool,
    #[serde(default)]
    trace_outputs_match: bool,
    #[serde(default)]
    trace_signatures_match: bool,
    #[serde(default)]
    trace_event_count_delta: i64,
    #[serde(default)]
    trace_divergence_class: String,
    #[serde(default)]
    first_trace_divergence: Option<TraceDivergence>,
    #[serde(default)]
    timing_only_drift: bool,
    #[serde(default)]
    comparison_class: String,
    metric_deltas: BundleComparisonMetricDeltas,
    file_deltas: Vec<BundleFileDelta>,
}

#[derive(Debug, Clone, Serialize, serde::Deserialize)]
struct BundleComparisonMetricDeltas {
    events_processed_delta: i64,
    violations_total_delta: i64,
    shifts_completed_delta: i64,
    distance_km_delta: f64,
    elapsed_ms_delta: f64,
    events_per_second_delta: f64,
    #[serde(default)]
    shoulder_deltas: BTreeMap<String, ShoulderMetricDelta>,
}

#[derive(Debug, Clone, Serialize, serde::Deserialize)]
struct BundleFileDelta {
    name: String,
    baseline_bytes: usize,
    current_bytes: usize,
    baseline_lines: usize,
    current_lines: usize,
    same_sha256: bool,
}

#[derive(Debug, Clone, Serialize, serde::Deserialize)]
struct TraceDivergence {
    index: usize,
    baseline_time: Option<u64>,
    current_time: Option<u64>,
    baseline_kind: Option<String>,
    current_kind: Option<String>,
    baseline_crew_id: Option<String>,
    current_crew_id: Option<String>,
    baseline_loco_id: Option<String>,
    current_loco_id: Option<String>,
    baseline_segment_id: Option<String>,
    current_segment_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, serde::Deserialize)]
struct TraceComparisonReport {
    schema: String,
    baseline_trace: String,
    current_trace: String,
    baseline_trace_signature: String,
    current_trace_signature: String,
    trace_signatures_match: bool,
    baseline_event_count: usize,
    current_event_count: usize,
    trace_event_count_delta: i64,
    trace_divergence_class: String,
    first_trace_divergence: Option<TraceDivergence>,
}

#[derive(Serialize)]
struct ComparisonLeaderboardReport {
    schema: &'static str,
    comparisons: Vec<ComparisonLeaderboardEntry>,
}

#[derive(Serialize)]
struct ComparisonLeaderboardEntry {
    label: String,
    comparison_class: String,
    trace_divergence_class: String,
    shoulder_drift_shoulders: usize,
    business_outputs_match: bool,
    artifact_outputs_match: bool,
    trace_outputs_match: bool,
    trace_signatures_match: bool,
    file_hashes_match: bool,
    timing_only_drift: bool,
    elapsed_ms_delta: f64,
    events_per_second_delta: f64,
}

#[derive(Debug, Clone, Default, Serialize, serde::Deserialize, PartialEq, Eq)]
struct ShoulderMetrics {
    events_processed: u64,
    violations_total: usize,
    unique_crews: usize,
    unique_locos: usize,
    segments_touched: usize,
    first_event_minute: Option<u64>,
    last_event_minute: Option<u64>,
}

#[derive(Debug, Clone, Default, Serialize, serde::Deserialize, PartialEq, Eq)]
struct ShoulderMetricDelta {
    events_processed_delta: i64,
    violations_total_delta: i64,
    unique_crews_delta: i64,
    unique_locos_delta: i64,
    segments_touched_delta: i64,
    first_event_minute_delta: Option<i64>,
    last_event_minute_delta: Option<i64>,
}

#[derive(Default)]
struct ShoulderMetricsAccumulator {
    events_processed: u64,
    violations_total: usize,
    crews: BTreeSet<String>,
    locos: BTreeSet<String>,
    segments: BTreeSet<String>,
    first_event_minute: Option<u64>,
    last_event_minute: Option<u64>,
}

#[derive(Clone, Serialize)]
struct BenchmarkBusinessSignature {
    events_processed: u64,
    violations_total: usize,
    violations_by_type: BTreeMap<String, u32>,
    violations_by_severity: BTreeMap<String, u32>,
    shoulder_metrics: BTreeMap<String, ShoulderMetrics>,
    shifts_completed: u64,
    distance_km: String,
    tu3_rows: usize,
    event_errors: u64,
}

impl BenchmarkProfile {
    fn named(name: &str) -> Result<Self, String> {
        match name {
            "one-road-demo" => Ok(Self {
                name: "one-road-demo",
                scenario: ScenarioConfig {
                    crew_count: 12,
                    loco_count: 12,
                    seed: 42,
                    duration_days: 2,
                    ..Default::default()
                },
                warmup_minutes: 0,
                shift_start_offset_min: 60,
                shift_interval_min: 10,
                inject: Some("data/injection/sample_fault_windows.json"),
                synthetic_manual_violations: None,
            }),
            "one-road-month" => Ok(Self {
                name: "one-road-month",
                scenario: ScenarioConfig {
                    crew_count: 180,
                    loco_count: 90,
                    seed: 20260708,
                    duration_days: 30,
                    ..Default::default()
                },
                warmup_minutes: 0,
                shift_start_offset_min: 60,
                shift_interval_min: 10,
                inject: None,
                synthetic_manual_violations: None,
            }),
            "one-road-stress" => Ok(Self {
                name: "one-road-stress",
                scenario: ScenarioConfig {
                    crew_count: 720,
                    loco_count: 360,
                    seed: 20260708,
                    duration_days: 30,
                    ..Default::default()
                },
                warmup_minutes: 0,
                shift_start_offset_min: 30,
                shift_interval_min: 2,
                inject: None,
                synthetic_manual_violations: None,
            }),
            "one-road-shoulder-a" => Ok(Self {
                name: "one-road-shoulder-a",
                scenario: ScenarioConfig {
                    crew_count: 60,
                    loco_count: 30,
                    seed: 20260708,
                    duration_days: 30,
                    allowed_depot_ids: vec!["SD-1".into(), "SD-2".into(), "TD-72".into()],
                    allowed_leg_ids: vec!["LEG-01".into(), "LEG-02".into(), "LEG-03".into()],
                    ..Default::default()
                },
                warmup_minutes: 0,
                shift_start_offset_min: 45,
                shift_interval_min: 8,
                inject: None,
                synthetic_manual_violations: Some(SyntheticManualViolationBurst {
                    count: 5_000,
                    start_minute: 90,
                }),
            }),
            "one-road-shoulder-b" => Ok(Self {
                name: "one-road-shoulder-b",
                scenario: ScenarioConfig {
                    crew_count: 80,
                    loco_count: 40,
                    seed: 20260708,
                    duration_days: 30,
                    allowed_depot_ids: vec![
                        "SD-3".into(),
                        "SD-5".into(),
                        "TD-74".into(),
                        "SD-7".into(),
                    ],
                    allowed_leg_ids: vec![
                        "LEG-04".into(),
                        "LEG-05".into(),
                        "LEG-06".into(),
                        "LEG-10".into(),
                    ],
                    ..Default::default()
                },
                warmup_minutes: 0,
                shift_start_offset_min: 30,
                shift_interval_min: 6,
                inject: None,
                synthetic_manual_violations: Some(SyntheticManualViolationBurst {
                    count: 7_500,
                    start_minute: 120,
                }),
            }),
            "one-road-shoulder-c" => Ok(Self {
                name: "one-road-shoulder-c",
                scenario: ScenarioConfig {
                    crew_count: 70,
                    loco_count: 35,
                    seed: 20260708,
                    duration_days: 30,
                    allowed_depot_ids: vec!["TD-71".into(), "TD-75".into(), "TD-77".into()],
                    allowed_leg_ids: vec!["LEG-07".into(), "LEG-08".into(), "LEG-09".into()],
                    ..Default::default()
                },
                warmup_minutes: 0,
                shift_start_offset_min: 20,
                shift_interval_min: 5,
                inject: None,
                synthetic_manual_violations: Some(SyntheticManualViolationBurst {
                    count: 9_000,
                    start_minute: 150,
                }),
            }),
            "one-road-competition" => Ok(Self {
                name: "one-road-competition",
                scenario: ScenarioConfig {
                    crew_count: 1500,
                    loco_count: 750,
                    seed: 20260708,
                    duration_days: 30,
                    ..Default::default()
                },
                warmup_minutes: 0,
                shift_start_offset_min: 15,
                shift_interval_min: 1,
                inject: None,
                synthetic_manual_violations: Some(SyntheticManualViolationBurst {
                    count: 500_000,
                    start_minute: 90,
                }),
            }),
            "one-road-rzd-scale" => Ok(Self {
                name: "one-road-rzd-scale",
                scenario: ScenarioConfig {
                    crew_count: 2200,
                    loco_count: 1100,
                    seed: 20260708,
                    duration_days: 30,
                    ..Default::default()
                },
                warmup_minutes: 0,
                shift_start_offset_min: 10,
                shift_interval_min: 1,
                inject: None,
                synthetic_manual_violations: Some(SyntheticManualViolationBurst {
                    count: 750_000,
                    start_minute: 90,
                }),
            }),
            other => Err(format!(
                "Unknown benchmark profile: {}. Use one-road-demo, one-road-month, one-road-stress, one-road-shoulder-a, one-road-shoulder-b, one-road-shoulder-c, one-road-competition, or one-road-rzd-scale",
                other
            )),
        }
    }

    fn names_from_arg(arg: &str) -> Result<Vec<&'static str>, String> {
        if arg == "all" {
            return Ok(vec!["one-road-demo", "one-road-month", "one-road-stress"]);
        }

        let mut names = Vec::new();
        for raw in arg.split(',') {
            let name = raw.trim();
            match name {
                "one-road-demo" => names.push("one-road-demo"),
                "one-road-month" => names.push("one-road-month"),
                "one-road-stress" => names.push("one-road-stress"),
                "one-road-shoulder-a" => names.push("one-road-shoulder-a"),
                "one-road-shoulder-b" => names.push("one-road-shoulder-b"),
                "one-road-shoulder-c" => names.push("one-road-shoulder-c"),
                "one-road-competition" => names.push("one-road-competition"),
                "one-road-rzd-scale" => names.push("one-road-rzd-scale"),
                other => {
                    return Err(format!(
                        "Unknown benchmark profile in suite: {}. Use one-road-demo, one-road-month, one-road-stress, one-road-shoulder-a, one-road-shoulder-b, one-road-shoulder-c, one-road-competition, one-road-rzd-scale, or all",
                        other
                    ))
                }
            }
        }
        Ok(names)
    }
}


fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = commands::parse();

    match cli.command {
        Commands::Run {
            network,
            scenario,
            crews,
            locos,
            days,
            seed,
            warmup,
            inject,
            output,
            format,
        } => {
            let (mut engine, scenario_config) = build_engine_from_inputs(BuildEngineInputs {
                network,
                scenario,
                crews,
                locos,
                days,
                seed,
                warmup,
                inject,
            })?;
            let until = (days as u64) * 1440;
            let estimated_events = (crews as u64 * locos as u64 * days as u64 * 100).max(1000);
            let progress = ProgressTracker::new("Simulation", estimated_events);
            engine.run_with_progress(until, |current, _total, _elapsed| {
                progress.update(current);
            });
            let final_count = engine.stats().total_events_processed;
            progress.update(final_count);
            progress.finish();
            let result = if format == "bundle" {
                let ced = engine.export_ced();
                let violation_summary = engine.violations_summary();
                let bundle = build_replay_bundle(ReplayBundleInputs {
                    network: &engine.network,
                    scenario: &scenario_config,
                    seed,
                    until_minutes: until,
                    warmup_minutes: warmup,
                    stats: engine.stats(),
                    violation_summary: &violation_summary,
                    tu3_rows: &engine.tu3_rows,
                    ced: &ced,
                });
                to_canonical_json(&bundle)?
            } else {
                render_engine_output(&engine, &format, seed, warmup, days)?
            };

            if let Some(out_path) = output {
                let safe_path = sanitize_output_path(&out_path)?;
                std::fs::write(&safe_path, &result)?;
                eprintln!("Written to {}", safe_path.display());
            } else {
                println!("{}", result);
            }

            eprintln!(
                "Simulation complete: {} events, {} shifts, {} violations, {} crews, {} locos",
                engine.stats().total_events_processed,
                engine.stats().total_shifts_completed,
                engine.stats().total_violations,
                engine.crew_count(),
                engine.loco_count(),
            );
        }

        Commands::Generate {
            network,
            crews,
            locos,
            days,
            seed,
            output,
        } => {
            let network_json = std::fs::read_to_string(&network)?;
            let net = Network::from_json(&network_json)?;

            let config = ScenarioConfig {
                crew_count: crews,
                loco_count: locos,
                seed,
                duration_days: days,
                ..Default::default()
            };
            let scenario = generate_scenario(&net, config);

            let json = serde_json::to_string_pretty(&scenario)?;

            if let Some(out_path) = output {
                let safe_path = sanitize_output_path(&out_path)?;
                std::fs::write(&safe_path, &json)?;
                eprintln!("Written to {}", safe_path.display());
            } else {
                println!("{}", json);
            }

            eprintln!(
                "Generated: {} crews, {} locos",
                scenario.crews.len(),
                scenario.locos.len()
            );
        }

        Commands::Validate { network } => {
            let network_json = std::fs::read_to_string(&network)?;
            match Network::from_json(&network_json) {
                Ok(net) => {
                    let errors = net.validate();
                    if errors.is_empty() {
                        println!(
                            "Network is valid: {} stations, {} segments, {} legs, {} roads",
                            net.stations.len(),
                            net.segments.len(),
                            net.legs.len(),
                            net.roads.len()
                        );
                    } else {
                        eprintln!("Validation errors:");
                        for err in &errors {
                            eprintln!("  - {}", err);
                        }
                        std::process::exit(1);
                    }
                }
                Err(e) => {
                    eprintln!("Invalid network JSON: {}", e);
                    std::process::exit(1);
                }
            }
        }

        Commands::Benchmark {
            network,
            profile,
            output,
        } => {
            let result = run_benchmark_profile(&network, &BenchmarkProfile::named(&profile)?)?;
            let report = build_benchmark_report_json(&result, &network);

            let rendered = serde_json::to_string_pretty(&report)?;
            if let Some(out_path) = output {
                let safe_path = sanitize_output_path(&out_path)?;
                std::fs::write(&safe_path, &rendered)?;
                eprintln!("Written benchmark report to {}", safe_path.display());
            } else {
                println!("{}", rendered);
            }
        }

        Commands::BenchmarkBundle {
            network,
            profile,
            output_dir,
        } => {
            let execution =
                write_benchmark_bundle(&network, &BenchmarkProfile::named(&profile)?, &output_dir)?;

            eprintln!(
                "Benchmark bundle written to {} (profile {}, {} files)",
                output_dir.display(),
                execution.result.profile,
                9
            );
        }

        Commands::BenchmarkSuite {
            network,
            profiles,
            repeats,
            output,
        } => {
            let profile_names = BenchmarkProfile::names_from_arg(&profiles)?;
            let mut suite_profiles = Vec::new();
            for profile_name in profile_names {
                let profile = BenchmarkProfile::named(profile_name)?;
                let mut runs = Vec::new();
                let mut elapsed_samples = Vec::new();
                let mut eps_samples = Vec::new();
                let mut business_signatures = Vec::new();

                for _ in 0..repeats {
                    let result = run_benchmark_profile(&network, &profile)?;
                    elapsed_samples.push(result.elapsed_ms);
                    eps_samples.push(result.events_per_second);
                    business_signatures.push(result.business_signature_json.clone());
                    runs.push(serde_json::json!({
                        "elapsed_ms": result.elapsed_ms,
                        "events_processed": result.events_processed,
                        "events_per_second": result.events_per_second,
                        "violations_total": result.violations_total,
                        "violations_by_type": result.violations_by_type,
                        "violations_by_severity": result.violations_by_severity,
                        "shifts_completed": result.shifts_completed,
                        "distance_km": result.distance_km,
                        "tu3_rows": result.tu3_rows,
                        "event_errors": result.event_errors,
                        "exports": result.exports,
                        "business_signature": result.business_signature
                    }));
                }

                elapsed_samples.sort_by(|a, b| a.partial_cmp(b).unwrap());
                eps_samples.sort_by(|a, b| a.partial_cmp(b).unwrap());
                business_signatures.sort();
                business_signatures.dedup();
                let elapsed_mean =
                    elapsed_samples.iter().sum::<f64>() / elapsed_samples.len() as f64;
                let eps_mean = eps_samples.iter().sum::<f64>() / eps_samples.len() as f64;
                let p50_idx = elapsed_samples.len() / 2;
                let p90_idx = ((elapsed_samples.len() - 1) as f64 * 0.9).round() as usize;

                suite_profiles.push(serde_json::json!({
                    "profile": profile.name,
                    "scenario": {
                        "crews": profile.scenario.crew_count,
                        "locos": profile.scenario.loco_count,
                        "days": profile.scenario.duration_days,
                        "seed": profile.scenario.seed,
                        "warmup_minutes": profile.warmup_minutes,
                        "shift_start_offset_min": profile.shift_start_offset_min,
                        "shift_interval_min": profile.shift_interval_min,
                        "injection": profile.inject,
                        "synthetic_manual_violations": profile.synthetic_manual_violations.map(|v| v.count)
                    },
                    "aggregates": {
                        "repeats": repeats,
                        "elapsed_ms_mean": elapsed_mean,
                        "elapsed_ms_min": elapsed_samples.first().copied().unwrap_or(0.0),
                        "elapsed_ms_p50": elapsed_samples.get(p50_idx).copied().unwrap_or(0.0),
                        "elapsed_ms_p90": elapsed_samples.get(p90_idx).copied().unwrap_or(0.0),
                        "elapsed_ms_max": elapsed_samples.last().copied().unwrap_or(0.0),
                        "events_per_second_mean": eps_mean,
                        "events_per_second_min": eps_samples.first().copied().unwrap_or(0.0),
                        "events_per_second_p50": eps_samples.get(p50_idx).copied().unwrap_or(0.0),
                        "events_per_second_p90": eps_samples.get(p90_idx).copied().unwrap_or(0.0),
                        "events_per_second_max": eps_samples.last().copied().unwrap_or(0.0)
                    },
                    "determinism": {
                        "unique_business_signatures": business_signatures.len(),
                        "business_outputs_stable_across_repeats": business_signatures.len() == 1
                    },
                    "runs": runs
                }));
            }

            let report = serde_json::json!({
                "schema": "rzd-des-benchmark-suite/v1",
                "network": {
                    "path": network.display().to_string()
                },
                "profiles": suite_profiles
            });
            let rendered = serde_json::to_string_pretty(&report)?;
            if let Some(out_path) = output {
                let safe_path = sanitize_output_path(&out_path)?;
                std::fs::write(&safe_path, &rendered)?;
                eprintln!("Written benchmark suite report to {}", safe_path.display());
            } else {
                println!("{}", rendered);
            }
        }

        Commands::CompareBundles {
            baseline_dir,
            current_dir,
            output_dir,
        } => {
            let _comparison = write_bundle_comparison(&baseline_dir, &current_dir, &output_dir)?;

            eprintln!(
                "Bundle comparison written to {} (baseline {} vs current {})",
                output_dir.display(),
                baseline_dir.display(),
                current_dir.display()
            );
        }

        Commands::CompareTraces {
            baseline_trace,
            current_trace,
            output_dir,
        } => {
            let comparison = build_trace_comparison_report(&baseline_trace, &current_trace)?;
            std::fs::create_dir_all(&output_dir)?;
            let safe_output_dir = sanitize_output_path(&output_dir)?;
            std::fs::write(
                safe_output_dir.join("trace_comparison.json"),
                serde_json::to_string_pretty(&comparison)?,
            )?;
            std::fs::write(
                safe_output_dir.join("trace_comparison.md"),
                build_trace_comparison_markdown(&comparison),
            )?;
            std::fs::write(
                safe_output_dir.join("trace_comparison.html"),
                build_trace_comparison_html(&comparison),
            )?;

            eprintln!(
                "Trace comparison written to {} (baseline {} vs current {})",
                output_dir.display(),
                baseline_trace.display(),
                current_trace.display()
            );
        }

        Commands::ComparisonLeaderboard {
            input_dir,
            output_dir,
        } => {
            let leaderboard = write_comparison_leaderboard(&input_dir, &output_dir)?;

            eprintln!(
                "Comparison leaderboard written to {} ({} comparisons)",
                output_dir.display(),
                leaderboard.comparisons.len()
            );
        }

        Commands::ScalePacket {
            network,
            output_dir,
        } => {
            let paths = scale_packet_paths(&output_dir);
            std::fs::create_dir_all(&paths.root)?;

            let competition = write_benchmark_bundle(
                &network,
                &BenchmarkProfile::named("one-road-competition")?,
                &paths.competition_bundle_dir,
            )?;
            let rzd_scale = write_benchmark_bundle(
                &network,
                &BenchmarkProfile::named("one-road-rzd-scale")?,
                &paths.rzd_scale_bundle_dir,
            )?;
            let comparison = write_bundle_comparison(
                &paths.competition_bundle_dir,
                &paths.rzd_scale_bundle_dir,
                &paths.comparison_dir,
            )?;
            let leaderboard =
                write_comparison_leaderboard(&paths.comparison_dir, &paths.leaderboard_dir)?;
            let scale_packet_md = build_scale_packet_readme(
                &network,
                &competition,
                &rzd_scale,
                &comparison,
                &leaderboard,
                &paths,
            );
            let safe_root = sanitize_output_path(&paths.root)?;
            std::fs::write(safe_root.join("SCALE_PACKET.md"), &scale_packet_md)?;
            let key_files = vec![
                build_file_descriptor_from_path(&safe_root, &safe_root.join("SCALE_PACKET.md"))?,
                build_file_descriptor_from_path(
                    &safe_root,
                    &paths.comparison_dir.join("comparison.json"),
                )?,
                build_file_descriptor_from_path(
                    &safe_root,
                    &paths.leaderboard_dir.join("leaderboard.json"),
                )?,
                build_file_descriptor_from_path(
                    &safe_root,
                    &paths.competition_bundle_dir.join("manifest.json"),
                )?,
                build_file_descriptor_from_path(
                    &safe_root,
                    &paths.rzd_scale_bundle_dir.join("manifest.json"),
                )?,
            ];
            let manifest = build_scale_packet_manifest(
                &network,
                &competition,
                &rzd_scale,
                &comparison,
                &leaderboard,
                key_files,
                &paths,
            )?;
            std::fs::write(
                safe_root.join("MANIFEST_SCALE.json"),
                serde_json::to_string_pretty(&manifest)?,
            )?;
            let summary_record = build_scale_packet_summary_record(&manifest);
            std::fs::write(
                safe_root.join("packet_summary.json"),
                serde_json::to_string_pretty(&summary_record)?,
            )?;
            std::fs::write(
                safe_root.join("packet_summary.tsv"),
                build_scale_packet_summary_tsv(&summary_record),
            )?;
            std::fs::write(
                safe_root.join("SCALE_PACKET.md"),
                build_scale_packet_readme(
                    &network,
                    &competition,
                    &rzd_scale,
                    &comparison,
                    &leaderboard,
                    &paths,
                ),
            )?;
            std::fs::write(
                safe_root.join("SCALE_PACKET.html"),
                build_scale_packet_html(
                    &network,
                    &manifest,
                    &competition,
                    &rzd_scale,
                    &comparison,
                    &leaderboard,
                ),
            )?;

            eprintln!(
                "Scale packet written to {} (competition, rzd-scale, comparison, leaderboard)",
                paths.root.display()
            );
        }

        Commands::Checkpoint {
            network,
            scenario,
            crews,
            locos,
            days,
            seed,
            warmup,
            inject,
            checkpoint_minute,
            output,
        } => {
            let (mut engine, _scenario_config) = build_engine_from_inputs(BuildEngineInputs {
                network,
                scenario,
                crews,
                locos,
                days,
                seed,
                warmup,
                inject,
            })?;
            engine.run(checkpoint_minute);
            let snapshot = to_canonical_json(&engine.snapshot())?;
            let safe_output = sanitize_output_path(&output)?;
            std::fs::write(&safe_output, snapshot)?;
            eprintln!(
                "Checkpoint written to {} at minute {} ({} events, {} shifts)",
                safe_output.display(),
                checkpoint_minute,
                engine.stats().total_events_processed,
                engine.stats().total_shifts_completed
            );
        }

        Commands::Resume {
            snapshot,
            until_minute,
            output,
            format,
            snapshot_output,
        } => {
            let snapshot_json = std::fs::read_to_string(&snapshot).map_err(|e| {
                format!(
                    "Failed to read snapshot file '{}': {}",
                    snapshot.display(),
                    e
                )
            })?;
            let snapshot_state: EngineSnapshot = serde_json::from_str(&snapshot_json)?;
            let warmup = snapshot_state.warmup_minutes;
            let seed = snapshot_state.prng.state();
            let duration_days = until_minute.div_ceil(1440) as u32;
            let mut engine = Engine::from_snapshot(snapshot_state)?;
            engine.run(until_minute);

            let result = render_engine_output(&engine, &format, seed, warmup, duration_days)?;

            if let Some(out_path) = output {
                let safe_path = sanitize_output_path(&out_path)?;
                std::fs::write(&safe_path, result)?;
                eprintln!("Written to {}", safe_path.display());
            } else {
                println!("{}", result);
            }

            if let Some(snapshot_path) = snapshot_output {
                let safe_snapshot = sanitize_output_path(&snapshot_path)?;
                std::fs::write(safe_snapshot, to_canonical_json(&engine.snapshot())?)?;
            }

            eprintln!(
                "Resume complete: {} events, {} shifts, {} violations, {} crews, {} locos",
                engine.stats().total_events_processed,
                engine.stats().total_shifts_completed,
                engine.stats().total_violations,
                engine.crew_count(),
                engine.loco_count(),
            );
        }

        Commands::GenerateMm {
            network,
            scenario,
            crews,
            locos,
            days,
            seed,
            base_date,
            output,
        } => {
            let start = Instant::now();
            let (mut engine, config) = build_engine_from_inputs(BuildEngineInputs {
                network,
                scenario,
                crews,
                locos,
                days,
                seed,
                warmup: 0,
                inject: None,
            })?;

            engine.schedule_all_shifts(0, 30);
            engine.run((days as u64) * 1440);

            let mut exporter = rzd_core::mm_detalization::MmDetalizationExporter::new(seed);
            exporter
                .export_to_xlsx(&engine, &output, &base_date)
                .map_err(|e| format!("XLSX export error: {}", e))?;

            let elapsed = start.elapsed();
            eprintln!(
                "GenerateMm: {} tu3 rows → {} | crews={} locos={} days={} seed={} ({:.1}s)",
                engine.tu3_rows.len(),
                output.display(),
                config.crew_count,
                config.loco_count,
                config.duration_days,
                seed,
                elapsed.as_secs_f64(),
            );
        }
    }

    Ok(())
}

fn build_engine_from_inputs(
    input: BuildEngineInputs,
) -> Result<(Engine, ScenarioConfig), Box<dyn std::error::Error>> {
    let network_json = std::fs::read_to_string(&input.network).map_err(|e| {
        format!(
            "Failed to read network file '{}': {}",
            input.network.display(),
            e
        )
    })?;
    let net = Network::from_json(&network_json)?;
    let mut engine = Engine::new(net, input.seed);

    let scenario_config = if let Some(scenario_path) = input.scenario {
        let scenario_json = std::fs::read_to_string(&scenario_path).map_err(|e| {
            format!(
                "Failed to read scenario file '{}': {}",
                scenario_path.display(),
                e
            )
        })?;
        let config = ScenarioDocument::from_json(&scenario_json)
            .map_err(|e| {
                format!(
                    "Failed to parse scenario file '{}': {}",
                    scenario_path.display(),
                    e
                )
            })?
            .config;
        let gen = generate_scenario(&engine.network, config.clone());
        for c in gen.crews {
            engine.add_crew(c);
        }
        for l in gen.locos {
            engine.add_loco(l);
        }
        config
    } else {
        let config = ScenarioConfig {
            crew_count: input.crews,
            loco_count: input.locos,
            seed: input.seed,
            duration_days: input.days,
            ..Default::default()
        };
        let gen = generate_scenario(&engine.network, config.clone());
        for c in gen.crews {
            engine.add_crew(c);
        }
        for l in gen.locos {
            engine.add_loco(l);
        }
        config
    };

    engine.set_warmup(input.warmup);
    if let Some(inject_path) = input.inject {
        let inject_json = std::fs::read_to_string(&inject_path).map_err(|e| {
            format!(
                "Failed to read injection file '{}': {}",
                inject_path.display(),
                e
            )
        })?;
        let plan: InjectionPlan = serde_json::from_str(&inject_json)?;
        engine.apply_injection_plan(&plan)?;
    }
    engine.schedule_all_shifts(60, 10);

    Ok((engine, scenario_config))
}

fn write_benchmark_bundle(
    network: &Path,
    profile: &BenchmarkProfile,
    output_dir: &Path,
) -> Result<BenchmarkExecutionArtifacts, Box<dyn std::error::Error>> {
    let execution = run_benchmark_profile_with_artifacts(&network.to_path_buf(), profile)?;
    std::fs::create_dir_all(output_dir)?;
    let safe_output_dir = sanitize_output_path(output_dir)?;

    let report_json =
        serde_json::to_string_pretty(&build_benchmark_report_json(&execution.result, network))?;
    let report_path = safe_output_dir.join("report.json");
    let tu3_path = safe_output_dir.join("tu3.tsv");
    let ced_path = safe_output_dir.join("ced.json");
    let violations_path = safe_output_dir.join("violations.ndjson");
    let event_trace_path = safe_output_dir.join("event_trace.ndjson");

    std::fs::write(&report_path, &report_json)?;
    std::fs::write(&tu3_path, &execution.tu3_tsv)?;
    std::fs::write(&ced_path, &execution.ced_json)?;
    std::fs::write(&violations_path, &execution.violations_ndjson)?;
    std::fs::write(&event_trace_path, &execution.event_trace_ndjson)?;

    let manifest = BenchmarkBundleManifest {
        schema: "rzd-des-benchmark-bundle/v1".to_string(),
        profile: execution.result.profile.to_string(),
        network_path: network.display().to_string(),
        files: vec![
            build_bundle_file("report.json", &report_json),
            build_bundle_file("tu3.tsv", &execution.tu3_tsv),
            build_bundle_file("ced.json", &execution.ced_json),
            build_bundle_file("violations.ndjson", &execution.violations_ndjson),
            build_bundle_file("event_trace.ndjson", &execution.event_trace_ndjson),
        ],
    };
    let manifest_json = serde_json::to_string_pretty(&manifest)?;
    std::fs::write(safe_output_dir.join("manifest.json"), &manifest_json)?;
    std::fs::write(
        safe_output_dir.join("SUMMARY.md"),
        build_bundle_summary_markdown(BenchmarkBundleSummaryContext {
            manifest: &manifest,
            result: &execution.result,
        }),
    )?;
    std::fs::write(
        safe_output_dir.join("report.html"),
        build_bundle_report_html(BenchmarkBundleSummaryContext {
            manifest: &manifest,
            result: &execution.result,
        }),
    )?;
    std::fs::write(
        safe_output_dir.join("JUDGING_PACKET.md"),
        build_judging_packet_markdown(BenchmarkBundleSummaryContext {
            manifest: &manifest,
            result: &execution.result,
        }),
    )?;

    Ok(execution)
}

fn write_bundle_comparison(
    baseline_dir: &Path,
    current_dir: &Path,
    output_dir: &Path,
) -> Result<BundleComparisonReport, Box<dyn std::error::Error>> {
    let baseline_report: BenchmarkReport =
        serde_json::from_str(&std::fs::read_to_string(baseline_dir.join("report.json"))?)?;
    let current_report: BenchmarkReport =
        serde_json::from_str(&std::fs::read_to_string(current_dir.join("report.json"))?)?;
    let baseline_manifest: BenchmarkBundleManifest = serde_json::from_str(
        &std::fs::read_to_string(baseline_dir.join("manifest.json"))?,
    )?;
    let current_manifest: BenchmarkBundleManifest =
        serde_json::from_str(&std::fs::read_to_string(current_dir.join("manifest.json"))?)?;

    let comparison = build_bundle_comparison_report(
        baseline_dir,
        current_dir,
        &baseline_report,
        &current_report,
        &baseline_manifest,
        &current_manifest,
    );
    std::fs::create_dir_all(output_dir)?;
    let safe_output_dir = sanitize_output_path(output_dir)?;
    std::fs::write(
        safe_output_dir.join("comparison.json"),
        serde_json::to_string_pretty(&comparison)?,
    )?;
    std::fs::write(
        safe_output_dir.join("comparison.md"),
        build_bundle_comparison_markdown(&comparison),
    )?;
    std::fs::write(
        safe_output_dir.join("comparison.html"),
        build_bundle_comparison_html(&comparison),
    )?;
    Ok(comparison)
}

fn write_comparison_leaderboard(
    input_dir: &Path,
    output_dir: &Path,
) -> Result<ComparisonLeaderboardReport, Box<dyn std::error::Error>> {
    let comparison_paths = find_comparison_json_files(input_dir)?;
    let mut comparisons = Vec::new();
    for path in comparison_paths {
        let content = std::fs::read_to_string(&path)?;
        let comparison: BundleComparisonReport = serde_json::from_str(&content)?;
        comparisons.push(comparison);
    }
    let leaderboard = build_comparison_leaderboard(&comparisons);
    std::fs::create_dir_all(output_dir)?;
    let safe_output_dir = sanitize_output_path(output_dir)?;
    std::fs::write(
        safe_output_dir.join("leaderboard.json"),
        serde_json::to_string_pretty(&leaderboard)?,
    )?;
    std::fs::write(
        safe_output_dir.join("leaderboard.md"),
        build_leaderboard_markdown(&leaderboard),
    )?;
    std::fs::write(
        safe_output_dir.join("leaderboard.html"),
        build_leaderboard_html(&leaderboard),
    )?;
    Ok(leaderboard)
}

fn scale_packet_paths(root: &Path) -> ScalePacketPaths {
    ScalePacketPaths {
        root: root.to_path_buf(),
        competition_bundle_dir: root.join("competition_bundle"),
        rzd_scale_bundle_dir: root.join("rzd_scale_bundle"),
        comparison_dir: root.join("comparison"),
        leaderboard_dir: root.join("leaderboard"),
    }
}

fn current_unix_timestamp_seconds() -> Result<u64, Box<dyn std::error::Error>> {
    Ok(std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_secs())
}

fn build_file_descriptor_from_path(
    base_dir: &Path,
    path: &Path,
) -> Result<BenchmarkBundleFile, Box<dyn std::error::Error>> {
    let content = std::fs::read_to_string(path)?;
    let relative_name = path
        .strip_prefix(base_dir)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/");
    Ok(build_bundle_file(&relative_name, &content))
}

fn compute_scale_packet_id(files: &[BenchmarkBundleFile]) -> String {
    let mut parts: Vec<String> = files
        .iter()
        .map(|file| format!("{}:{}", file.name, file.sha256))
        .collect();
    parts.sort();
    sha256_hex(parts.join("|").as_bytes())
}

fn build_scale_packet_manifest(
    network: &Path,
    competition: &BenchmarkExecutionArtifacts,
    rzd_scale: &BenchmarkExecutionArtifacts,
    comparison: &BundleComparisonReport,
    leaderboard: &ComparisonLeaderboardReport,
    key_files: Vec<BenchmarkBundleFile>,
    paths: &ScalePacketPaths,
) -> Result<ScalePacketManifest, Box<dyn std::error::Error>> {
    let packet_id = compute_scale_packet_id(&key_files);
    Ok(ScalePacketManifest {
        schema: "rzd-des-scale-packet/v1",
        packet_id,
        generated_at_unix_seconds: current_unix_timestamp_seconds()?,
        network_path: network.display().to_string(),
        packet_root: paths.root.display().to_string(),
        competition_bundle_dir: paths.competition_bundle_dir.display().to_string(),
        rzd_scale_bundle_dir: paths.rzd_scale_bundle_dir.display().to_string(),
        comparison_dir: paths.comparison_dir.display().to_string(),
        leaderboard_dir: paths.leaderboard_dir.display().to_string(),
        key_files,
        competition: ScalePacketProfileSummary {
            profile: competition.result.profile.to_string(),
            events_processed: competition.result.events_processed,
            violations_total: competition.result.violations_total,
            shifts_completed: competition.result.shifts_completed,
            distance_km: competition.result.distance_km,
            elapsed_ms: competition.result.elapsed_ms,
            events_per_second: competition.result.events_per_second,
            tu3_rows: competition.result.tu3_rows,
            trace_signature: competition.result.trace_signature.clone(),
        },
        rzd_scale: ScalePacketProfileSummary {
            profile: rzd_scale.result.profile.to_string(),
            events_processed: rzd_scale.result.events_processed,
            violations_total: rzd_scale.result.violations_total,
            shifts_completed: rzd_scale.result.shifts_completed,
            distance_km: rzd_scale.result.distance_km,
            elapsed_ms: rzd_scale.result.elapsed_ms,
            events_per_second: rzd_scale.result.events_per_second,
            tu3_rows: rzd_scale.result.tu3_rows,
            trace_signature: rzd_scale.result.trace_signature.clone(),
        },
        comparison: ScalePacketComparisonSummary {
            comparison_class: comparison.comparison_class.clone(),
            trace_divergence_class: comparison.trace_divergence_class.clone(),
            events_processed_delta: comparison.metric_deltas.events_processed_delta,
            violations_total_delta: comparison.metric_deltas.violations_total_delta,
            shifts_completed_delta: comparison.metric_deltas.shifts_completed_delta,
            distance_km_delta: comparison.metric_deltas.distance_km_delta,
            elapsed_ms_delta: comparison.metric_deltas.elapsed_ms_delta,
            events_per_second_delta: comparison.metric_deltas.events_per_second_delta,
            shoulder_drift_shoulders: count_nonzero_shoulder_drift(
                &comparison.metric_deltas.shoulder_deltas,
            ),
        },
        leaderboard: ScalePacketLeaderboardSummary {
            comparisons_aggregated: leaderboard.comparisons.len(),
            best_comparison_class: leaderboard
                .comparisons
                .first()
                .map(|entry| entry.comparison_class.clone()),
        },
    })
}

fn build_scale_packet_readme(
    network: &Path,
    competition: &BenchmarkExecutionArtifacts,
    rzd_scale: &BenchmarkExecutionArtifacts,
    comparison: &BundleComparisonReport,
    leaderboard: &ComparisonLeaderboardReport,
    paths: &ScalePacketPaths,
) -> String {
    let mut out = String::new();
    out.push_str("# Scale Packet\n\n");
    out.push_str(
        "This packet bundles the current synthetic one-road scale story for external review.\n\n",
    );
    out.push_str(&format!("- Network: `{}`\n", network.display()));
    out.push_str(&format!(
        "- Competition bundle: `{}`\n- RZD-scale bundle: `{}`\n- Comparison: `{}`\n- Leaderboard: `{}`\n\n",
        paths.competition_bundle_dir.display(),
        paths.rzd_scale_bundle_dir.display(),
        paths.comparison_dir.display(),
        paths.leaderboard_dir.display()
    ));
    out.push_str("## Profiles\n\n");
    out.push_str(&format!(
        "- `{}`: `{}` events, `{}` violations, `{:.3}` ms\n",
        competition.result.profile,
        competition.result.events_processed,
        competition.result.violations_total,
        competition.result.elapsed_ms
    ));
    out.push_str(&format!(
        "- `{}`: `{}` events, `{}` violations, `{:.3}` ms\n\n",
        rzd_scale.result.profile,
        rzd_scale.result.events_processed,
        rzd_scale.result.violations_total,
        rzd_scale.result.elapsed_ms
    ));
    out.push_str("## Comparison\n\n");
    out.push_str(&format!(
        "- Comparison class: `{}`\n- Trace divergence class: `{}`\n- Event delta: `{}`\n- Violation delta: `{}`\n- Shoulder drift count: `{}`\n\n",
        comparison.comparison_class,
        comparison.trace_divergence_class,
        comparison.metric_deltas.events_processed_delta,
        comparison.metric_deltas.violations_total_delta,
        count_nonzero_shoulder_drift(&comparison.metric_deltas.shoulder_deltas)
    ));
    out.push_str("## Leaderboard\n\n");
    out.push_str(&format!(
        "- Comparisons aggregated: `{}`\n",
        leaderboard.comparisons.len()
    ));
    out
}

fn build_scale_packet_html(
    network: &Path,
    manifest: &ScalePacketManifest,
    competition: &BenchmarkExecutionArtifacts,
    rzd_scale: &BenchmarkExecutionArtifacts,
    comparison: &BundleComparisonReport,
    leaderboard: &ComparisonLeaderboardReport,
) -> String {
    format!(
        "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width, initial-scale=1\"><title>Scale Packet</title><style>body{{font-family:Segoe UI,Arial,sans-serif;margin:0;background:#0b1020;color:#e8edf7;}}.wrap{{max-width:1180px;margin:0 auto;padding:32px 24px 48px;}}.metric{{background:#131a2e;border:1px solid #26314f;border-radius:8px;padding:14px;margin:12px 0;}}.grid{{display:grid;grid-template-columns:repeat(auto-fit,minmax(280px,1fr));gap:14px;}}a{{color:#8ec5ff;text-decoration:none;}}a:hover{{text-decoration:underline;}}table{{width:100%;border-collapse:collapse;margin-top:12px;background:#131a2e;border:1px solid #26314f;}}th,td{{padding:10px 12px;border-bottom:1px solid #26314f;text-align:left;vertical-align:top;}}th{{color:#9fb0d6;font-size:12px;text-transform:uppercase;}}code{{font-family:Consolas,monospace;}}</style></head><body><div class=\"wrap\"><h1>Scale Packet</h1><div class=\"metric\"><div>Network: <code>{network}</code></div><div>Packet ID: <code>{packet_id}</code></div><div>Generated at (unix): <strong>{generated_at}</strong></div><div>Comparison class: <strong>{comparison_class}</strong></div><div>Trace drift: <strong>{trace_drift}</strong></div><div>Shoulder drift count: <strong>{shoulder_drift}</strong></div></div><div class=\"grid\"><div class=\"metric\"><h2>Competition</h2><div>Profile: <code>{competition_profile}</code></div><div>Events: <strong>{competition_events}</strong></div><div>Violations: <strong>{competition_violations}</strong></div><div>Elapsed: <strong>{competition_elapsed:.3} ms</strong></div><div><a href=\"competition_bundle/report.html\">report.html</a></div><div><a href=\"competition_bundle/SUMMARY.md\">SUMMARY.md</a></div><div><a href=\"competition_bundle/JUDGING_PACKET.md\">JUDGING_PACKET.md</a></div><div><a href=\"competition_bundle/manifest.json\">manifest.json</a></div></div><div class=\"metric\"><h2>RZD Scale</h2><div>Profile: <code>{rzd_profile}</code></div><div>Events: <strong>{rzd_events}</strong></div><div>Violations: <strong>{rzd_violations}</strong></div><div>Elapsed: <strong>{rzd_elapsed:.3} ms</strong></div><div><a href=\"rzd_scale_bundle/report.html\">report.html</a></div><div><a href=\"rzd_scale_bundle/SUMMARY.md\">SUMMARY.md</a></div><div><a href=\"rzd_scale_bundle/JUDGING_PACKET.md\">JUDGING_PACKET.md</a></div><div><a href=\"rzd_scale_bundle/manifest.json\">manifest.json</a></div></div><div class=\"metric\"><h2>Comparison</h2><div>Event delta: <strong>{event_delta}</strong></div><div>Violation delta: <strong>{violation_delta}</strong></div><div>Elapsed delta: <strong>{elapsed_delta:.3} ms</strong></div><div><a href=\"comparison/comparison.html\">comparison.html</a></div><div><a href=\"comparison/comparison.md\">comparison.md</a></div><div><a href=\"comparison/comparison.json\">comparison.json</a></div></div><div class=\"metric\"><h2>Leaderboard</h2><div>Comparisons aggregated: <strong>{leaderboard_count}</strong></div><div><a href=\"leaderboard/leaderboard.html\">leaderboard.html</a></div><div><a href=\"leaderboard/leaderboard.md\">leaderboard.md</a></div><div><a href=\"leaderboard/leaderboard.json\">leaderboard.json</a></div></div></div><h2>Key Files</h2><table><thead><tr><th>File</th><th>Bytes</th><th>Lines</th><th>SHA-256</th></tr></thead><tbody>{key_rows}</tbody></table><div class=\"metric\"><div><a href=\"SCALE_PACKET.md\">SCALE_PACKET.md</a></div><div><a href=\"MANIFEST_SCALE.json\">MANIFEST_SCALE.json</a></div></div></div></body></html>",
        network = escape_html(&network.display().to_string()),
        packet_id = escape_html(&manifest.packet_id),
        generated_at = manifest.generated_at_unix_seconds,
        comparison_class = escape_html(&comparison.comparison_class),
        trace_drift = escape_html(&comparison.trace_divergence_class),
        shoulder_drift = count_nonzero_shoulder_drift(&comparison.metric_deltas.shoulder_deltas),
        competition_profile = escape_html(competition.result.profile),
        competition_events = competition.result.events_processed,
        competition_violations = competition.result.violations_total,
        competition_elapsed = competition.result.elapsed_ms,
        rzd_profile = escape_html(rzd_scale.result.profile),
        rzd_events = rzd_scale.result.events_processed,
        rzd_violations = rzd_scale.result.violations_total,
        rzd_elapsed = rzd_scale.result.elapsed_ms,
        event_delta = comparison.metric_deltas.events_processed_delta,
        violation_delta = comparison.metric_deltas.violations_total_delta,
        elapsed_delta = comparison.metric_deltas.elapsed_ms_delta,
        leaderboard_count = leaderboard.comparisons.len(),
        key_rows = manifest
            .key_files
            .iter()
            .map(|file| format!(
                "<tr><td>{}</td><td>{}</td><td>{}</td><td><code>{}</code></td></tr>",
                escape_html(&file.name),
                file.bytes,
                file.lines,
                escape_html(&file.sha256)
            ))
            .collect::<Vec<_>>()
            .join("")
    )
}

fn build_scale_packet_summary_record(manifest: &ScalePacketManifest) -> ScalePacketSummaryRecord {
    ScalePacketSummaryRecord {
        schema: "rzd-des-scale-packet-summary/v1",
        packet_id: manifest.packet_id.clone(),
        generated_at_unix_seconds: manifest.generated_at_unix_seconds,
        network_path: manifest.network_path.clone(),
        competition_profile: manifest.competition.profile.clone(),
        competition_events_processed: manifest.competition.events_processed,
        competition_violations_total: manifest.competition.violations_total,
        competition_elapsed_ms: manifest.competition.elapsed_ms,
        rzd_scale_profile: manifest.rzd_scale.profile.clone(),
        rzd_scale_events_processed: manifest.rzd_scale.events_processed,
        rzd_scale_violations_total: manifest.rzd_scale.violations_total,
        rzd_scale_elapsed_ms: manifest.rzd_scale.elapsed_ms,
        comparison_class: manifest.comparison.comparison_class.clone(),
        trace_divergence_class: manifest.comparison.trace_divergence_class.clone(),
        events_processed_delta: manifest.comparison.events_processed_delta,
        violations_total_delta: manifest.comparison.violations_total_delta,
        shoulder_drift_shoulders: manifest.comparison.shoulder_drift_shoulders,
    }
}

fn build_scale_packet_summary_tsv(record: &ScalePacketSummaryRecord) -> String {
    let header = [
        "schema",
        "packet_id",
        "generated_at_unix_seconds",
        "network_path",
        "competition_profile",
        "competition_events_processed",
        "competition_violations_total",
        "competition_elapsed_ms",
        "rzd_scale_profile",
        "rzd_scale_events_processed",
        "rzd_scale_violations_total",
        "rzd_scale_elapsed_ms",
        "comparison_class",
        "trace_divergence_class",
        "events_processed_delta",
        "violations_total_delta",
        "shoulder_drift_shoulders",
    ]
    .join("\t");

    let row = [
        record.schema.to_string(),
        record.packet_id.clone(),
        record.generated_at_unix_seconds.to_string(),
        record.network_path.clone(),
        record.competition_profile.clone(),
        record.competition_events_processed.to_string(),
        record.competition_violations_total.to_string(),
        format!("{:.3}", record.competition_elapsed_ms),
        record.rzd_scale_profile.clone(),
        record.rzd_scale_events_processed.to_string(),
        record.rzd_scale_violations_total.to_string(),
        format!("{:.3}", record.rzd_scale_elapsed_ms),
        record.comparison_class.clone(),
        record.trace_divergence_class.clone(),
        record.events_processed_delta.to_string(),
        record.violations_total_delta.to_string(),
        record.shoulder_drift_shoulders.to_string(),
    ]
    .join("\t");

    format!("{header}\n{row}\n")
}

fn render_engine_output(
    engine: &Engine,
    format: &str,
    fallback_seed: u32,
    fallback_warmup: u64,
    fallback_duration_days: u32,
) -> Result<String, Box<dyn std::error::Error>> {
    let rendered = match format {
        "tsv" => Export::tu3_to_tsv(&engine.tu3_rows),
        "json" => Export::tu3_to_json(&engine.tu3_rows)?,
        "ced" => serde_json::to_string_pretty(&engine.export_ced())?,
        "summary" => serde_json::to_string_pretty(&engine.violations_summary())?,
        "snapshot" => to_canonical_json(&engine.snapshot())?,
        "trace" => Export::event_trace_to_json(&engine.export_event_trace())?,
        "trace-ndjson" => Export::events_to_ndjson(&engine.processed_events)?,
        "bundle" => {
            let scenario = ScenarioConfig {
                seed: fallback_seed,
                duration_days: fallback_duration_days,
                ..Default::default()
            };
            let ced = engine.export_ced();
            let violation_summary = engine.violations_summary();
            let bundle = build_replay_bundle(ReplayBundleInputs {
                network: &engine.network,
                scenario: &scenario,
                seed: fallback_seed,
                until_minutes: engine.current_time,
                warmup_minutes: fallback_warmup,
                stats: engine.stats(),
                violation_summary: &violation_summary,
                tu3_rows: &engine.tu3_rows,
                ced: &ced,
            });
            to_canonical_json(&bundle)?
        }
        other => {
            return Err(format!(
                "Unknown format: {}. Use 'tsv', 'json', 'ced', 'bundle', 'summary', 'snapshot', 'trace', or 'trace-ndjson'",
                other
            )
            .into())
        }
    };

    Ok(rendered)
}

fn build_benchmark_report_json(result: &BenchmarkRunResult, network: &Path) -> serde_json::Value {
    serde_json::json!({
        "schema": "rzd-des-benchmark-report/v1",
        "profile": result.profile,
        "network": {
            "path": network.display().to_string(),
            "roads": result.roads,
            "stations": result.stations,
            "segments": result.segments,
            "legs": result.legs
        },
        "scenario": {
            "crews": result.scenario.crew_count,
            "locos": result.scenario.loco_count,
            "days": result.scenario.duration_days,
            "seed": result.scenario.seed,
            "warmup_minutes": result.warmup_minutes,
            "shift_start_offset_min": result.shift_start_offset_min,
            "shift_interval_min": result.shift_interval_min,
            "injection": result.injection,
            "synthetic_manual_violations": result.synthetic_manual_violations,
        },
        "results": {
            "elapsed_ms": result.elapsed_ms,
            "events_processed": result.events_processed,
            "events_per_second": result.events_per_second,
            "violations_total": result.violations_total,
            "violations_by_type": result.violations_by_type,
            "violations_by_severity": result.violations_by_severity,
            "shoulder_metrics": result.shoulder_metrics,
            "shifts_completed": result.shifts_completed,
            "distance_km": result.distance_km,
            "tu3_rows": result.tu3_rows,
            "event_errors": result.event_errors,
            "trace_signature": result.trace_signature,
            "exports": result.exports
        }
    })
}

fn run_benchmark_profile(
    network_path: &PathBuf,
    profile: &BenchmarkProfile,
) -> Result<BenchmarkRunResult, Box<dyn std::error::Error>> {
    Ok(run_benchmark_profile_with_artifacts(network_path, profile)?.result)
}

fn run_benchmark_profile_with_artifacts(
    network_path: &PathBuf,
    profile: &BenchmarkProfile,
) -> Result<BenchmarkExecutionArtifacts, Box<dyn std::error::Error>> {
    let network_json = std::fs::read_to_string(network_path)?;
    let net = Network::from_json(&network_json)?;
    let mut engine = Engine::new(net, profile.scenario.seed);
    let scenario = generate_scenario(&engine.network, profile.scenario.clone());

    for crew in scenario.crews {
        engine.add_crew(crew);
    }
    for loco in scenario.locos {
        engine.add_loco(loco);
    }

    engine.set_warmup(profile.warmup_minutes);
    if let Some(inject_path) = profile.inject {
        let inject_json = std::fs::read_to_string(inject_path)?;
        let plan: InjectionPlan = serde_json::from_str(&inject_json)?;
        engine.apply_injection_plan(&plan)?;
    }

    engine.schedule_all_shifts(profile.shift_start_offset_min, profile.shift_interval_min);
    let until = (profile.scenario.duration_days as u64) * 1440;
    if let Some(burst) = profile.synthetic_manual_violations {
        let plan = build_synthetic_manual_violation_plan(&engine, &profile.scenario, burst, until);
        engine.apply_injection_plan(&plan)?;
    }
    let started = Instant::now();
    engine.run(until);
    let elapsed = started.elapsed();
    let elapsed_ms = elapsed.as_secs_f64() * 1000.0;
    let events_processed = engine.stats().total_events_processed;
    let events_per_second = if elapsed.as_secs_f64() > 0.0 {
        events_processed as f64 / elapsed.as_secs_f64()
    } else {
        0.0
    };
    let tu3_tsv_started = Instant::now();
    let tu3_tsv = Export::tu3_to_tsv(&engine.tu3_rows);
    let tu3_tsv_elapsed_ms = tu3_tsv_started.elapsed().as_secs_f64() * 1000.0;

    let ced_json_started = Instant::now();
    let ced_json = serde_json::to_string_pretty(&engine.export_ced())?;
    let ced_json_elapsed_ms = ced_json_started.elapsed().as_secs_f64() * 1000.0;

    let violations_ndjson_started = Instant::now();
    let violations_ndjson = Export::violations_to_ndjson(&engine.violations)?;
    let violations_ndjson_elapsed_ms = violations_ndjson_started.elapsed().as_secs_f64() * 1000.0;

    let event_trace_ndjson_started = Instant::now();
    let event_trace_ndjson = Export::events_to_ndjson(&engine.processed_events)?;
    let event_trace_ndjson_elapsed_ms = event_trace_ndjson_started.elapsed().as_secs_f64() * 1000.0;
    let trace_signature = sha256_hex(event_trace_ndjson.as_bytes());

    let violation_summary = engine.violations_summary();
    let violations_by_type = normalize_violation_type_counts(&violation_summary.by_type);
    let violations_by_severity = normalize_string_counts(&violation_summary.by_severity);
    let shoulder_metrics = summarize_shoulders(
        &engine.network,
        &engine.crews,
        &engine.locos,
        &engine.processed_events,
        &engine.violations.violations,
    );
    let exports = BenchmarkExportMetrics {
        tu3_tsv: BenchmarkExportArtifact {
            bytes: tu3_tsv.len(),
            lines: count_lines(&tu3_tsv),
            render_ms: tu3_tsv_elapsed_ms,
        },
        ced_json: BenchmarkExportArtifact {
            bytes: ced_json.len(),
            lines: count_lines(&ced_json),
            render_ms: ced_json_elapsed_ms,
        },
        violations_ndjson: BenchmarkExportArtifact {
            bytes: violations_ndjson.len(),
            lines: count_lines(&violations_ndjson),
            render_ms: violations_ndjson_elapsed_ms,
        },
        event_trace_ndjson: BenchmarkExportArtifact {
            bytes: event_trace_ndjson.len(),
            lines: count_lines(&event_trace_ndjson),
            render_ms: event_trace_ndjson_elapsed_ms,
        },
    };
    let business_signature = BenchmarkBusinessSignature {
        events_processed,
        violations_total: violation_summary.total,
        violations_by_type: violations_by_type.clone(),
        violations_by_severity: violations_by_severity.clone(),
        shoulder_metrics: shoulder_metrics.clone(),
        shifts_completed: engine.stats().total_shifts_completed,
        distance_km: normalized_distance(engine.stats().total_distance_km),
        tu3_rows: engine.tu3_rows.len(),
        event_errors: engine.event_errors,
    };
    let business_signature_json = serde_json::to_string(&business_signature)?;
    let result = BenchmarkRunResult {
        profile: profile.name,
        scenario: profile.scenario.clone(),
        warmup_minutes: profile.warmup_minutes,
        shift_start_offset_min: profile.shift_start_offset_min,
        shift_interval_min: profile.shift_interval_min,
        injection: profile.inject,
        synthetic_manual_violations: profile.synthetic_manual_violations.map(|v| v.count),
        roads: engine.network.roads.len(),
        stations: engine.network.stations.len(),
        segments: engine.network.segments.len(),
        legs: engine.network.legs.len(),
        elapsed_ms,
        events_processed,
        events_per_second,
        violations_total: violation_summary.total,
        violations_by_type,
        violations_by_severity,
        shoulder_metrics,
        shifts_completed: engine.stats().total_shifts_completed,
        distance_km: engine.stats().total_distance_km,
        tu3_rows: engine.tu3_rows.len(),
        event_errors: engine.event_errors,
        exports,
        business_signature,
        business_signature_json,
        trace_signature,
    };

    Ok(BenchmarkExecutionArtifacts {
        result,
        tu3_tsv,
        ced_json,
        violations_ndjson,
        event_trace_ndjson,
    })
}

fn normalize_violation_type_counts<K>(
    counts: &std::collections::HashMap<K, u32>,
) -> BTreeMap<String, u32>
where
    K: std::fmt::Debug,
{
    counts
        .iter()
        .map(|(key, value)| (format!("{:?}", key), *value))
        .collect()
}

fn normalize_string_counts(
    counts: &std::collections::HashMap<String, u32>,
) -> BTreeMap<String, u32> {
    counts
        .iter()
        .map(|(key, value)| (key.clone(), *value))
        .collect()
}

fn normalized_distance(distance_km: f64) -> String {
    format!("{distance_km:.3}")
}

fn build_shoulder_metric_deltas(
    baseline: &BTreeMap<String, ShoulderMetrics>,
    current: &BTreeMap<String, ShoulderMetrics>,
) -> BTreeMap<String, ShoulderMetricDelta> {
    let mut keys: BTreeSet<String> = baseline.keys().cloned().collect();
    keys.extend(current.keys().cloned());

    keys.into_iter()
        .map(|key| {
            let baseline_metrics = baseline.get(&key).cloned().unwrap_or_default();
            let current_metrics = current.get(&key).cloned().unwrap_or_default();
            let delta = ShoulderMetricDelta {
                events_processed_delta: current_metrics.events_processed as i64
                    - baseline_metrics.events_processed as i64,
                violations_total_delta: current_metrics.violations_total as i64
                    - baseline_metrics.violations_total as i64,
                unique_crews_delta: current_metrics.unique_crews as i64
                    - baseline_metrics.unique_crews as i64,
                unique_locos_delta: current_metrics.unique_locos as i64
                    - baseline_metrics.unique_locos as i64,
                segments_touched_delta: current_metrics.segments_touched as i64
                    - baseline_metrics.segments_touched as i64,
                first_event_minute_delta: option_u64_delta(
                    baseline_metrics.first_event_minute,
                    current_metrics.first_event_minute,
                ),
                last_event_minute_delta: option_u64_delta(
                    baseline_metrics.last_event_minute,
                    current_metrics.last_event_minute,
                ),
            };
            (key, delta)
        })
        .collect()
}

fn option_u64_delta(baseline: Option<u64>, current: Option<u64>) -> Option<i64> {
    match (baseline, current) {
        (Some(a), Some(b)) => Some(b as i64 - a as i64),
        (None, Some(b)) => Some(b as i64),
        (Some(a), None) => Some(-(a as i64)),
        (None, None) => None,
    }
}

fn count_nonzero_shoulder_drift(deltas: &BTreeMap<String, ShoulderMetricDelta>) -> usize {
    deltas
        .values()
        .filter(|delta| {
            delta.events_processed_delta != 0
                || delta.violations_total_delta != 0
                || delta.unique_crews_delta != 0
                || delta.unique_locos_delta != 0
                || delta.segments_touched_delta != 0
                || delta.first_event_minute_delta.unwrap_or(0) != 0
                || delta.last_event_minute_delta.unwrap_or(0) != 0
        })
        .count()
}

fn summarize_shoulders(
    network: &Network,
    crews: &HashMap<String, rzd_core::entities::Crew>,
    locos: &HashMap<String, rzd_core::entities::Locomotive>,
    events: &[rzd_core::events::SimEvent],
    violations: &[rzd_core::violations::Violation],
) -> BTreeMap<String, ShoulderMetrics> {
    let mut depot_regions: HashMap<&str, &str> = HashMap::new();
    for station in network.stations.values() {
        if let Some(depot_id) = station.depot_id.as_deref() {
            depot_regions
                .entry(depot_id)
                .or_insert(station.region.as_str());
        }
    }

    let crew_regions: HashMap<&str, &str> = crews
        .iter()
        .filter_map(|(crew_id, crew)| {
            depot_regions
                .get(crew.depot_id.as_str())
                .copied()
                .map(|region| (crew_id.as_str(), region))
        })
        .collect();
    let loco_regions: HashMap<&str, &str> = locos
        .iter()
        .filter_map(|(loco_id, loco)| {
            depot_regions
                .get(loco.depot_id.as_str())
                .copied()
                .map(|region| (loco_id.as_str(), region))
        })
        .collect();

    let mut accumulators: BTreeMap<String, ShoulderMetricsAccumulator> = BTreeMap::new();

    for event in events {
        if let Some(region) =
            resolve_region_for_trace_event(network, &crew_regions, &loco_regions, event)
        {
            let acc = accumulators.entry(region.to_string()).or_default();
            acc.events_processed += 1;
            if let Some(crew_id) = event.crew_id.as_deref() {
                acc.crews.insert(crew_id.to_string());
            }
            if let Some(loco_id) = event.loco_id.as_deref() {
                acc.locos.insert(loco_id.to_string());
            }
            if let Some(segment_id) = event.segment_id.as_deref() {
                acc.segments.insert(segment_id.to_string());
            }
            acc.first_event_minute = Some(
                acc.first_event_minute
                    .map_or(event.time, |current| current.min(event.time)),
            );
            acc.last_event_minute = Some(
                acc.last_event_minute
                    .map_or(event.time, |current| current.max(event.time)),
            );
        }
    }

    for violation in violations {
        if let Some(region) =
            resolve_region_for_violation(network, &crew_regions, &loco_regions, violation)
        {
            let acc = accumulators.entry(region.to_string()).or_default();
            acc.violations_total += 1;
            acc.crews.insert(violation.crew_id.clone());
            if let Some(loco_id) = violation.loco_id.as_deref() {
                acc.locos.insert(loco_id.to_string());
            }
            if let Some(segment_id) = violation.segment_id.as_deref() {
                acc.segments.insert(segment_id.to_string());
            }
        }
    }

    accumulators
        .into_iter()
        .map(|(region, acc)| {
            (
                region,
                ShoulderMetrics {
                    events_processed: acc.events_processed,
                    violations_total: acc.violations_total,
                    unique_crews: acc.crews.len(),
                    unique_locos: acc.locos.len(),
                    segments_touched: acc.segments.len(),
                    first_event_minute: acc.first_event_minute,
                    last_event_minute: acc.last_event_minute,
                },
            )
        })
        .collect()
}

fn resolve_region_for_trace_event<'a>(
    network: &'a Network,
    crew_regions: &HashMap<&str, &'a str>,
    loco_regions: &HashMap<&str, &'a str>,
    event: &'a rzd_core::events::SimEvent,
) -> Option<&'a str> {
    event
        .segment_id
        .as_deref()
        .and_then(|segment_id| network.region_for_segment(segment_id))
        .or_else(|| {
            event
                .crew_id
                .as_deref()
                .and_then(|crew_id| crew_regions.get(crew_id).copied())
        })
        .or_else(|| {
            event
                .loco_id
                .as_deref()
                .and_then(|loco_id| loco_regions.get(loco_id).copied())
        })
}

fn resolve_region_for_violation<'a>(
    network: &'a Network,
    crew_regions: &HashMap<&str, &'a str>,
    loco_regions: &HashMap<&str, &'a str>,
    violation: &'a rzd_core::violations::Violation,
) -> Option<&'a str> {
    violation
        .segment_id
        .as_deref()
        .and_then(|segment_id| network.region_for_segment(segment_id))
        .or_else(|| crew_regions.get(violation.crew_id.as_str()).copied())
        .or_else(|| {
            violation
                .loco_id
                .as_deref()
                .and_then(|loco_id| loco_regions.get(loco_id).copied())
        })
}

fn count_lines(rendered: &str) -> usize {
    rendered.lines().count()
}

fn build_bundle_file(name: &str, content: &str) -> BenchmarkBundleFile {
    BenchmarkBundleFile {
        name: name.to_string(),
        sha256: sha256_hex(content.as_bytes()),
        bytes: content.len(),
        lines: count_lines(content),
    }
}

fn build_bundle_summary_markdown(context: BenchmarkBundleSummaryContext<'_>) -> String {
    let result = context.result;
    let manifest = context.manifest;

    let mut out = String::new();
    out.push_str("# Benchmark Bundle Summary\n\n");
    out.push_str(&format!("- Profile: `{}`\n", result.profile));
    out.push_str(&format!("- Network: `{}`\n", manifest.network_path));
    out.push_str(&format!(
        "- Events processed: `{}`\n",
        result.events_processed
    ));
    out.push_str(&format!(
        "- Violations total: `{}`\n",
        result.violations_total
    ));
    out.push_str(&format!(
        "- Shifts completed: `{}`\n",
        result.shifts_completed
    ));
    out.push_str(&format!("- TU-3 rows: `{}`\n", result.tu3_rows));
    out.push_str(&format!("- Event errors: `{}`\n", result.event_errors));
    out.push_str(&format!(
        "- Simulation elapsed: `{:.3} ms`\n",
        result.elapsed_ms
    ));
    out.push_str(&format!(
        "- Throughput: `{:.3}` events/sec\n",
        result.events_per_second
    ));
    out.push_str(&format!("- Distance: `{:.3}` km\n\n", result.distance_km));

    out.push_str("## Export Artifacts\n\n");
    out.push_str("| File | Bytes | Lines | SHA-256 |\n");
    out.push_str("|------|------:|------:|--------|\n");
    for file in &manifest.files {
        out.push_str(&format!(
            "| `{}` | `{}` | `{}` | `{}` |\n",
            file.name, file.bytes, file.lines, file.sha256
        ));
    }

    out.push_str("\n## Export Render Metrics\n\n");
    out.push_str("| Artifact | Render ms |\n");
    out.push_str("|----------|----------:|\n");
    out.push_str(&format!(
        "| `tu3.tsv` | `{:.3}` |\n",
        result.exports.tu3_tsv.render_ms
    ));
    out.push_str(&format!(
        "| `ced.json` | `{:.3}` |\n",
        result.exports.ced_json.render_ms
    ));
    out.push_str(&format!(
        "| `violations.ndjson` | `{:.3}` |\n",
        result.exports.violations_ndjson.render_ms
    ));
    out.push_str(&format!(
        "| `event_trace.ndjson` | `{:.3}` |\n",
        result.exports.event_trace_ndjson.render_ms
    ));

    out.push_str("\n## Deterministic Signature\n\n```json\n");
    out.push_str(
        &serde_json::to_string_pretty(&result.business_signature)
            .expect("business signature should serialize"),
    );
    out.push_str("\n```\n");
    if !result.shoulder_metrics.is_empty() {
        out.push_str("\n## Shoulder Metrics\n\n");
        out.push_str("| Shoulder | Events | Violations | Crews | Locos | Segments | First Min | Last Min |\n");
        out.push_str("|----------|-------:|-----------:|------:|------:|---------:|----------:|---------:|\n");
        for (shoulder, metrics) in &result.shoulder_metrics {
            out.push_str(&format!(
                "| `{}` | `{}` | `{}` | `{}` | `{}` | `{}` | `{}` | `{}` |\n",
                shoulder,
                metrics.events_processed,
                metrics.violations_total,
                metrics.unique_crews,
                metrics.unique_locos,
                metrics.segments_touched,
                metrics
                    .first_event_minute
                    .map_or_else(|| "n/a".to_string(), |value| value.to_string()),
                metrics
                    .last_event_minute
                    .map_or_else(|| "n/a".to_string(), |value| value.to_string())
            ));
        }
    }
    out
}

fn build_bundle_report_html(context: BenchmarkBundleSummaryContext<'_>) -> String {
    let result = context.result;
    let manifest = context.manifest;
    let signature_json = serde_json::to_string_pretty(&result.business_signature)
        .expect("business signature should serialize");

    let artifact_rows = manifest
        .files
        .iter()
        .map(|file| {
            format!(
                "<tr><td>{}</td><td>{}</td><td>{}</td><td><code>{}</code></td></tr>",
                escape_html(&file.name),
                file.bytes,
                file.lines,
                escape_html(&file.sha256)
            )
        })
        .collect::<Vec<_>>()
        .join("");

    format!(
        "<!doctype html>\
<html lang=\"en\">\
<head>\
<meta charset=\"utf-8\">\
<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\
<title>DES Benchmark Bundle Report</title>\
<style>\
body{{font-family:Segoe UI,Arial,sans-serif;margin:0;background:#0b1020;color:#e8edf7;}}\
.wrap{{max-width:1120px;margin:0 auto;padding:32px 24px 48px;}}\
h1,h2{{margin:0 0 16px;}}\
h1{{font-size:32px;}}\
h2{{font-size:20px;margin-top:28px;}}\
p{{margin:0;line-height:1.5;color:#b8c2d9;}}\
.grid{{display:grid;grid-template-columns:repeat(auto-fit,minmax(180px,1fr));gap:12px;margin-top:20px;}}\
.metric{{background:#131a2e;border:1px solid #26314f;border-radius:8px;padding:14px;}}\
.label{{font-size:12px;color:#8fa0c4;text-transform:uppercase;letter-spacing:.04em;}}\
.value{{font-size:24px;font-weight:600;margin-top:6px;}}\
table{{width:100%;border-collapse:collapse;margin-top:12px;background:#131a2e;border:1px solid #26314f;}}\
th,td{{padding:10px 12px;border-bottom:1px solid #26314f;text-align:left;vertical-align:top;}}\
th{{color:#9fb0d6;font-size:12px;text-transform:uppercase;letter-spacing:.04em;}}\
code,pre{{font-family:Consolas,monospace;}}\
pre{{background:#131a2e;border:1px solid #26314f;border-radius:8px;padding:16px;overflow:auto;color:#dfe7fb;}}\
.subtle{{margin-top:8px;color:#8fa0c4;}}\
</style>\
</head>\
<body>\
<div class=\"wrap\">\
<h1>Benchmark Bundle Report</h1>\
<p>Synthetic one-road benchmark bundle for external review and contest demonstration.</p>\
<div class=\"grid\">\
<div class=\"metric\"><div class=\"label\">Profile</div><div class=\"value\">{profile}</div></div>\
<div class=\"metric\"><div class=\"label\">Events</div><div class=\"value\">{events}</div></div>\
<div class=\"metric\"><div class=\"label\">Violations</div><div class=\"value\">{violations}</div></div>\
<div class=\"metric\"><div class=\"label\">Shifts</div><div class=\"value\">{shifts}</div></div>\
<div class=\"metric\"><div class=\"label\">TU-3 Rows</div><div class=\"value\">{tu3}</div></div>\
<div class=\"metric\"><div class=\"label\">Throughput</div><div class=\"value\">{eps:.3}</div></div>\
</div>\
<p class=\"subtle\">Network: <code>{network}</code></p>\
<h2>Artifacts</h2>\
<table>\
<thead><tr><th>File</th><th>Bytes</th><th>Lines</th><th>SHA-256</th></tr></thead>\
<tbody>{artifact_rows}</tbody>\
</table>\
<h2>Render Times</h2>\
<table>\
<thead><tr><th>Artifact</th><th>Render ms</th></tr></thead>\
<tbody>\
<tr><td>tu3.tsv</td><td>{tu3_ms:.3}</td></tr>\
<tr><td>ced.json</td><td>{ced_ms:.3}</td></tr>\
<tr><td>violations.ndjson</td><td>{ndjson_ms:.3}</td></tr>\
<tr><td>event_trace.ndjson</td><td>{trace_ms:.3}</td></tr>\
</tbody>\
</table>\
<h2>Deterministic Signature</h2>\
<pre>{signature}</pre>\
</div>\
</body></html>",
        profile = escape_html(result.profile),
        events = result.events_processed,
        violations = result.violations_total,
        shifts = result.shifts_completed,
        tu3 = result.tu3_rows,
        eps = result.events_per_second,
        network = escape_html(&manifest.network_path),
        artifact_rows = artifact_rows,
        tu3_ms = result.exports.tu3_tsv.render_ms,
        ced_ms = result.exports.ced_json.render_ms,
        ndjson_ms = result.exports.violations_ndjson.render_ms,
        trace_ms = result.exports.event_trace_ndjson.render_ms,
        signature = escape_html(&signature_json),
    )
}

fn build_judging_packet_markdown(context: BenchmarkBundleSummaryContext<'_>) -> String {
    let result = context.result;
    let manifest = context.manifest;

    let mut out = String::new();
    out.push_str("# Judging Packet\n\n");
    out.push_str("## What This Demonstrates\n\n");
    out.push_str(
        "This bundle demonstrates a deterministic Rust discrete-event railway workload on a fully synthetic one-road network with no real station names and no real crew identities.\n\n",
    );

    out.push_str("## Core Metrics\n\n");
    out.push_str(&format!("- Profile: `{}`\n", result.profile));
    out.push_str(&format!("- Network: `{}`\n", manifest.network_path));
    out.push_str(&format!(
        "- Events processed: `{}`\n",
        result.events_processed
    ));
    out.push_str(&format!(
        "- Violations produced: `{}`\n",
        result.violations_total
    ));
    out.push_str(&format!(
        "- Shifts completed: `{}`\n",
        result.shifts_completed
    ));
    out.push_str(&format!("- TU-3 rows: `{}`\n", result.tu3_rows));
    out.push_str(&format!(
        "- Throughput: `{:.3}` events/sec\n",
        result.events_per_second
    ));
    out.push_str(&format!(
        "- Distance simulated: `{:.3}` km\n\n",
        result.distance_km
    ));

    out.push_str("## External Artifacts\n\n");
    out.push_str("| File | Bytes | Lines |\n");
    out.push_str("|------|------:|------:|\n");
    for file in &manifest.files {
        out.push_str(&format!(
            "| `{}` | `{}` | `{}` |\n",
            file.name, file.bytes, file.lines
        ));
    }

    out.push_str("\n## Reproducibility\n\n");
    out.push_str("- `manifest.json` records SHA-256 for each emitted artifact.\n");
    out.push_str("- `report.json` records scenario parameters and measured outputs.\n");
    out.push_str("- `SUMMARY.md` provides a concise engineering summary.\n");
    out.push_str("- `report.html` provides an offline demo view for reviewers.\n");
    out.push_str("- The business signature below captures the deterministic result envelope for this run.\n\n");

    out.push_str("## Deterministic Signature\n\n```json\n");
    out.push_str(
        &serde_json::to_string_pretty(&result.business_signature)
            .expect("business signature should serialize"),
    );
    out.push_str("\n```\n");
    out
}

fn build_bundle_comparison_report(
    baseline_dir: &Path,
    current_dir: &Path,
    baseline_report: &BenchmarkReport,
    current_report: &BenchmarkReport,
    baseline_manifest: &BenchmarkBundleManifest,
    current_manifest: &BenchmarkBundleManifest,
) -> BundleComparisonReport {
    let baseline_signature = business_signature_from_report(baseline_report);
    let current_signature = business_signature_from_report(current_report);
    let baseline_files = manifest_file_map(baseline_manifest);
    let current_files = manifest_file_map(current_manifest);
    let mut names: Vec<String> = baseline_files
        .keys()
        .chain(current_files.keys())
        .cloned()
        .collect();
    names.sort();
    names.dedup();

    let file_deltas: Vec<BundleFileDelta> = names
        .into_iter()
        .map(|name| {
            let baseline = baseline_files.get(&name);
            let current = current_files.get(&name);
            BundleFileDelta {
                name,
                baseline_bytes: baseline.map(|f| f.bytes).unwrap_or(0),
                current_bytes: current.map(|f| f.bytes).unwrap_or(0),
                baseline_lines: baseline.map(|f| f.lines).unwrap_or(0),
                current_lines: current.map(|f| f.lines).unwrap_or(0),
                same_sha256: match (baseline, current) {
                    (Some(a), Some(b)) => a.sha256 == b.sha256,
                    _ => false,
                },
            }
        })
        .collect();

    let file_hashes_match = manifest_file_map(baseline_manifest)
        .iter()
        .all(|(name, file)| {
            current_manifest
                .files
                .iter()
                .find(|other| other.name == *name)
                .is_some_and(|other| other.sha256 == file.sha256)
        });
    let artifact_outputs_match = file_deltas
        .iter()
        .filter(|file| file.name != "report.json")
        .all(|file| file.same_sha256);
    let trace_outputs_match = file_deltas
        .iter()
        .find(|file| file.name == "event_trace.ndjson")
        .is_some_and(|file| file.same_sha256);
    let baseline_trace_signature =
        effective_trace_signature(baseline_report, baseline_manifest).unwrap_or_default();
    let current_trace_signature =
        effective_trace_signature(current_report, current_manifest).unwrap_or_default();
    let trace_signatures_match = baseline_trace_signature == current_trace_signature;
    let baseline_trace_events = load_trace_events_from_bundle(baseline_dir).ok().flatten();
    let current_trace_events = load_trace_events_from_bundle(current_dir).ok().flatten();
    let trace_event_count_delta = current_trace_events
        .as_ref()
        .map(|events| events.len() as i64)
        .unwrap_or(0)
        - baseline_trace_events
            .as_ref()
            .map(|events| events.len() as i64)
            .unwrap_or(0);
    let first_trace_divergence = match (&baseline_trace_events, &current_trace_events) {
        (Some(baseline), Some(current)) => first_trace_divergence(baseline, current),
        _ => None,
    };
    let trace_divergence_class = classify_trace_drift(
        baseline_trace_events.as_deref(),
        current_trace_events.as_deref(),
        first_trace_divergence.as_ref(),
    );
    let business_outputs_match = baseline_signature == current_signature;
    let timing_only_drift = business_outputs_match && artifact_outputs_match && !file_hashes_match;
    let shoulder_deltas = build_shoulder_metric_deltas(
        &baseline_report.results.shoulder_metrics,
        &current_report.results.shoulder_metrics,
    );
    let comparison_class = if file_hashes_match {
        "fully_equivalent"
    } else if timing_only_drift {
        "timing_only_drift"
    } else if business_outputs_match {
        "business_equivalent_only"
    } else {
        "business_drift"
    };

    BundleComparisonReport {
        schema: "rzd-des-benchmark-comparison/v1".to_string(),
        baseline_dir: baseline_dir.display().to_string(),
        current_dir: current_dir.display().to_string(),
        baseline_profile: baseline_report.profile.clone(),
        current_profile: current_report.profile.clone(),
        business_outputs_match,
        file_hashes_match,
        artifact_outputs_match,
        trace_outputs_match,
        trace_signatures_match,
        trace_event_count_delta,
        trace_divergence_class: trace_divergence_class.to_string(),
        first_trace_divergence,
        timing_only_drift,
        comparison_class: comparison_class.to_string(),
        metric_deltas: BundleComparisonMetricDeltas {
            events_processed_delta: current_report.results.events_processed as i64
                - baseline_report.results.events_processed as i64,
            violations_total_delta: current_report.results.violations_total as i64
                - baseline_report.results.violations_total as i64,
            shifts_completed_delta: current_report.results.shifts_completed as i64
                - baseline_report.results.shifts_completed as i64,
            distance_km_delta: current_report.results.distance_km
                - baseline_report.results.distance_km,
            elapsed_ms_delta: current_report.results.elapsed_ms
                - baseline_report.results.elapsed_ms,
            events_per_second_delta: current_report.results.events_per_second
                - baseline_report.results.events_per_second,
            shoulder_deltas,
        },
        file_deltas,
    }
}

fn manifest_file_map(manifest: &BenchmarkBundleManifest) -> BTreeMap<String, &BenchmarkBundleFile> {
    manifest
        .files
        .iter()
        .map(|file| (file.name.clone(), file))
        .collect()
}

fn effective_trace_signature(
    report: &BenchmarkReport,
    manifest: &BenchmarkBundleManifest,
) -> Option<String> {
    if !report.results.trace_signature.is_empty() {
        return Some(report.results.trace_signature.clone());
    }

    manifest
        .files
        .iter()
        .find(|file| file.name == "event_trace.ndjson")
        .map(|file| file.sha256.clone())
}

fn load_trace_events_from_bundle(
    bundle_dir: &Path,
) -> Result<Option<Vec<rzd_core::events::SimEvent>>, Box<dyn std::error::Error>> {
    let path = bundle_dir.join("event_trace.ndjson");
    if !path.exists() {
        return Ok(None);
    }

    let content = std::fs::read_to_string(path)?;
    let mut events = Vec::new();
    for line in content.lines().filter(|line| !line.trim().is_empty()) {
        let event: rzd_core::events::SimEvent = serde_json::from_str(line)?;
        events.push(event);
    }
    Ok(Some(events))
}

fn first_trace_divergence(
    baseline: &[rzd_core::events::SimEvent],
    current: &[rzd_core::events::SimEvent],
) -> Option<TraceDivergence> {
    let max_len = baseline.len().max(current.len());
    for idx in 0..max_len {
        let b = baseline.get(idx);
        let c = current.get(idx);
        if b == c {
            continue;
        }

        return Some(TraceDivergence {
            index: idx,
            baseline_time: b.map(|e| e.time),
            current_time: c.map(|e| e.time),
            baseline_kind: b.map(|e| format!("{:?}", e.kind)),
            current_kind: c.map(|e| format!("{:?}", e.kind)),
            baseline_crew_id: b.and_then(|e| e.crew_id.clone()),
            current_crew_id: c.and_then(|e| e.crew_id.clone()),
            baseline_loco_id: b.and_then(|e| e.loco_id.clone()),
            current_loco_id: c.and_then(|e| e.loco_id.clone()),
            baseline_segment_id: b.and_then(|e| e.segment_id.clone()),
            current_segment_id: c.and_then(|e| e.segment_id.clone()),
        });
    }
    None
}

fn classify_trace_drift(
    baseline: Option<&[rzd_core::events::SimEvent]>,
    current: Option<&[rzd_core::events::SimEvent]>,
    divergence: Option<&TraceDivergence>,
) -> &'static str {
    let Some(divergence) = divergence else {
        return "no_trace_drift";
    };

    let baseline_len = baseline.map(|events| events.len()).unwrap_or(0);
    let current_len = current.map(|events| events.len()).unwrap_or(0);
    if baseline_len != current_len {
        return "count_only_drift";
    }

    let kinds_match = divergence.baseline_kind == divergence.current_kind;
    let entity_context_matches = divergence.baseline_crew_id == divergence.current_crew_id
        && divergence.baseline_loco_id == divergence.current_loco_id
        && divergence.baseline_segment_id == divergence.current_segment_id;
    let time_matches = divergence.baseline_time == divergence.current_time;

    if kinds_match && entity_context_matches && !time_matches {
        return "timing_only_trace_drift";
    }
    if !kinds_match {
        return "event_kind_drift";
    }
    if !entity_context_matches {
        return "entity_context_drift";
    }
    "structural_trace_drift"
}

fn load_trace_events_from_file(
    path: &Path,
) -> Result<Vec<rzd_core::events::SimEvent>, Box<dyn std::error::Error>> {
    let content = std::fs::read_to_string(path)?;
    let mut events = Vec::new();
    for line in content.lines().filter(|line| !line.trim().is_empty()) {
        let event: rzd_core::events::SimEvent = serde_json::from_str(line)?;
        events.push(event);
    }
    Ok(events)
}

fn build_trace_comparison_report(
    baseline_trace: &Path,
    current_trace: &Path,
) -> Result<TraceComparisonReport, Box<dyn std::error::Error>> {
    let baseline_events = load_trace_events_from_file(baseline_trace)?;
    let current_events = load_trace_events_from_file(current_trace)?;
    let baseline_trace_signature = sha256_hex(std::fs::read(baseline_trace)?.as_slice());
    let current_trace_signature = sha256_hex(std::fs::read(current_trace)?.as_slice());
    let first_divergence = first_trace_divergence(&baseline_events, &current_events);
    let trace_divergence_class = classify_trace_drift(
        Some(&baseline_events),
        Some(&current_events),
        first_divergence.as_ref(),
    );

    Ok(TraceComparisonReport {
        schema: "rzd-des-trace-comparison/v1".to_string(),
        baseline_trace: baseline_trace.display().to_string(),
        current_trace: current_trace.display().to_string(),
        baseline_trace_signature: baseline_trace_signature.clone(),
        current_trace_signature: current_trace_signature.clone(),
        trace_signatures_match: baseline_trace_signature == current_trace_signature,
        baseline_event_count: baseline_events.len(),
        current_event_count: current_events.len(),
        trace_event_count_delta: current_events.len() as i64 - baseline_events.len() as i64,
        trace_divergence_class: trace_divergence_class.to_string(),
        first_trace_divergence: first_divergence,
    })
}

fn business_signature_from_report(report: &BenchmarkReport) -> String {
    serde_json::to_string(&serde_json::json!({
        "events_processed": report.results.events_processed,
        "violations_total": report.results.violations_total,
        "violations_by_type": report.results.violations_by_type,
        "violations_by_severity": report.results.violations_by_severity,
        "shoulder_metrics": report.results.shoulder_metrics,
        "shifts_completed": report.results.shifts_completed,
        "distance_km": format!("{:.3}", report.results.distance_km),
        "tu3_rows": report.results.tu3_rows,
        "event_errors": report.results.event_errors
    }))
    .expect("report signature should serialize")
}

fn build_bundle_comparison_markdown(report: &BundleComparisonReport) -> String {
    let mut out = String::new();
    out.push_str("# Bundle Comparison\n\n");
    out.push_str(&format!(
        "- Baseline: `{}`\n- Current: `{}`\n",
        report.baseline_dir, report.current_dir
    ));
    out.push_str(&format!(
        "- Comparison class: `{}`\n- Business outputs match: `{}`\n- Artifact outputs match: `{}`\n- Trace outputs match: `{}`\n- Trace signatures match: `{}`\n- File hashes match: `{}`\n- Timing-only drift: `{}`\n\n",
        report.comparison_class,
        report.business_outputs_match,
        report.artifact_outputs_match,
        report.trace_outputs_match,
        report.trace_signatures_match,
        report.file_hashes_match,
        report.timing_only_drift
    ));
    out.push_str(&format!(
        "- Trace divergence class: `{}`\n\n",
        report.trace_divergence_class
    ));
    out.push_str("## Metric Deltas\n\n");
    out.push_str(&format!(
        "- Events processed delta: `{}`\n- Violations total delta: `{}`\n- Shifts completed delta: `{}`\n- Distance delta: `{:.3}` km\n- Elapsed delta: `{:.3}` ms\n- Throughput delta: `{:.3}` events/sec\n- Trace event count delta: `{}`\n\n",
        report.metric_deltas.events_processed_delta,
        report.metric_deltas.violations_total_delta,
        report.metric_deltas.shifts_completed_delta,
        report.metric_deltas.distance_km_delta,
        report.metric_deltas.elapsed_ms_delta,
        report.metric_deltas.events_per_second_delta,
        report.trace_event_count_delta
    ));
    if !report.metric_deltas.shoulder_deltas.is_empty() {
        out.push_str("## Shoulder Deltas\n\n");
        out.push_str("| Shoulder | Events | Violations | Crews | Locos | Segments | First Min | Last Min |\n");
        out.push_str("|----------|-------:|-----------:|------:|------:|---------:|----------:|---------:|\n");
        for (shoulder, delta) in &report.metric_deltas.shoulder_deltas {
            out.push_str(&format!(
                "| `{}` | `{}` | `{}` | `{}` | `{}` | `{}` | `{}` | `{}` |\n",
                shoulder,
                delta.events_processed_delta,
                delta.violations_total_delta,
                delta.unique_crews_delta,
                delta.unique_locos_delta,
                delta.segments_touched_delta,
                delta
                    .first_event_minute_delta
                    .map_or_else(|| "n/a".to_string(), |value| value.to_string()),
                delta
                    .last_event_minute_delta
                    .map_or_else(|| "n/a".to_string(), |value| value.to_string())
            ));
        }
        out.push('\n');
    }
    if let Some(divergence) = &report.first_trace_divergence {
        out.push_str("## First Trace Divergence\n\n");
        out.push_str(&format!(
            "- Index: `{}`\n- Baseline time: `{:?}`\n- Current time: `{:?}`\n- Baseline kind: `{:?}`\n- Current kind: `{:?}`\n- Baseline crew: `{:?}`\n- Current crew: `{:?}`\n- Baseline loco: `{:?}`\n- Current loco: `{:?}`\n\n",
            divergence.index,
            divergence.baseline_time,
            divergence.current_time,
            divergence.baseline_kind,
            divergence.current_kind,
            divergence.baseline_crew_id,
            divergence.current_crew_id,
            divergence.baseline_loco_id,
            divergence.current_loco_id
        ));
    }
    out.push_str("## File Deltas\n\n| File | Baseline Bytes | Current Bytes | Baseline Lines | Current Lines | Same SHA |\n|------|---------------:|--------------:|---------------:|--------------:|:--------:|\n");
    for file in &report.file_deltas {
        out.push_str(&format!(
            "| `{}` | `{}` | `{}` | `{}` | `{}` | `{}` |\n",
            file.name,
            file.baseline_bytes,
            file.current_bytes,
            file.baseline_lines,
            file.current_lines,
            file.same_sha256
        ));
    }
    out
}

fn build_bundle_comparison_html(report: &BundleComparisonReport) -> String {
    let rows = report
        .file_deltas
        .iter()
        .map(|file| {
            format!(
                "<tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>",
                escape_html(&file.name),
                file.baseline_bytes,
                file.current_bytes,
                file.baseline_lines,
                file.current_lines,
                file.same_sha256
            )
        })
        .collect::<Vec<_>>()
        .join("");
    let shoulder_rows = report
        .metric_deltas
        .shoulder_deltas
        .iter()
        .map(|(shoulder, delta)| {
            format!(
                "<tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>",
                escape_html(shoulder),
                delta.events_processed_delta,
                delta.violations_total_delta,
                delta.unique_crews_delta,
                delta.unique_locos_delta,
                delta.segments_touched_delta,
                delta.first_event_minute_delta.map_or_else(|| "n/a".to_string(), |value| value.to_string()),
                delta.last_event_minute_delta.map_or_else(|| "n/a".to_string(), |value| value.to_string())
            )
        })
        .collect::<Vec<_>>()
        .join("");

    format!(
        "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width, initial-scale=1\"><title>Bundle Comparison</title><style>body{{font-family:Segoe UI,Arial,sans-serif;margin:0;background:#0b1020;color:#e8edf7;}}.wrap{{max-width:1120px;margin:0 auto;padding:32px 24px 48px;}}table{{width:100%;border-collapse:collapse;margin-top:12px;background:#131a2e;border:1px solid #26314f;}}th,td{{padding:10px 12px;border-bottom:1px solid #26314f;text-align:left;}}th{{color:#9fb0d6;font-size:12px;text-transform:uppercase;}}.metric{{background:#131a2e;border:1px solid #26314f;border-radius:8px;padding:14px;margin:12px 0;}}code{{font-family:Consolas,monospace;}}</style></head><body><div class=\"wrap\"><h1>Bundle Comparison</h1><div class=\"metric\"><div>Baseline: <code>{}</code></div><div>Current: <code>{}</code></div><div>Comparison class: <strong>{}</strong></div><div>Business outputs match: <strong>{}</strong></div><div>Artifact outputs match: <strong>{}</strong></div><div>Trace outputs match: <strong>{}</strong></div><div>Trace signatures match: <strong>{}</strong></div><div>Trace divergence class: <strong>{}</strong></div><div>Trace event count delta: <strong>{}</strong></div><div>File hashes match: <strong>{}</strong></div><div>Timing-only drift: <strong>{}</strong></div></div><h2>Shoulder Deltas</h2><table><thead><tr><th>Shoulder</th><th>Events</th><th>Violations</th><th>Crews</th><th>Locos</th><th>Segments</th><th>First Min</th><th>Last Min</th></tr></thead><tbody>{}</tbody></table><h2>File Deltas</h2><table><thead><tr><th>File</th><th>Baseline Bytes</th><th>Current Bytes</th><th>Baseline Lines</th><th>Current Lines</th><th>Same SHA</th></tr></thead><tbody>{}</tbody></table></div></body></html>",
        escape_html(&report.baseline_dir),
        escape_html(&report.current_dir),
        escape_html(&report.comparison_class),
        report.business_outputs_match,
        report.artifact_outputs_match,
        report.trace_outputs_match,
        report.trace_signatures_match,
        escape_html(&report.trace_divergence_class),
        report.trace_event_count_delta,
        report.file_hashes_match,
        report.timing_only_drift,
        shoulder_rows,
        rows
    )
}

fn build_trace_comparison_markdown(report: &TraceComparisonReport) -> String {
    let mut out = String::new();
    out.push_str("# Trace Comparison\n\n");
    out.push_str(&format!(
        "- Baseline trace: `{}`\n- Current trace: `{}`\n- Trace signatures match: `{}`\n- Trace divergence class: `{}`\n- Baseline events: `{}`\n- Current events: `{}`\n- Trace event count delta: `{}`\n\n",
        report.baseline_trace,
        report.current_trace,
        report.trace_signatures_match,
        report.trace_divergence_class,
        report.baseline_event_count,
        report.current_event_count,
        report.trace_event_count_delta
    ));

    if let Some(divergence) = &report.first_trace_divergence {
        out.push_str("## First Trace Divergence\n\n");
        out.push_str(&format!(
            "- Index: `{}`\n- Baseline time: `{:?}`\n- Current time: `{:?}`\n- Baseline kind: `{:?}`\n- Current kind: `{:?}`\n- Baseline crew: `{:?}`\n- Current crew: `{:?}`\n- Baseline loco: `{:?}`\n- Current loco: `{:?}`\n- Baseline segment: `{:?}`\n- Current segment: `{:?}`\n",
            divergence.index,
            divergence.baseline_time,
            divergence.current_time,
            divergence.baseline_kind,
            divergence.current_kind,
            divergence.baseline_crew_id,
            divergence.current_crew_id,
            divergence.baseline_loco_id,
            divergence.current_loco_id,
            divergence.baseline_segment_id,
            divergence.current_segment_id
        ));
    }

    out
}

fn build_trace_comparison_html(report: &TraceComparisonReport) -> String {
    let divergence_html = if let Some(divergence) = &report.first_trace_divergence {
        format!(
            "<h2>First Trace Divergence</h2><div class=\"metric\"><div>Index: <strong>{}</strong></div><div>Baseline time: <code>{:?}</code></div><div>Current time: <code>{:?}</code></div><div>Baseline kind: <code>{:?}</code></div><div>Current kind: <code>{:?}</code></div><div>Baseline crew: <code>{:?}</code></div><div>Current crew: <code>{:?}</code></div><div>Baseline loco: <code>{:?}</code></div><div>Current loco: <code>{:?}</code></div><div>Baseline segment: <code>{:?}</code></div><div>Current segment: <code>{:?}</code></div></div>",
            divergence.index,
            divergence.baseline_time,
            divergence.current_time,
            divergence.baseline_kind,
            divergence.current_kind,
            divergence.baseline_crew_id,
            divergence.current_crew_id,
            divergence.baseline_loco_id,
            divergence.current_loco_id,
            divergence.baseline_segment_id,
            divergence.current_segment_id
        )
    } else {
        "<h2>First Trace Divergence</h2><div class=\"metric\"><div>No divergence detected.</div></div>".to_string()
    };

    format!(
        "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width, initial-scale=1\"><title>Trace Comparison</title><style>body{{font-family:Segoe UI,Arial,sans-serif;margin:0;background:#0b1020;color:#e8edf7;}}.wrap{{max-width:1120px;margin:0 auto;padding:32px 24px 48px;}}.metric{{background:#131a2e;border:1px solid #26314f;border-radius:8px;padding:14px;margin:12px 0;}}code{{font-family:Consolas,monospace;}}</style></head><body><div class=\"wrap\"><h1>Trace Comparison</h1><div class=\"metric\"><div>Baseline trace: <code>{}</code></div><div>Current trace: <code>{}</code></div><div>Trace signatures match: <strong>{}</strong></div><div>Trace divergence class: <strong>{}</strong></div><div>Baseline events: <strong>{}</strong></div><div>Current events: <strong>{}</strong></div><div>Trace event count delta: <strong>{}</strong></div></div>{}</div></body></html>",
        escape_html(&report.baseline_trace),
        escape_html(&report.current_trace),
        report.trace_signatures_match,
        escape_html(&report.trace_divergence_class),
        report.baseline_event_count,
        report.current_event_count,
        report.trace_event_count_delta,
        divergence_html
    )
}

fn find_comparison_json_files(
    input_dir: &Path,
) -> Result<Vec<PathBuf>, Box<dyn std::error::Error>> {
    let mut found = Vec::new();
    for entry in std::fs::read_dir(input_dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            found.extend(find_comparison_json_files(&path)?);
        } else if path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name == "comparison.json")
        {
            found.push(path);
        }
    }
    Ok(found)
}

fn build_comparison_leaderboard(
    comparisons: &[BundleComparisonReport],
) -> ComparisonLeaderboardReport {
    let mut entries: Vec<ComparisonLeaderboardEntry> = comparisons
        .iter()
        .map(|comparison| ComparisonLeaderboardEntry {
            label: format!("{} -> {}", comparison.baseline_dir, comparison.current_dir),
            comparison_class: comparison.comparison_class.clone(),
            trace_divergence_class: comparison.trace_divergence_class.clone(),
            shoulder_drift_shoulders: count_nonzero_shoulder_drift(
                &comparison.metric_deltas.shoulder_deltas,
            ),
            business_outputs_match: comparison.business_outputs_match,
            artifact_outputs_match: comparison.artifact_outputs_match,
            trace_outputs_match: comparison.trace_outputs_match,
            trace_signatures_match: comparison.trace_signatures_match,
            file_hashes_match: comparison.file_hashes_match,
            timing_only_drift: comparison.timing_only_drift,
            elapsed_ms_delta: comparison.metric_deltas.elapsed_ms_delta,
            events_per_second_delta: comparison.metric_deltas.events_per_second_delta,
        })
        .collect();

    entries.sort_by(|a, b| {
        comparison_rank(&a.comparison_class)
            .cmp(&comparison_rank(&b.comparison_class))
            .then_with(|| a.label.cmp(&b.label))
    });

    ComparisonLeaderboardReport {
        schema: "rzd-des-benchmark-leaderboard/v1",
        comparisons: entries,
    }
}

fn comparison_rank(class: &str) -> u8 {
    match class {
        "fully_equivalent" => 0,
        "timing_only_drift" => 1,
        "business_equivalent_only" => 2,
        "business_drift" => 3,
        _ => 255,
    }
}

fn build_leaderboard_markdown(report: &ComparisonLeaderboardReport) -> String {
    let mut out = String::new();
    out.push_str("# Comparison Leaderboard\n\n");
    out.push_str("| Comparison | Class | Trace Drift | Shoulder Drift | Business | Artifacts | Trace | Trace Sig | Hashes | Timing Drift | Elapsed Delta ms | EPS Delta |\n");
    out.push_str("|------------|-------|-------------|---------------:|:--------:|:---------:|:-----:|:---------:|:------:|:------------:|-----------------:|----------:|\n");
    for entry in &report.comparisons {
        out.push_str(&format!(
            "| `{}` | `{}` | `{}` | `{}` | `{}` | `{}` | `{}` | `{}` | `{}` | `{}` | `{:.3}` | `{:.3}` |\n",
            entry.label,
            entry.comparison_class,
            entry.trace_divergence_class,
            entry.shoulder_drift_shoulders,
            entry.business_outputs_match,
            entry.artifact_outputs_match,
            entry.trace_outputs_match,
            entry.trace_signatures_match,
            entry.file_hashes_match,
            entry.timing_only_drift,
            entry.elapsed_ms_delta,
            entry.events_per_second_delta
        ));
    }
    out
}

fn build_leaderboard_html(report: &ComparisonLeaderboardReport) -> String {
    let rows = report
        .comparisons
        .iter()
        .map(|entry| {
            format!(
                "<tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{:.3}</td><td>{:.3}</td></tr>",
                escape_html(&entry.label),
                escape_html(&entry.comparison_class),
                escape_html(&entry.trace_divergence_class),
                entry.shoulder_drift_shoulders,
                entry.business_outputs_match,
                entry.artifact_outputs_match,
                entry.trace_outputs_match,
                entry.trace_signatures_match,
                entry.file_hashes_match,
                entry.timing_only_drift,
                entry.elapsed_ms_delta,
                entry.events_per_second_delta
            )
        })
        .collect::<Vec<_>>()
        .join("");

    format!(
        "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width, initial-scale=1\"><title>Comparison Leaderboard</title><style>body{{font-family:Segoe UI,Arial,sans-serif;margin:0;background:#0b1020;color:#e8edf7;}}.wrap{{max-width:1280px;margin:0 auto;padding:32px 24px 48px;}}table{{width:100%;border-collapse:collapse;margin-top:12px;background:#131a2e;border:1px solid #26314f;}}th,td{{padding:10px 12px;border-bottom:1px solid #26314f;text-align:left;vertical-align:top;}}th{{color:#9fb0d6;font-size:12px;text-transform:uppercase;}}code{{font-family:Consolas,monospace;}}</style></head><body><div class=\"wrap\"><h1>Comparison Leaderboard</h1><table><thead><tr><th>Comparison</th><th>Class</th><th>Trace Drift</th><th>Shoulder Drift</th><th>Business</th><th>Artifacts</th><th>Trace</th><th>Trace Sig</th><th>Hashes</th><th>Timing Drift</th><th>Elapsed Delta ms</th><th>EPS Delta</th></tr></thead><tbody>{}</tbody></table></div></body></html>",
        rows
    )
}

fn escape_html(input: &str) -> String {
    input
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    digest.iter().map(|b| format!("{b:02x}")).collect()
}

fn build_synthetic_manual_violation_plan(
    engine: &Engine,
    scenario: &ScenarioConfig,
    burst: SyntheticManualViolationBurst,
    until_minutes: u64,
) -> InjectionPlan {
    let mut crew_ids: Vec<String> = engine.crews.keys().cloned().collect();
    crew_ids.sort();

    let segment_ids = collect_scoped_segment_ids(&engine.network, scenario);

    let end_minute = until_minutes.saturating_sub(1).max(burst.start_minute);
    let events = build_synthetic_manual_violation_events(
        &crew_ids,
        &segment_ids,
        burst.count,
        burst.start_minute,
        end_minute,
    );

    InjectionPlan {
        schema: "rzd-des-injection-plan/v1".to_string(),
        events,
    }
}

fn collect_scoped_segment_ids(network: &Network, scenario: &ScenarioConfig) -> Vec<String> {
    let allowed_legs: BTreeSet<&str> = scenario
        .allowed_leg_ids
        .iter()
        .map(String::as_str)
        .collect();
    if !allowed_legs.is_empty() {
        let mut segment_ids: BTreeSet<String> = BTreeSet::new();
        for leg_id in allowed_legs {
            if let Some(leg) = network.legs.get(leg_id) {
                segment_ids.extend(leg.segments.iter().cloned());
            }
        }
        return segment_ids.into_iter().collect();
    }

    let allowed_depots: BTreeSet<&str> = scenario
        .allowed_depot_ids
        .iter()
        .map(String::as_str)
        .collect();
    if !allowed_depots.is_empty() {
        let mut segment_ids: BTreeSet<String> = BTreeSet::new();
        for leg in network.legs.values() {
            if allowed_depots.contains(leg.from_depot.as_str())
                || allowed_depots.contains(leg.to_depot.as_str())
            {
                segment_ids.extend(leg.segments.iter().cloned());
            }
        }
        return segment_ids.into_iter().collect();
    }

    let mut segment_ids: Vec<String> = network.segments.keys().cloned().collect();
    segment_ids.sort();
    segment_ids
}

fn build_synthetic_manual_violation_events(
    crew_ids: &[String],
    segment_ids: &[String],
    count: usize,
    start_minute: u64,
    end_minute: u64,
) -> Vec<InjectedEvent> {
    if crew_ids.is_empty() || count == 0 {
        return Vec::new();
    }

    let violation_types = [
        ViolationType::AcceptanceDelay,
        ViolationType::VoltageSagShift,
        ViolationType::LineChangeForced,
        ViolationType::Overtime8h,
        ViolationType::NightOverload,
    ];
    let time_span = end_minute.saturating_sub(start_minute) + 1;

    (0..count)
        .map(|idx| {
            let violation_type = violation_types[idx % violation_types.len()];
            let severity = match violation_type {
                ViolationType::LineChangeForced | ViolationType::Overtime8h => Severity::Critical,
                _ => Severity::Warning,
            };
            let time = start_minute + (((idx as u64) * 97) % time_span);
            let description = format!("Synthetic competition violation burst #{:06}", idx + 1);
            let segment_id =
                (!segment_ids.is_empty()).then(|| segment_ids[idx % segment_ids.len()].clone());

            InjectedEvent::ManualViolation {
                time,
                crew_id: crew_ids[idx % crew_ids.len()].clone(),
                loco_id: None,
                segment_id,
                violation_type,
                severity,
                description,
                norm_reference: "SYNTH-COMPETITION-2026".to_string(),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{
        build_bundle_comparison_html, build_bundle_comparison_markdown,
        build_bundle_comparison_report, build_bundle_report_html, build_bundle_summary_markdown,
        build_comparison_leaderboard, build_judging_packet_markdown,
        build_synthetic_manual_violation_events, classify_trace_drift, collect_scoped_segment_ids,
        comparison_rank, count_lines, sha256_hex, summarize_shoulders, BenchmarkBundleFile,
        BenchmarkBundleManifest, BenchmarkBundleSummaryContext, BenchmarkBusinessSignature,
        BenchmarkExportArtifact, BenchmarkExportMetrics, BenchmarkReport, BenchmarkReportNetwork,
        BenchmarkReportResults, BenchmarkReportScenario, BenchmarkRunResult,
        BundleComparisonMetricDeltas, BundleComparisonReport, InjectedEvent, ShoulderMetricDelta,
        ShoulderMetrics, TraceDivergence,
    };
    use rzd_core::entities::{Crew, CrewState, LocoKind, LocoState, Locomotive};
    use rzd_core::events::{make_event, EventKind, ViolationType};
    use rzd_core::network::Network;
    use rzd_core::scenario::ScenarioConfig;
    use rzd_core::violations::{Severity, Violation};
    use std::collections::BTreeMap;
    use std::collections::HashMap;
    use std::path::Path;

    #[test]
    fn synthetic_manual_violation_events_match_requested_count() {
        let crew_ids = vec!["CREW-0001".to_string(), "CREW-0002".to_string()];
        let segment_ids = vec!["SEG-001".to_string(), "SEG-002".to_string()];

        let events = build_synthetic_manual_violation_events(&crew_ids, &segment_ids, 10, 90, 120);

        assert_eq!(events.len(), 10);
        assert!(matches!(events[0], InjectedEvent::ManualViolation { .. }));
    }

    #[test]
    fn synthetic_manual_violation_events_are_deterministic() {
        let crew_ids = vec!["CREW-0001".to_string(), "CREW-0002".to_string()];
        let segment_ids = vec!["SEG-001".to_string(), "SEG-002".to_string()];

        let a = build_synthetic_manual_violation_events(&crew_ids, &segment_ids, 32, 90, 240);
        let b = build_synthetic_manual_violation_events(&crew_ids, &segment_ids, 32, 90, 240);

        let a_json = serde_json::to_string(&a).expect("events should serialize");
        let b_json = serde_json::to_string(&b).expect("events should serialize");
        assert_eq!(a_json, b_json);

        match &a[2] {
            InjectedEvent::ManualViolation {
                violation_type,
                description,
                ..
            } => {
                assert_eq!(*violation_type, ViolationType::LineChangeForced);
                assert!(description.contains("Synthetic competition violation burst"));
            }
            other => panic!("unexpected event kind: {other:?}"),
        }
    }

    #[test]
    fn count_lines_matches_newline_separated_payload() {
        assert_eq!(count_lines(""), 0);
        assert_eq!(count_lines("a"), 1);
        assert_eq!(count_lines("a\nb\n"), 2);
    }

    #[test]
    fn sha256_hex_is_stable() {
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn bundle_summary_mentions_core_artifacts() {
        let result = BenchmarkRunResult {
            profile: "one-road-competition",
            scenario: ScenarioConfig::default(),
            warmup_minutes: 0,
            shift_start_offset_min: 15,
            shift_interval_min: 1,
            injection: None,
            synthetic_manual_violations: Some(500_000),
            roads: 1,
            stations: 35,
            segments: 38,
            legs: 10,
            elapsed_ms: 10.0,
            events_processed: 100,
            events_per_second: 10.0,
            violations_total: 50,
            violations_by_type: BTreeMap::new(),
            violations_by_severity: BTreeMap::new(),
            shoulder_metrics: BTreeMap::from([(
                "A-West".to_string(),
                ShoulderMetrics {
                    events_processed: 10,
                    violations_total: 5,
                    unique_crews: 2,
                    unique_locos: 1,
                    segments_touched: 3,
                    first_event_minute: Some(10),
                    last_event_minute: Some(90),
                },
            )]),
            shifts_completed: 2,
            distance_km: 123.0,
            tu3_rows: 2,
            event_errors: 0,
            exports: BenchmarkExportMetrics {
                tu3_tsv: BenchmarkExportArtifact {
                    bytes: 10,
                    lines: 2,
                    render_ms: 1.0,
                },
                ced_json: BenchmarkExportArtifact {
                    bytes: 20,
                    lines: 3,
                    render_ms: 2.0,
                },
                violations_ndjson: BenchmarkExportArtifact {
                    bytes: 30,
                    lines: 4,
                    render_ms: 3.0,
                },
                event_trace_ndjson: BenchmarkExportArtifact {
                    bytes: 40,
                    lines: 5,
                    render_ms: 4.0,
                },
            },
            business_signature: BenchmarkBusinessSignature {
                events_processed: 100,
                violations_total: 50,
                violations_by_type: BTreeMap::new(),
                violations_by_severity: BTreeMap::new(),
                shoulder_metrics: BTreeMap::from([(
                    "A-West".to_string(),
                    ShoulderMetrics {
                        events_processed: 10,
                        violations_total: 5,
                        unique_crews: 2,
                        unique_locos: 1,
                        segments_touched: 3,
                        first_event_minute: Some(10),
                        last_event_minute: Some(90),
                    },
                )]),
                shifts_completed: 2,
                distance_km: "123.000".to_string(),
                tu3_rows: 2,
                event_errors: 0,
            },
            business_signature_json: "{}".to_string(),
            trace_signature: "trace-sig".to_string(),
        };
        let manifest = BenchmarkBundleManifest {
            schema: "rzd-des-benchmark-bundle/v1".to_string(),
            profile: "one-road-competition".to_string(),
            network_path: "data/networks/synthetic_one_road_88_05.json".to_string(),
            files: vec![
                BenchmarkBundleFile {
                    name: "report.json".to_string(),
                    sha256: "abc".to_string(),
                    bytes: 10,
                    lines: 1,
                },
                BenchmarkBundleFile {
                    name: "event_trace.ndjson".to_string(),
                    sha256: "trace".to_string(),
                    bytes: 20,
                    lines: 2,
                },
            ],
        };

        let summary = build_bundle_summary_markdown(BenchmarkBundleSummaryContext {
            manifest: &manifest,
            result: &result,
        });

        assert!(summary.contains("# Benchmark Bundle Summary"));
        assert!(summary.contains("`report.json`"));
        assert!(summary.contains("Deterministic Signature"));
        assert!(summary.contains("Shoulder Metrics"));
    }

    #[test]
    fn bundle_html_mentions_core_artifacts() {
        let result = BenchmarkRunResult {
            profile: "one-road-competition",
            scenario: ScenarioConfig::default(),
            warmup_minutes: 0,
            shift_start_offset_min: 15,
            shift_interval_min: 1,
            injection: None,
            synthetic_manual_violations: Some(500_000),
            roads: 1,
            stations: 35,
            segments: 38,
            legs: 10,
            elapsed_ms: 10.0,
            events_processed: 100,
            events_per_second: 10.0,
            violations_total: 50,
            violations_by_type: BTreeMap::new(),
            violations_by_severity: BTreeMap::new(),
            shoulder_metrics: BTreeMap::new(),
            shifts_completed: 2,
            distance_km: 123.0,
            tu3_rows: 2,
            event_errors: 0,
            exports: BenchmarkExportMetrics {
                tu3_tsv: BenchmarkExportArtifact {
                    bytes: 10,
                    lines: 2,
                    render_ms: 1.0,
                },
                ced_json: BenchmarkExportArtifact {
                    bytes: 20,
                    lines: 3,
                    render_ms: 2.0,
                },
                violations_ndjson: BenchmarkExportArtifact {
                    bytes: 30,
                    lines: 4,
                    render_ms: 3.0,
                },
                event_trace_ndjson: BenchmarkExportArtifact {
                    bytes: 40,
                    lines: 5,
                    render_ms: 4.0,
                },
            },
            business_signature: BenchmarkBusinessSignature {
                events_processed: 100,
                violations_total: 50,
                violations_by_type: BTreeMap::new(),
                violations_by_severity: BTreeMap::new(),
                shoulder_metrics: BTreeMap::new(),
                shifts_completed: 2,
                distance_km: "123.000".to_string(),
                tu3_rows: 2,
                event_errors: 0,
            },
            business_signature_json: "{}".to_string(),
            trace_signature: "trace-sig".to_string(),
        };
        let manifest = BenchmarkBundleManifest {
            schema: "rzd-des-benchmark-bundle/v1".to_string(),
            profile: "one-road-competition".to_string(),
            network_path: "data/networks/synthetic_one_road_88_05.json".to_string(),
            files: vec![
                BenchmarkBundleFile {
                    name: "report.json".to_string(),
                    sha256: "abc".to_string(),
                    bytes: 10,
                    lines: 1,
                },
                BenchmarkBundleFile {
                    name: "event_trace.ndjson".to_string(),
                    sha256: "trace".to_string(),
                    bytes: 20,
                    lines: 2,
                },
            ],
        };

        let html = build_bundle_report_html(BenchmarkBundleSummaryContext {
            manifest: &manifest,
            result: &result,
        });

        assert!(html.contains("<!doctype html>"));
        assert!(html.contains("Benchmark Bundle Report"));
        assert!(html.contains("report.json"));
    }

    #[test]
    fn judging_packet_mentions_reproducibility_artifacts() {
        let result = BenchmarkRunResult {
            profile: "one-road-competition",
            scenario: ScenarioConfig::default(),
            warmup_minutes: 0,
            shift_start_offset_min: 15,
            shift_interval_min: 1,
            injection: None,
            synthetic_manual_violations: Some(500_000),
            roads: 1,
            stations: 35,
            segments: 38,
            legs: 10,
            elapsed_ms: 10.0,
            events_processed: 100,
            events_per_second: 10.0,
            violations_total: 50,
            violations_by_type: BTreeMap::new(),
            violations_by_severity: BTreeMap::new(),
            shoulder_metrics: BTreeMap::new(),
            shifts_completed: 2,
            distance_km: 123.0,
            tu3_rows: 2,
            event_errors: 0,
            exports: BenchmarkExportMetrics {
                tu3_tsv: BenchmarkExportArtifact {
                    bytes: 10,
                    lines: 2,
                    render_ms: 1.0,
                },
                ced_json: BenchmarkExportArtifact {
                    bytes: 20,
                    lines: 3,
                    render_ms: 2.0,
                },
                violations_ndjson: BenchmarkExportArtifact {
                    bytes: 30,
                    lines: 4,
                    render_ms: 3.0,
                },
                event_trace_ndjson: BenchmarkExportArtifact {
                    bytes: 40,
                    lines: 5,
                    render_ms: 4.0,
                },
            },
            business_signature: BenchmarkBusinessSignature {
                events_processed: 100,
                violations_total: 50,
                violations_by_type: BTreeMap::new(),
                violations_by_severity: BTreeMap::new(),
                shoulder_metrics: BTreeMap::new(),
                shifts_completed: 2,
                distance_km: "123.000".to_string(),
                tu3_rows: 2,
                event_errors: 0,
            },
            business_signature_json: "{}".to_string(),
            trace_signature: "trace-sig".to_string(),
        };
        let manifest = BenchmarkBundleManifest {
            schema: "rzd-des-benchmark-bundle/v1".to_string(),
            profile: "one-road-competition".to_string(),
            network_path: "data/networks/synthetic_one_road_88_05.json".to_string(),
            files: vec![
                BenchmarkBundleFile {
                    name: "report.json".to_string(),
                    sha256: "abc".to_string(),
                    bytes: 10,
                    lines: 1,
                },
                BenchmarkBundleFile {
                    name: "event_trace.ndjson".to_string(),
                    sha256: "trace".to_string(),
                    bytes: 20,
                    lines: 2,
                },
            ],
        };

        let packet = build_judging_packet_markdown(BenchmarkBundleSummaryContext {
            manifest: &manifest,
            result: &result,
        });

        assert!(packet.contains("# Judging Packet"));
        assert!(packet.contains("manifest.json"));
        assert!(packet.contains("report.html"));
    }

    #[test]
    fn comparison_report_detects_matching_outputs() {
        let report = BenchmarkReport {
            profile: "one-road-competition".to_string(),
            network: BenchmarkReportNetwork {
                path: "n".to_string(),
                roads: 1,
                stations: 35,
                segments: 38,
                legs: 10,
            },
            scenario: BenchmarkReportScenario {
                crews: 1500,
                locos: 750,
                days: 30,
                seed: 20260708,
                warmup_minutes: 0,
                shift_start_offset_min: 15,
                shift_interval_min: 1,
                injection: None,
                synthetic_manual_violations: Some(500_000),
            },
            results: BenchmarkReportResults {
                elapsed_ms: 10.0,
                events_processed: 100,
                events_per_second: 10.0,
                violations_total: 50,
                violations_by_type: BTreeMap::new(),
                violations_by_severity: BTreeMap::new(),
                shoulder_metrics: BTreeMap::new(),
                shifts_completed: 2,
                distance_km: 123.0,
                tu3_rows: 2,
                event_errors: 0,
                trace_signature: "trace-sig".to_string(),
                exports: BenchmarkExportMetrics {
                    tu3_tsv: BenchmarkExportArtifact {
                        bytes: 10,
                        lines: 2,
                        render_ms: 1.0,
                    },
                    ced_json: BenchmarkExportArtifact {
                        bytes: 20,
                        lines: 3,
                        render_ms: 2.0,
                    },
                    violations_ndjson: BenchmarkExportArtifact {
                        bytes: 30,
                        lines: 4,
                        render_ms: 3.0,
                    },
                    event_trace_ndjson: BenchmarkExportArtifact {
                        bytes: 40,
                        lines: 5,
                        render_ms: 4.0,
                    },
                },
            },
        };
        let manifest = BenchmarkBundleManifest {
            schema: "rzd-des-benchmark-bundle/v1".to_string(),
            profile: "one-road-competition".to_string(),
            network_path: "data/networks/synthetic_one_road_88_05.json".to_string(),
            files: vec![
                BenchmarkBundleFile {
                    name: "report.json".to_string(),
                    sha256: "abc".to_string(),
                    bytes: 10,
                    lines: 1,
                },
                BenchmarkBundleFile {
                    name: "event_trace.ndjson".to_string(),
                    sha256: "trace".to_string(),
                    bytes: 20,
                    lines: 2,
                },
            ],
        };

        let comparison = build_bundle_comparison_report(
            Path::new("baseline"),
            Path::new("current"),
            &report,
            &report,
            &manifest,
            &manifest,
        );

        assert!(comparison.business_outputs_match);
        assert!(comparison.file_hashes_match);
        assert!(comparison.artifact_outputs_match);
        assert!(comparison.trace_outputs_match);
        assert!(comparison.trace_signatures_match);
        assert!(!comparison.timing_only_drift);
        assert_eq!(comparison.comparison_class, "fully_equivalent");
        assert_eq!(comparison.trace_divergence_class, "no_trace_drift");
        assert!(build_bundle_comparison_markdown(&comparison).contains("# Bundle Comparison"));
        assert!(build_bundle_comparison_html(&comparison).contains("Bundle Comparison"));
    }

    #[test]
    fn leaderboard_sorts_by_comparison_class_rank() {
        let comparisons = vec![
            BundleComparisonReport {
                schema: "rzd-des-benchmark-comparison/v1".to_string(),
                baseline_dir: "b".to_string(),
                current_dir: "c".to_string(),
                baseline_profile: "p".to_string(),
                current_profile: "p".to_string(),
                business_outputs_match: false,
                file_hashes_match: false,
                artifact_outputs_match: false,
                trace_outputs_match: false,
                trace_signatures_match: false,
                trace_event_count_delta: 0,
                trace_divergence_class: "event_kind_drift".to_string(),
                first_trace_divergence: None,
                timing_only_drift: false,
                comparison_class: "business_drift".to_string(),
                metric_deltas: BundleComparisonMetricDeltas {
                    events_processed_delta: 0,
                    violations_total_delta: 0,
                    shifts_completed_delta: 0,
                    distance_km_delta: 0.0,
                    elapsed_ms_delta: 0.0,
                    events_per_second_delta: 0.0,
                    shoulder_deltas: BTreeMap::new(),
                },
                file_deltas: vec![],
            },
            BundleComparisonReport {
                schema: "rzd-des-benchmark-comparison/v1".to_string(),
                baseline_dir: "a".to_string(),
                current_dir: "b".to_string(),
                baseline_profile: "p".to_string(),
                current_profile: "p".to_string(),
                business_outputs_match: true,
                file_hashes_match: false,
                artifact_outputs_match: true,
                trace_outputs_match: true,
                trace_signatures_match: true,
                trace_event_count_delta: 0,
                trace_divergence_class: "timing_only_trace_drift".to_string(),
                first_trace_divergence: None,
                timing_only_drift: true,
                comparison_class: "timing_only_drift".to_string(),
                metric_deltas: BundleComparisonMetricDeltas {
                    events_processed_delta: 0,
                    violations_total_delta: 0,
                    shifts_completed_delta: 0,
                    distance_km_delta: 0.0,
                    elapsed_ms_delta: 0.0,
                    events_per_second_delta: 0.0,
                    shoulder_deltas: BTreeMap::from([(
                        "A-West".to_string(),
                        ShoulderMetricDelta {
                            events_processed_delta: 1,
                            ..Default::default()
                        },
                    )]),
                },
                file_deltas: vec![],
            },
        ];

        let leaderboard = build_comparison_leaderboard(&comparisons);
        assert_eq!(comparison_rank("timing_only_drift"), 1);
        assert_eq!(
            leaderboard.comparisons[0].comparison_class,
            "timing_only_drift"
        );
        assert_eq!(
            leaderboard.comparisons[0].trace_divergence_class,
            "timing_only_trace_drift"
        );
        assert_eq!(leaderboard.comparisons[0].shoulder_drift_shoulders, 1);
    }

    #[test]
    fn classify_trace_drift_prefers_event_count_signal() {
        let divergence = TraceDivergence {
            index: 10,
            baseline_time: Some(120),
            current_time: Some(120),
            baseline_kind: Some("WaitStart".to_string()),
            current_kind: Some("WaitStart".to_string()),
            baseline_crew_id: Some("CREW-1".to_string()),
            current_crew_id: Some("CREW-1".to_string()),
            baseline_loco_id: Some("LOCO-1".to_string()),
            current_loco_id: Some("LOCO-1".to_string()),
            baseline_segment_id: Some("SEG-1".to_string()),
            current_segment_id: Some("SEG-1".to_string()),
        };
        let current = [make_event(
            120,
            EventKind::WaitStart,
            Some("CREW-1".to_string()),
            Some("LOCO-1".to_string()),
            Some("SEG-1".to_string()),
            0,
            1,
        )];

        assert_eq!(
            classify_trace_drift(Some(&[]), Some(&current), Some(&divergence)),
            "count_only_drift"
        );
    }

    #[test]
    fn classify_trace_drift_distinguishes_time_kind_and_entity_changes() {
        let timing_only = TraceDivergence {
            index: 1,
            baseline_time: Some(100),
            current_time: Some(101),
            baseline_kind: Some("ArriveStation".to_string()),
            current_kind: Some("ArriveStation".to_string()),
            baseline_crew_id: Some("CREW-1".to_string()),
            current_crew_id: Some("CREW-1".to_string()),
            baseline_loco_id: Some("LOCO-1".to_string()),
            current_loco_id: Some("LOCO-1".to_string()),
            baseline_segment_id: Some("SEG-1".to_string()),
            current_segment_id: Some("SEG-1".to_string()),
        };
        let kind_drift = TraceDivergence {
            current_kind: Some("WaitStart".to_string()),
            ..timing_only.clone()
        };
        let entity_drift = TraceDivergence {
            current_kind: timing_only.current_kind.clone(),
            current_crew_id: Some("CREW-2".to_string()),
            ..timing_only.clone()
        };

        assert_eq!(
            classify_trace_drift(Some(&[]), Some(&[]), Some(&timing_only)),
            "timing_only_trace_drift"
        );
        assert_eq!(
            classify_trace_drift(Some(&[]), Some(&[]), Some(&kind_drift)),
            "event_kind_drift"
        );
        assert_eq!(
            classify_trace_drift(Some(&[]), Some(&[]), Some(&entity_drift)),
            "entity_context_drift"
        );
    }

    #[test]
    fn summarize_shoulders_groups_events_and_violations_by_region() {
        let network = Network::from_json(
            r#"{
              "stations": {
                "ST-1": {"id":"ST-1","name":"Alpha","kind":"MainDepot","region":"A-West","coords":[0.0,0.0],"depot_id":"DEP-1","voltage_kv":"AC25000","electrification":true},
                "ST-2": {"id":"ST-2","name":"Beta","kind":"Waypoint","region":"A-West","coords":[1.0,0.0],"depot_id":null,"voltage_kv":"AC25000","electrification":true}
              },
              "segments": {
                "SEG-1": {"id":"SEG-1","from":"ST-1","to":"ST-2","distance_km":10.0,"base_speed_kmh":80.0,"gradient_factor":1.0,"failure_base_rate":0.001,"voltage_sag_prob":0.0,"single_track":false}
              },
              "legs": {
                "LEG-1": {"id":"LEG-1","name":"Alpha-Beta","from_depot":"DEP-1","to_depot":"DEP-1","segments":["SEG-1"],"total_km":10.0,"road_id":"ROAD-1"}
              },
              "roads": {
                "ROAD-1": {"id":"ROAD-1","name":"Road","stations":["ST-1","ST-2"],"depots":["DEP-1"],"legs":["LEG-1"],"region_multiplier":1.0}
              }
            }"#,
        )
        .expect("network should parse");

        let mut crews = HashMap::new();
        crews.insert(
            "CREW-1".to_string(),
            Crew {
                id: "CREW-1".to_string(),
                depot_id: "DEP-1".to_string(),
                state: CrewState::AtHome,
                shift_work_min: 0,
                shift_start: None,
                cumulative_work_min: 0,
                last_home_rest_end: None,
                night_shifts_in_row: 0,
                total_shifts: 0,
                current_leg: None,
                current_loco: None,
                position: "ST-1".to_string(),
                violations: vec![],
                acceptance_issues: vec![],
                route_segments: vec![],
                route_index: 0,
                overtime_8h_flagged: false,
                total_distance_km: 0.0,
                in_voltage_sag: false,
                personnel_number: 0,
                full_name: String::new(),
                assistant_personnel_number: 0,
                assistant_full_name: String::new(),
                qualification: None,
            },
        );
        let mut locos = HashMap::new();
        locos.insert(
            "LOCO-1".to_string(),
            Locomotive {
                id: "LOCO-1".to_string(),
                series: "E1".to_string(),
                loco_kind: LocoKind::Electric,
                sections: 2,
                depot_id: "DEP-1".to_string(),
                state: LocoState::Available,
                position: "ST-1".to_string(),
                mileage_km: 0.0,
                hours_since_to: 0.0,
                failures_total: 0,
                inventory_number: 0,
                manufacture_year: 2015,
            },
        );

        let events = vec![
            make_event(
                10,
                EventKind::ShiftAppear,
                Some("CREW-1".to_string()),
                None,
                None,
                0,
                1,
            ),
            make_event(
                20,
                EventKind::ArriveStation,
                Some("CREW-1".to_string()),
                Some("LOCO-1".to_string()),
                Some("SEG-1".to_string()),
                0,
                2,
            ),
        ];
        let violations = vec![Violation {
            id: "V-1".to_string(),
            violation_type: ViolationType::AcceptanceDelay,
            severity: Severity::Warning,
            crew_id: "CREW-1".to_string(),
            loco_id: Some("LOCO-1".to_string()),
            segment_id: Some("SEG-1".to_string()),
            detected_at: 25,
            description: "demo".to_string(),
            norm_reference: "N".to_string(),
        }];

        let metrics = summarize_shoulders(&network, &crews, &locos, &events, &violations);
        let west = metrics.get("A-West").expect("A-West metrics should exist");
        assert_eq!(west.events_processed, 2);
        assert_eq!(west.violations_total, 1);
        assert_eq!(west.unique_crews, 1);
        assert_eq!(west.unique_locos, 1);
        assert_eq!(west.segments_touched, 1);
        assert_eq!(west.first_event_minute, Some(10));
        assert_eq!(west.last_event_minute, Some(20));
    }

    #[test]
    fn collect_scoped_segment_ids_prefers_allowed_legs() {
        let network = Network::from_json(
            r#"{
              "stations": {
                "ST-1": {"id":"ST-1","name":"Alpha","kind":"MainDepot","region":"A-West","coords":[0.0,0.0],"depot_id":"DEP-1","voltage_kv":"AC25000","electrification":true},
                "ST-2": {"id":"ST-2","name":"Beta","kind":"Waypoint","region":"A-West","coords":[1.0,0.0],"depot_id":null,"voltage_kv":"AC25000","electrification":true},
                "ST-3": {"id":"ST-3","name":"Gamma","kind":"MainDepot","region":"B-Central","coords":[2.0,0.0],"depot_id":"DEP-2","voltage_kv":"DC3000","electrification":true}
              },
              "segments": {
                "SEG-1": {"id":"SEG-1","from":"ST-1","to":"ST-2","distance_km":10.0,"base_speed_kmh":80.0,"gradient_factor":1.0,"failure_base_rate":0.001,"voltage_sag_prob":0.0,"single_track":false},
                "SEG-2": {"id":"SEG-2","from":"ST-2","to":"ST-3","distance_km":10.0,"base_speed_kmh":80.0,"gradient_factor":1.0,"failure_base_rate":0.001,"voltage_sag_prob":0.0,"single_track":false}
              },
              "legs": {
                "LEG-1": {"id":"LEG-1","name":"Alpha-Beta","from_depot":"DEP-1","to_depot":"DEP-1","segments":["SEG-1"],"total_km":10.0,"road_id":"ROAD-1"},
                "LEG-2": {"id":"LEG-2","name":"Beta-Gamma","from_depot":"DEP-1","to_depot":"DEP-2","segments":["SEG-2"],"total_km":10.0,"road_id":"ROAD-1"}
              },
              "roads": {
                "ROAD-1": {"id":"ROAD-1","name":"Road","stations":["ST-1","ST-2","ST-3"],"depots":["DEP-1","DEP-2"],"legs":["LEG-1","LEG-2"],"region_multiplier":1.0}
              }
            }"#,
        )
        .expect("network should parse");

        let scenario = ScenarioConfig {
            crew_count: 1,
            loco_count: 1,
            seed: 1,
            duration_days: 1,
            shift_spread_min: 30,
            night_shift_ratio: 0.0,
            allowed_depot_ids: vec!["DEP-1".to_string()],
            allowed_leg_ids: vec!["LEG-1".to_string()],
        };

        let segment_ids = collect_scoped_segment_ids(&network, &scenario);
        assert_eq!(segment_ids, vec!["SEG-1".to_string()]);
    }
}
