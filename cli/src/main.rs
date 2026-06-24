use clap::{Parser, Subcommand};
use rzd_core::network::Network;
use rzd_core::scenario::{generate_scenario, ScenarioConfig};
use rzd_core::engine::Engine;
use rzd_core::export::Export;
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "rzd-cli", about = "RZD-DES-ENGINE simulation runner")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
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

        /// Output file (stdout if not provided)
        #[arg(short, long)]
        output: Option<PathBuf>,

        /// Output format (tsv, json, ced)
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
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Run { network, scenario, crews, locos, days, seed, warmup, output, format } => {
            let network_json = std::fs::read_to_string(&network)
                .map_err(|e| format!("Failed to read network file '{}': {}", network.display(), e))?;
            let net = Network::from_json(&network_json)?;

            let mut engine = Engine::new(net, seed);

            if let Some(scenario_path) = scenario {
                let scenario_json = std::fs::read_to_string(&scenario_path)
                    .map_err(|e| format!("Failed to read scenario file '{}': {}", scenario_path.display(), e))?;
                let config: ScenarioConfig = serde_json::from_str(&scenario_json)?;
                let gen = generate_scenario(&engine.network, config);
                for c in gen.crews { engine.add_crew(c); }
                for l in gen.locos { engine.add_loco(l); }
            } else {
                let config = ScenarioConfig { crew_count: crews, loco_count: locos, seed, duration_days: days };
                let gen = generate_scenario(&engine.network, config);
                for c in gen.crews { engine.add_crew(c); }
                for l in gen.locos { engine.add_loco(l); }
            }

            engine.set_warmup(warmup);
            engine.schedule_all_shifts(60, 10);
            let until = (days as u64) * 1440;
            engine.run(until);

            let result = match format.as_str() {
                "tsv" => Export::tu3_to_tsv(&engine.tu3_rows),
                "json" => Export::tu3_to_json(&engine.tu3_rows)?,
                "ced" => serde_json::to_string_pretty(&engine.export_ced())?,
                "summary" => {
                    let summary = engine.violations_summary();
                    serde_json::to_string_pretty(&summary)?
                }
                _ => return Err(format!("Unknown format: {}. Use 'tsv', 'json', 'ced', or 'summary'", format).into()),
            };

            if let Some(out_path) = output {
                std::fs::write(&out_path, &result)?;
                eprintln!("Written to {}", out_path.display());
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

        Commands::Generate { network, crews, locos, days, seed, output } => {
            let network_json = std::fs::read_to_string(&network)?;
            let net = Network::from_json(&network_json)?;

            let config = ScenarioConfig { crew_count: crews, loco_count: locos, seed, duration_days: days };
            let scenario = generate_scenario(&net, config);

            let json = serde_json::to_string_pretty(&scenario)?;

            if let Some(out_path) = output {
                std::fs::write(&out_path, &json)?;
                eprintln!("Written to {}", out_path.display());
            } else {
                println!("{}", json);
            }

            eprintln!("Generated: {} crews, {} locos", scenario.crews.len(), scenario.locos.len());
        }

        Commands::Validate { network } => {
            let network_json = std::fs::read_to_string(&network)?;
            match Network::from_json(&network_json) {
                Ok(net) => {
                    let errors = net.validate();
                    if errors.is_empty() {
                        println!("Network is valid: {} stations, {} segments, {} legs, {} roads",
                            net.stations.len(), net.segments.len(), net.legs.len(), net.roads.len());
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
    }

    Ok(())
}
