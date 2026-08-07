use clap::Parser;

pub(crate) fn parse() -> super::Cli {
    super::Cli::parse()
}

use clap::Subcommand;
use std::path::PathBuf;

#[derive(Subcommand)]
pub(crate) enum Commands {
    /// Run a full simulation
    Run {
        /// Path to network JSON file
        #[arg(short, long)]
        network: PathBuf,

        /// Path to scenario config JSON (optional, will generate if not provided)
        #[arg(long)]
        scenario: Option<PathBuf>,

        /// Number of crews (for auto-generate)
        #[arg(short = 'c', long, default_value = "50")]
        crews: usize,

        /// Number of locos (for auto-generate)
        #[arg(short = 'l', long, default_value = "20")]
        locos: usize,

        /// Simulation duration in days
        #[arg(short = 'd', long, default_value = "30")]
        days: u32,

        /// PRNG seed
        #[arg(short = 's', long, default_value = "42")]
        seed: u32,

        /// Warm-up period in minutes
        #[arg(long, default_value = "0")]
        warmup: u64,

        /// Path to injection plan JSON (optional)
        #[arg(long)]
        inject: Option<PathBuf>,

        /// Output file (stdout if not provided)
        #[arg(short, long)]
        output: Option<PathBuf>,

        /// Output format (tsv, json, ced, bundle, trace, trace-ndjson)
        #[arg(short, long, default_value = "tsv")]
        format: String,
    },

    /// Generate a scenario
    Generate {
        /// Path to network JSON file
        #[arg(short, long)]
        network: PathBuf,

        /// Number of crews
        #[arg(long, default_value = "50")]
        crews: usize,

        /// Number of locos
        #[arg(long, default_value = "20")]
        locos: usize,

        /// Simulation duration in days
        #[arg(short, long, default_value = "30")]
        days: u32,

        /// PRNG seed
        #[arg(short, long, default_value = "42")]
        seed: u32,

        /// Output file (stdout if not provided)
        #[arg(short, long)]
        output: Option<PathBuf>,
    },

    /// Validate a network file
    Validate {
        /// Path to network JSON file
        #[arg(short, long)]
        network: PathBuf,
    },

    /// Run a reproducible benchmark profile and emit JSON metrics
    Benchmark {
        /// Path to network JSON file
        #[arg(short, long)]
        network: PathBuf,

        /// Benchmark profile name
        #[arg(long, default_value = "one-road-month")]
        profile: String,

        /// Output file (stdout if not provided)
        #[arg(short, long)]
        output: Option<PathBuf>,
    },

    /// Export a contest-ready benchmark bundle with files, manifest, and SHA-256 hashes
    BenchmarkBundle {
        /// Path to network JSON file
        #[arg(short, long)]
        network: PathBuf,

        /// Benchmark profile name
        #[arg(long, default_value = "one-road-competition")]
        profile: String,

        /// Output directory for emitted files
        #[arg(short = 'o', long)]
        output_dir: PathBuf,
    },

    /// Compare two benchmark bundle directories and emit comparison artifacts
    CompareBundles {
        /// Baseline bundle directory
        #[arg(long)]
        baseline_dir: PathBuf,

        /// Current bundle directory
        #[arg(long)]
        current_dir: PathBuf,

        /// Output directory for comparison artifacts
        #[arg(short = 'o', long)]
        output_dir: PathBuf,
    },

    /// Compare two event trace files directly and emit trace-diff artifacts
    CompareTraces {
        /// Baseline trace file (JSON NDJSON emitted by event_trace.ndjson)
        #[arg(long)]
        baseline_trace: PathBuf,

        /// Current trace file (JSON NDJSON emitted by event_trace.ndjson)
        #[arg(long)]
        current_trace: PathBuf,

        /// Output directory for comparison artifacts
        #[arg(short = 'o', long)]
        output_dir: PathBuf,
    },

