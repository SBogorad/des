# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Initial project scaffold: workspace with `rzd-core`, `rzd-wasm`, `rzd-cli`
- Network module: `from_json()`, shortest path (BFS + Dijkstra), validation
- Entities: Crew (11-state machine), Locomotive, FailureKind (10), AcceptanceIssueKind
- Events: 24 EventKind variants, BinaryHeap-based EventQueue with cancellation
- Engine: all event handlers, spatial indexes, incremental distance tracking, warm-up support
- Violations: ViolationAnalytics with crew_index, summary by type/severity/crew/loco
- Statistics: Welford online mean/variance, TimeWeightedAverage, Histogram, SimulationStats
- PRNG: Mulberry32 deterministic
- TU-3: 73-column row, TSV export with BOM
- Export: tu3_to_tsv/json, violations_to_json, summary_to_json, build_ced_export (ЦЭД format)
- WASM: full bindings — load_network, load_scenario, run, get_violations_json, etc.
- Scenario generation: deterministic, crew route assignment, loco allocation
- Northern network: 35 stations, 38 segments, 10 legs, 10 depots, 3 regions
- Tests: 37 passing (module: 25, complex_simulation: 1, ced_export: 6, scenario: 5)
- Quality: zero clippy warnings
- GitHub issue templates (bug, feature, enhancement)
- CI workflow (check, test, wasm build)
