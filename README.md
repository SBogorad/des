# DES Engine

[![CI](https://github.com/xodapi/des/actions/workflows/ci.yml/badge.svg)](https://github.com/xodapi/des/actions/workflows/ci.yml)
[![License](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

**Rust/WASM Discrete Event Simulation engine for Russian Railway (РЖД) locomotive crew operations.**

Replaces legacy TypeScript SPA (`stats.syntog.ru` v3.0) with 50–100× performance gains.

## Architecture

```
┌─────────────────────────────────────────────────┐
│                   TypeScript Host                │
│              (Vite + React / CLI)                │
├──────────────────────┬──────────────────────────┤
│    rzd-wasm (WASM)   │    rzd-cli (native)      │
│   wasm-bindgen glue  │   clap-based runner       │
├──────────────────────┴──────────────────────────┤
│                  rzd-core                        │
│  ┌────────┬───────┬──────┬──────┬────────────┐  │
│  │ Engine │Events │Crew  │Loco  │ Statistics │  │
│  ├────────┼───────┼──────┼──────┼────────────┤  │
│  │Network │Export │TU-3  │PRNG  │ Violations │  │
│  └────────┴───────┴──────┴──────┴────────────┘  │
└─────────────────────────────────────────────────┘
```

## Demo and 60-second verification

The engine uses synthetic railway scenarios only. From a checkout with Rust installed:

```bash
cargo check --all-targets
cargo run -p rzd-cli -- --help
cargo run -p rzd-cli -- validate --network data/networks/synthetic_one_road_88_05.json
```

For the browser visualization and a reproducible demo walkthrough, see [the visualization runbook](visualization/DEMO-RUNBOOK.md). Repository rules are documented in [CONTRIBUTING.md](CONTRIBUTING.md), [SECURITY.md](SECURITY.md), and the [documentation index](docs/README.md).
## Quick Start

```bash
# Run all tests
cargo test --all-targets

# Check no warnings
cargo clippy --all-targets -- -D warnings

# Run CLI
cargo run -p rzd-cli

# Run with reproducible scripted disruptions
cargo run -p rzd-cli -- run --network data/networks/northern.json --crews 8 --locos 4 --days 2 --format bundle --inject data/injection/sample_fault_windows.json

# Run from declarative scenario JSON
cargo run -p rzd-cli -- run --network data/networks/synthetic_one_road_88_05.json --scenario data/scenarios/one_road_shoulder_b.json --format summary

# Emit a structured event trace for deterministic forensic/debug analysis
cargo run -p rzd-cli -- run --network data/networks/synthetic_one_road_88_05.json --scenario data/scenarios/one_road_shoulder_a.json --format trace

# Validate the one-road synthetic benchmark network (88_05)
cargo run -p rzd-cli -- validate --network data/networks/synthetic_one_road_88_05.json

# Generate a one-road synthetic month aligned to shoulders A/B/C
cargo run -p rzd-cli -- generate --network data/networks/synthetic_one_road_88_05.json --crews 18 --locos 12 --days 30 --seed 20260708

# Replay a short deterministic disruption scenario on the same road
cargo run -p rzd-cli -- run --network data/networks/synthetic_one_road_88_05.json --crews 12 --locos 12 --days 2 --seed 42 --format bundle --inject data/injection/sample_fault_windows.json

# Emit a reproducible benchmark report for the synthetic road
cargo run -p rzd-cli -- benchmark --network data/networks/synthetic_one_road_88_05.json --profile one-road-demo
cargo run -p rzd-cli -- benchmark --network data/networks/synthetic_one_road_88_05.json --profile one-road-month
cargo run -p rzd-cli -- benchmark --network data/networks/synthetic_one_road_88_05.json --profile one-road-stress
cargo run -p rzd-cli -- benchmark --network data/networks/synthetic_one_road_88_05.json --profile one-road-shoulder-a
cargo run -p rzd-cli -- benchmark --network data/networks/synthetic_one_road_88_05.json --profile one-road-shoulder-b
cargo run -p rzd-cli -- benchmark --network data/networks/synthetic_one_road_88_05.json --profile one-road-shoulder-c
cargo run -p rzd-cli -- benchmark --network data/networks/synthetic_one_road_88_05.json --profile one-road-competition
cargo run -p rzd-cli -- benchmark --network data/networks/synthetic_one_road_88_05.json --profile one-road-rzd-scale

# Emit a repeated benchmark suite across profiles
cargo run -p rzd-cli -- benchmark-suite --network data/networks/synthetic_one_road_88_05.json --profiles all --repeats 3
cargo run -p rzd-cli -- benchmark-suite --network data/networks/synthetic_one_road_88_05.json --profiles one-road-competition --repeats 2
cargo run -p rzd-cli -- benchmark-suite --network data/networks/synthetic_one_road_88_05.json --profiles one-road-rzd-scale --repeats 1

# Emit a contest-ready bundle directory with hashes and manifest
cargo run -p rzd-cli -- benchmark-bundle --network data/networks/synthetic_one_road_88_05.json --profile one-road-competition --output-dir out/one-road-competition-bundle
cargo run -p rzd-cli -- benchmark-bundle --network data/networks/synthetic_one_road_88_05.json --profile one-road-rzd-scale --output-dir out/one-road-rzd-scale-bundle

# Build a full scale packet with competition, rzd-scale, comparison, and leaderboard
cargo run -p rzd-cli -- scale-packet --network data/networks/synthetic_one_road_88_05.json --output-dir out/scale-packet

# Compare two bundle directories
cargo run -p rzd-cli -- compare-bundles --baseline-dir out/run-a --current-dir out/run-b --output-dir out/comparison

# Compare two trace files directly
cargo run -p rzd-cli -- compare-traces --baseline-trace out/run-a/event_trace.ndjson --current-trace out/run-b/event_trace.ndjson --output-dir out/trace-comparison

# Build an aggregate leaderboard from comparison outputs
cargo run -p rzd-cli -- comparison-leaderboard --input-dir out/comparisons-root --output-dir out/leaderboard

# Save and resume a synthetic-road checkpoint
cargo run -p rzd-cli -- checkpoint --network data/networks/synthetic_one_road_88_05.json --crews 24 --locos 12 --days 5 --seed 20260708 --checkpoint-minute 300 --output checkpoint_88_05.json
cargo run -p rzd-cli -- resume --snapshot checkpoint_88_05.json --until-minute 7200 --format summary

# Build WASM
cd crates/rzd-wasm && wasm-pack build --target web
```

## Data

| File | Description |
|------|-------------|
| `data/networks/northern.json` | Northern Railway topology: 35 stations, 38 segments, 10 legs |
| `data/networks/synthetic_one_road_88_05.json` | Synthetic one-road benchmark package: 35 stations, 38 segments, 10 legs, 3 operating shoulders (A/B/C) |
| `data/networks/northern_constants.json` | Loco series, norms, failure/event types |
| `data/scenarios/one_road_month.json` | Declarative baseline one-road month scenario |
| `data/scenarios/one_road_shoulder_*.json` | Declarative shoulder-scoped one-road month scenarios |

## Synthetic One-Road Benchmark

`synthetic_one_road_88_05.json` is the external-safe demo network for one synthetic road.

- No real station names or depot names
- One road with three comparable operating shoulders: `A-West`, `B-Central`, `C-East`
- Mixed electric / dual / diesel traction zones
- Sized to resemble a single-road monthly package such as `88_05`
- Intended for replayable demos, synthetic violation generation, and future large-scale benchmarks

Benchmark profiles currently available through `rzd-cli benchmark`:

- `one-road-demo` — short deterministic run with scripted disruptions
- `one-road-month` — baseline one-road synthetic month
- `one-road-stress` — denser same-road stress profile for throughput checks
- `one-road-shoulder-a` — western shoulder month (`SD-1`, `SD-2`, `TD-72`; legs `LEG-01..03`)
- `one-road-shoulder-b` — central shoulder month (`SD-3`, `SD-5`, `TD-74`, `SD-7`; legs `LEG-04..06`, `LEG-10`)
- `one-road-shoulder-c` — eastern shoulder month (`TD-71`, `TD-75`, `TD-77`; legs `LEG-07..09`)
- `one-road-competition` — competition-scale synthetic month with deterministic burst injection of `500,000` manual violations on the same one-road network
- `one-road-rzd-scale` — heavier synthetic one-road month with `750,000` manual violations and denser crew/loco load, intended as a stress step beyond the contest profile

Declarative scenario JSON examples currently available in `data/scenarios/`:

- `one_road_month.json`
- `one_road_shoulder_a.json`
- `one_road_shoulder_b.json`
- `one_road_shoulder_c.json`

These scenario files now use the versioned contract schema
`rzd-des-scenario/v1`. The CLI still accepts legacy bare `ScenarioConfig`
JSON for backward compatibility, but the versioned wrapper is the preferred
long-term replay and exchange format.

Measured evidence on `2026-07-08`:

- `one-road-competition` processed `523,900` events and produced `500,157` violations on `synthetic_one_road_88_05.json`
- `one-road-rzd-scale` processed `783,726` events and produced `750,168` violations on `synthetic_one_road_88_05.json`
- the single-run benchmark completed in about `11.6s` on the current workstation
- the `one-road-rzd-scale` simulation run completed in about `19.1s` on the current workstation
- repeated suite runs (`repeats=2`) preserved one unique business signature while wall-clock timing varied
- benchmark export rendering on the same run produced:
  - `violations_ndjson`: `500,157` lines, `128,518,655` bytes, about `27.5s` render time
  - `CED JSON`: `5,018,424` lines, `143,442,164` bytes, about `153.7s` render time
  - `TU-3 TSV`: `1,502` lines, `536,366` bytes, about `0.23s` render time
- benchmark export rendering on `one-road-rzd-scale` produced:
  - `violations_ndjson`: `750,168` lines, `192,758,779` bytes, about `42.3s` render time
  - `CED JSON`: `7,526,594` lines, `215,132,167` bytes, about `48.8s` render time
  - `event_trace.ndjson`: `783,726` lines, `221,448,020` bytes, about `51.1s` render time
  - `TU-3 TSV`: `2,203` lines, `784,916` bytes, about `0.37s` render time
- after indexing violations by `crew_id` inside CED export on `2026-07-08`, the same `CED JSON` artifact dropped to about `26.9s` render time on the competition profile while preserving identical outward bytes and lines
- `benchmark-bundle` now emits a reproducible directory containing `report.json`, `manifest.json`, `tu3.tsv`, `ced.json`, and `violations.ndjson`
- `benchmark-bundle` now also emits `event_trace.ndjson`, a structured processed-event history for deterministic forensic comparison
- the bundle manifest records SHA-256, bytes, and line counts for each outward artifact
- bundle comparison now evaluates the event trace artifact together with the other outward files via the shared manifest workflow
- bundle comparison now also reports shoulder-aware deltas, so drift across `A-West`, `B-Central`, and `C-East` can be inspected directly instead of inferred from aggregate totals
- `benchmark-bundle` also emits `SUMMARY.md`, a human-readable competition summary with key metrics, export sizes, render times, and the deterministic business signature
- `benchmark-bundle` also emits standalone `report.html`, an offline demo view for external reviewers that opens directly from the bundle directory
- `benchmark-bundle` also emits `JUDGING_PACKET.md`, a concise contest-facing packet with the problem statement, scale metrics, artifact inventory, and reproducibility notes
- `compare-bundles` emits `comparison.json`, `comparison.md`, and `comparison.html` for baseline-vs-current evidence across two benchmark bundle directories
- `compare-traces` emits `trace_comparison.json`, `trace_comparison.md`, and `trace_comparison.html` for direct event-trace forensic comparison
- comparison output now distinguishes `fully_equivalent`, `timing_only_drift`, `business_equivalent_only`, and `business_drift`, so regressions can be separated from harmless timing noise
- trace comparison now also reports `trace_signatures_match`, `trace_event_count_delta`, a `trace_divergence_class`, and the first divergence point when traces differ
- `comparison-leaderboard` emits aggregate `leaderboard.json`, `leaderboard.md`, and `leaderboard.html` across multiple comparison artifacts
- `scale-packet` emits a ready-to-review directory containing competition and RZD-scale bundles plus their comparison, leaderboard, a top-level `SCALE_PACKET.md`, `SCALE_PACKET.html`, `MANIFEST_SCALE.json`, `packet_summary.json`, and `packet_summary.tsv`

Benchmark suite surface currently available through `rzd-cli`:

- `benchmark-suite` — repeated runs across one or more profiles with aggregate latency and throughput statistics
- each run now emits a stable `business_signature` built from business results instead of wall-clock fields
- suite reports now include a `determinism` section with `unique_business_signatures` and `business_outputs_stable_across_repeats`
- on `2026-07-08`, `synthetic_one_road_88_05.json` validated stable business outputs across repeats for `one-road-demo`, `one-road-month`, and `one-road-stress`; suite variance was limited to wall-clock timing
- `one-road-competition` is intentionally explicit rather than part of `--profiles all`, so large competition-scale runs stay opt-in
- `one-road-rzd-scale` is also intentionally opt-in and excluded from `--profiles all`, so the heaviest synthetic-road runs remain deliberate

Checkpoint surface currently available through `rzd-cli`:

- `checkpoint` — run until a target minute and save `rzd-des-engine-snapshot/v1`
- `resume` — restore snapshot JSON and continue the run to a new minute boundary
- on `2026-07-08`, a checkpoint boundary bug that dropped the first post-checkpoint event was fixed; JSON snapshot roundtrip resume now matches the full synthetic-road run business results in automated tests

## Key Features

- **DES engine** — 24 event kinds, BinaryHeap priority queue, deterministic (Mulberry32 PRNG)
- **Crew state machine** — 11 states with validated transitions per Приказ Минтранса №339
- **Violation checks** — overtime, rest, night shift, acceptance delay, voltage sag
- **CED export** — competition format for `ced.syntog.ru` (ЦЭД)
- **Replay bundle export** — versioned JSON bundle with network, scenario, and run results for deterministic reruns
- **Structured event trace** — versioned `rzd-des-event-trace/v1` export for deterministic forensic analysis and replay debugging
- **Scripted disruption injection** — versioned JSON plan for waits, failures, voltage sag windows, and manual violations
- **Engine snapshot contract** — versioned JSON-safe engine state snapshot with restore of serialized runtime state and validated business-parity resume coverage on the synthetic road
- **TU-3 export** — 73-column TSV with BOM for Excel
- **WASM integration** — full JS interop via wasm-bindgen
- **Statistics** — Welford online mean/variance, time-weighted averages, histograms

## Performance Target

**<500 ms for 100,000 events** with 100+ stations and 50+ legs.

## Project Status

Phase 1 (Core Hardening) — in progress. See [CHANGELOG](CHANGELOG.md) and [Issues](https://github.com/xodapi/des/issues).

Supporting architecture notes:

- [PDES partitionability audit](docs/PDES_PARTITIONABILITY_AUDIT_2026-07-08.md)
- [Open-source engine shortlist](docs/OPEN_SOURCE_ENGINE_SHORTLIST_2026-07-08.md)
- [World-class DES scale story](docs/WORLD_CLASS_DES_SCALE_STORY_2026-07-08.md)

## License

[MIT License](LICENSE)