    /// Build an aggregate leaderboard from comparison.json artifacts
    ComparisonLeaderboard {
        /// Directory containing comparison.json files (searched recursively)
        #[arg(long)]
        input_dir: PathBuf,

        /// Output directory for leaderboard artifacts
        #[arg(short = 'o', long)]
        output_dir: PathBuf,
    },

    /// Build a full scale-study packet: competition bundle, rzd-scale bundle, comparison, and leaderboard
    ScalePacket {
        /// Path to network JSON file
        #[arg(short, long)]
        network: PathBuf,

        /// Output directory for the full packet
        #[arg(short = 'o', long)]
        output_dir: PathBuf,
    },

    /// Run a benchmark suite across multiple profiles with repeated runs
    BenchmarkSuite {
        /// Path to network JSON file
        #[arg(short, long)]
        network: PathBuf,

        /// Comma-separated profile list or "all"
        #[arg(long, default_value = "all")]
        profiles: String,

        /// Number of repeated runs per profile
        #[arg(long, default_value = "3")]
        repeats: usize,

        /// Output file (stdout if not provided)
        #[arg(short, long)]
        output: Option<PathBuf>,
    },

    /// Run until a checkpoint minute and save engine snapshot JSON
    Checkpoint {
        /// Path to network JSON file
        #[arg(short, long)]
        network: PathBuf,

        /// Path to scenario config JSON (optional, will generate if not provided)
        #[arg(long)]
        scenario: Option<PathBuf>,

        /// Number of crews (for auto-generate)
        #[arg(short = 'c', long, default_value = "50")]
        crews: usize,

        /// Number of locos (for auto-generate)
        #[arg(short = 'l', long, default_value = "20")]
        locos: usize,

        /// Simulation duration in days
        #[arg(short = 'd', long, default_value = "30")]
        days: u32,

        /// PRNG seed
        #[arg(short = 's', long, default_value = "42")]
        seed: u32,

        /// Warm-up period in minutes
        #[arg(long, default_value = "0")]
        warmup: u64,

        /// Path to injection plan JSON (optional)
        #[arg(long)]
        inject: Option<PathBuf>,

        /// Minute to stop at and save snapshot
        #[arg(long)]
        checkpoint_minute: u64,

        /// Snapshot output file
        #[arg(short, long)]
        output: PathBuf,
    },

    /// Resume a simulation from snapshot JSON and run to a new minute boundary
    Resume {
        /// Path to snapshot JSON file
        #[arg(short, long)]
        snapshot: PathBuf,

        /// Final minute to run to
        #[arg(long)]
        until_minute: u64,

        /// Output file (stdout if not provided)
        #[arg(short, long)]
        output: Option<PathBuf>,

        /// Output format (tsv, json, ced, bundle, summary, snapshot, trace, trace-ndjson)
        #[arg(short, long, default_value = "summary")]
        format: String,

        /// Optional snapshot output after resume
        #[arg(long)]
        snapshot_output: Option<PathBuf>,
    },

    /// Generate synthetic "Детализация ММ" XLSX from a simulation run
    GenerateMm {
        /// Path to network JSON file
        #[arg(short, long)]
        network: PathBuf,

        /// Path to scenario config JSON (optional, will generate if not provided)
        #[arg(long)]
        scenario: Option<PathBuf>,

        /// Number of crews (for auto-generate)
        #[arg(short = 'c', long, default_value = "50")]
        crews: usize,

        /// Number of locos
        #[arg(short = 'l', long, default_value = "20")]
        locos: usize,

        /// Simulation duration in days
        #[arg(short = 'd', long, default_value = "30")]
        days: u32,

        /// PRNG seed
        #[arg(short = 's', long, default_value = "42")]
        seed: u32,

        /// Base date for timestamps (YYYY-MM-DD)
        #[arg(long, default_value = "2026-05-01")]
        base_date: String,

        /// Output XLSX file path
        #[arg(short, long, default_value = "synthetic_mm.xlsx")]
        output: PathBuf,
    },
}
