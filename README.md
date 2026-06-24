# rzd-des-engine

[![CI](https://github.com/syntog/rzd-des-engine/actions/workflows/ci.yml/badge.svg)](https://github.com/syntog/rzd-des-engine/actions/workflows/ci.yml)
[![Crates.io](https://img.shields.io/crates/v/rzd-des-engine.svg)](https://crates.io/crates/rzd-des-engine)
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

## Quick Start

```bash
# Run all tests
cargo test --all-targets

# Check no warnings
cargo clippy --all-targets

# Run CLI
cargo run -p rzd-cli

# Build WASM
cd crates/rzd-wasm && wasm-pack build --target web
```

## Data

| File | Description |
|------|-------------|
| `data/networks/northern.json` | Northern Railway topology: 35 stations, 38 segments, 10 legs |
| `data/networks/northern_constants.json` | Loco series, norms, failure/event types |

## Key Features

- **DES engine** — 24 event kinds, BinaryHeap priority queue, deterministic (Mulberry32 PRNG)
- **Crew state machine** — 11 states with validated transitions per Приказ Минтранса №339
- **Violation checks** — overtime, rest, night shift, acceptance delay, voltage sag
- **CED export** — competition format for `ced.syntog.ru` (ЦЭД)
- **TU-3 export** — 73-column TSV with BOM for Excel
- **WASM integration** — full JS interop via wasm-bindgen
- **Statistics** — Welford online mean/variance, time-weighted averages, histograms

## Performance Target

**<500 ms for 100,000 events** with 100+ stations and 50+ legs.

## Project Status

Phase 1 (Core Hardening) — in progress. See [CHANGELOG](CHANGELOG.md) and [Issues](https://github.com/syntog/rzd-des-engine/issues).

## License

MIT
