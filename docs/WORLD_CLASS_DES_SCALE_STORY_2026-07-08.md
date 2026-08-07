# World-Class DES Scale Story

Date: 2026-07-08

## Goal

This note ties together three things:

1. the open-source reference stack worth watching;
2. the current synthetic one-road benchmark ladder in `rzd-des-engine`;
3. the measured evidence that the engine is moving from "working prototype"
   toward a serious DES demonstration.

This is not a claim that the engine is already equal to the best open railway
or DES platforms. It is a precise claim that the current engine now has a
credible, measured growth path.

## Reference Frame

The strongest open repositories worth tracking remain:

- `OSRD`, `NeTrainSim`, `DesRail` for rail-domain seriousness;
- `NeXosim`, `MadSim`, `tokio-rs/simulation`, `turmoil` for runtime maturity,
  deterministic testing, and failure-story discipline;
- `SimPy`, `SerializableSimpy`, `simmer`, `kalasim`, `JaamSim` for
  model-authoring and persistence ideas;
- `Sim-Diasca` for long-term PDES thinking.

See also:

- `docs/OPEN_SOURCE_ENGINE_SHORTLIST_2026-07-08.md`
- `docs/PDES_PARTITIONABILITY_AUDIT_2026-07-08.md`

## What "world-class" means here

For this project, "world-class" should mean:

- fully synthetic and external-safe data;
- deterministic replayability;
- versioned scenario and trace contracts;
- measurable benchmark ladder;
- machine-readable and reviewer-readable artifacts;
- regression comparison that explains drift instead of only reporting failure;
- a credible roadmap toward larger partitionable workloads.

## Current synthetic benchmark ladder

The current one-road benchmark family now has meaningful depth:

- `one-road-demo`
- `one-road-month`
- `one-road-stress`
- `one-road-shoulder-a`
- `one-road-shoulder-b`
- `one-road-shoulder-c`
- `one-road-competition`
- `one-road-rzd-scale`

The important point is not only the number of profiles. The important point is
that the heavier profiles are no longer conceptual; they have measured outputs.

## Measured evidence

### Competition profile

Source bundle:

- `C:\Users\d88u5\AppData\Local\Temp\rzd_bundle_competition_v2`

Observed results:

- `523,900` processed events
- `500,157` violations
- `1,501` TU-3 rows
- `141,425` km simulated
- about `14.36s` simulation time
- about `36.5k` events/sec

Rendered outward artifacts:

- `ced.json`: `143,442,164` bytes, `5,018,424` lines
- `violations.ndjson`: `128,518,655` bytes, `500,157` lines
- `event_trace.ndjson`: `147,768,288` bytes, `523,900` lines

### RZD-scale one-road profile

Source bundle:

- `C:\Users\d88u5\AppData\Local\Temp\rzd_bundle_rzd_scale_v1`

Observed results:

- `783,726` processed events
- `750,168` violations
- `2,202` TU-3 rows
- `186,025` km simulated
- about `20.32s` simulation time
- about `38.6k` events/sec

Rendered outward artifacts:

- `ced.json`: `215,132,167` bytes, `7,526,594` lines
- `violations.ndjson`: `192,758,779` bytes, `750,168` lines
- `event_trace.ndjson`: `221,448,020` bytes, `783,726` lines

This is already well past a toy benchmark. The engine is now producing
multi-million-line outward evidence on a single synthetic road.

## Scale delta: competition -> rzd-scale

Comparison artifacts:

- `C:\Users\d88u5\AppData\Local\Temp\rzd_bundle_comp_vs_scale`
- `C:\Users\d88u5\AppData\Local\Temp\rzd_bundle_comp_vs_scale_leaderboard_v6`

Measured deltas:

- `+259,826` processed events
- `+250,011` violations
- `+701` completed shifts
- `+44,600` km simulated
- about `+5.96s` simulation time

Trace classification:

- `comparison_class`: `business_drift`
- `trace_divergence_class`: `count_only_drift`

The drift is expected here because this is not a same-profile replay; it is an
intentional scale-up of the workload.

## Shoulder-aware growth

The newer comparison flow now explains how scale changes distribute across the
synthetic road instead of hiding that difference inside one aggregate total.

Measured shoulder deltas from `competition` to `rzd-scale`:

- `A-West`: `+95,323` events, `+92,117` violations
- `B-Central`: `+83,037` events, `+78,952` violations
- `C-East`: `+81,466` events, `+78,942` violations

That matters because it shows the synthetic network is not only "bigger". It
is being tracked as a structured road with inspectable internal growth.

## Why this matters relative to open-source references

### Relative to OSRD / NeTrainSim / DesRail

`rzd-des-engine` is still narrower in rail-domain breadth, but it now has a
much stronger benchmark-and-evidence story than a basic prototype:

- safe synthetic road;
- reproducible benchmark ladder;
- reviewer-facing bundles;
- explicit comparison artifacts.

### Relative to NeXosim / MadSim / turmoil

`rzd-des-engine` is now meaningfully aligned with the traits that make mature
simulation engines believable:

- deterministic signatures;
- trace export;
- comparison tooling;
- regression evidence;
- failure-story and trace drift surfaces.

### Relative to Sim-Diasca / SerializableSimpy

It is not yet a PDES engine, but it now has:

- snapshot/resume;
- replayability;
- shoulder-aware structure;
- a clear place to attach partition metrics next.

## Honest current limits

The engine is not yet "world-class" in the strongest possible sense.

Remaining gaps include:

- richer rail-domain planning depth;
- explicit partition ownership and PDES scheduling;
- cleaner separation of execution core vs. benchmark/reporting code;
- broader property/regression coverage on heavy profiles;
- stronger operator/reviewer surfaces around the generated artifacts.

## Best next steps

1. Add a scale-study packet generator that packages:
   `competition`, `rzd-scale`, their comparison, and the leaderboard together.
2. Add partition-readiness metrics:
   cross-shoulder transitions, queue pressure, and ownership boundaries.
3. Add repeatability checks for `one-road-rzd-scale`, at least in reduced form.
4. Move more comparison/report logic out of `cli/src/main.rs` into dedicated
   modules once the benchmark surface stabilizes.

## Bottom line

The engine is no longer only "fast Rust code on synthetic data".

It is now a measured synthetic DES system with:

- an open-source reference frame;
- a benchmark ladder;
- comparison evidence;
- shoulder-aware forensic analysis;
- and a heavy one-road workload that already emits multi-million-line artifacts.

That is a credible path toward a world-class demonstration, and it is grounded
in current evidence rather than aspiration.
