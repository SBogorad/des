# PDES Partitionability Audit

Date: 2026-07-08

## Scope

This note audits the current `rzd-des-engine` architecture as a competition-grade,
synthetic-only DES engine for one-road railway operations. The immediate target is
an external-safe one-road monthly package comparable in operational shape to
`88_05`, with a roadmap toward larger multi-road synthetic workloads.

## Current Strengths

- Deterministic seeded execution.
- Versioned scenario and trace contracts.
- Checkpoint and resume parity on the synthetic one-road benchmark.
- Reproducible benchmark bundle with outward artifact hashes.
- Direct bundle and trace comparison workflows.
- Competition-scale synthetic run already demonstrated at roughly
  `500,000` violations and more than `523,000` processed events.

These traits already place the engine in a strong position for auditability,
replay, and contest demonstration.

## Current Partitionability Limits

### 1. Single global event queue

Current engine execution is centered around a single `BinaryHeap` event queue.
That is a good fit for deterministic single-process execution, but it is not yet
partition-friendly in the PDES sense.

Impact:

- all shoulders A/B/C share one scheduler;
- cross-shoulder isolation is logical, not runtime-enforced;
- optimistic or conservative multi-partition scheduling is not yet possible.

### 2. Shared mutable operational state

Crew, locomotive, violation, and statistics state is held in shared mutable maps
inside one engine instance.

Impact:

- handlers can observe and mutate cross-entity state directly;
- synchronization boundaries for partition ownership are not explicit;
- the engine is strong for reproducible local replay, but not yet structured as
  independently advancing simulation partitions.

### 3. Cross-entity coupling in handlers

Current handler flow is centered on business correctness of crew/loco movement
and violation generation. That is the right priority today, but it means event
handlers still encode cross-entity coupling at the same layer where scheduling
is decided.

Impact:

- difficult to isolate causal channels;
- difficult to classify which messages could become partition-to-partition
  exchanges in a future PDES architecture.

### 4. Trace is strong, but partition telemetry is not yet explicit

The processed event trace is now a major asset. It gives us deterministic
forensic visibility into time, event kind, crew, loco, and segment context.

Impact:

- strong foundation for replay, drift analysis, and deterministic evidence;
- still missing explicit partition ownership markers, cross-partition edge
  counts, and rollback-free synchronization metrics.

## Comparison With Strong Open References

### NeXosim

NeXosim shows a more actor- and component-oriented shape that is naturally
closer to partitionable simulation. `rzd-des-engine` is currently ahead on
domain-specific railway outputs, but behind on execution partition structure.

### SerializableSimpy

SerializableSimpy is a good reminder that replay, serialization, and temporal
auditability matter as much as raw speed. On that axis, `rzd-des-engine` is
moving well: snapshot/resume, trace export, and bundle comparison are already
practical advantages.

### Sim-Diasca

Sim-Diasca represents the stronger end of distributed DES thinking: explicit
distributed execution concerns, synchronization, and partition governance.
`rzd-des-engine` is not there yet architecturally, but the current synthetic
one-road benchmark is the right proving ground before attempting that jump.

## Practical Reading For The One-Road Benchmark

For the synthetic `88_05`-shaped road, the current design is still the correct
default:

- one road;
- three shoulders A/B/C;
- deterministic synthetic depots, stations, and crews;
- comparable operational load and comparable violation patterns;
- competition-ready outward evidence.

In other words, we should not force PDES complexity too early. The better move
is to keep strengthening deterministic replay and then introduce partition
boundaries deliberately.

## Recommended Roadmap

### Stage 1. Shoulder-aware observability

Add metrics that show how much work belongs to each shoulder and how often
events cross shoulder boundaries.

Suggested outputs:

- events by shoulder;
- violations by shoulder;
- cross-shoulder transitions by event kind;
- per-shoulder queue pressure approximations.

### Stage 2. Explicit partition model

Introduce a lightweight partition model in data structures before introducing
parallel execution.

Suggested direction:

- map each depot, station, segment, and leg to a partition key;
- define ownership rules for crews and locos in transit;
- emit partition context into event trace export.

### Stage 3. Deterministic multi-queue prototype

Prototype multiple local queues with a deterministic merge discipline while
keeping the same business outputs and trace comparison surface.

Success criterion:

- bundle comparison remains business-equivalent;
- trace drift is explainable and classified, not opaque.

### Stage 4. PDES experiment

Only after stages 1-3 are stable, test either:

- conservative partition stepping; or
- optimistic rollback-capable experiments in a separate branch.

## Recommendation

Do not split the engine into a separate agent right now. The current highest
value work remains inside this repository:

- strengthen the one-road synthetic benchmark;
- improve deterministic diagnostics;
- make partitionability measurable before making it parallel.

That path keeps the demo story honest and gives a credible basis for later
claims about scaling toward larger synthetic RZD-like workloads.
