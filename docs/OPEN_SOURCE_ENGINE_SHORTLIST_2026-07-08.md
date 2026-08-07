# Open-Source Engine Shortlist

Date: 2026-07-08

## Why this file exists

`rzd-des-engine` is aiming at a world-class synthetic DES demonstration, not
just a working local simulator. This shortlist captures the strongest open
repositories worth tracking by role.

## Tier 1: Railway domain references

### OSRD

Repository: <https://github.com/OpenRailAssociation/osrd>

Use as reference for:

- railway infrastructure modeling discipline;
- timetabling and operations framing;
- serious product packaging for rail simulation.

### NeTrainSim

Repository: <https://github.com/VTTI-CSM/NeTrainSim>

Use as reference for:

- railway-specific scenario packaging;
- public-facing simulation examples;
- research/demo credibility for rail operations.

### DesRail

Repository: <https://github.com/corryp/DesRail>

Use as reference for:

- high-performance rail-operations DES framing;
- publication-grade performance claims;
- bridge between rail modeling and throughput evidence.

## Tier 2: Rust DES/runtime maturity references

### NeXosim

Repository: <https://github.com/asynchronics/nexosim>

Use as reference for:

- large-scale Rust DES architecture;
- save/restore and event injection ideas;
- cleaner runtime contracts around simulation execution.

### MadSim

Repository: <https://github.com/madsim-rs/madsim>

Use as reference for:

- deterministic replay discipline;
- fault-injection mindset;
- benchmark and regression reproducibility.

### Tokio simulation / turmoil

Repositories:

- <https://github.com/tokio-rs/simulation>
- <https://github.com/tokio-rs/turmoil>

Use as reference for:

- seeded adversity and reproducible test cases;
- deterministic simulation tooling around complex event flows;
- better failure-story regression coverage.

## Tier 3: Scenario/model authoring references

### SimRS

Repository: <https://github.com/ndebuhr/sim>

Use as reference for:

- Rust + WASM simulation packaging;
- declarative model exchange;
- built-in analysis ergonomics.

### SimPy / SerializableSimpy

References:

- <https://simpy.readthedocs.io/>
- <https://joss.theoj.org/papers/10.21105/joss.08502.pdf>

Use as reference for:

- checkpoint/restart as a first-class capability;
- process-oriented explainability;
- eventual partition/distribution thinking.

### simmer / kalasim / JaamSim

Repositories:

- <https://github.com/r-simmer/simmer>
- <https://github.com/holgerbrandl/kalasim>
- <https://github.com/jaamsim/jaamsim>

Use as reference for:

- higher-level scenario DSL ideas;
- auditability and persistence patterns;
- reviewer-friendly model authoring and inspection.

## Tier 4: Parallel/distributed DES references

### Sim-Diasca

Repository: <https://github.com/Olivier-Boudeville-EDF/Sim-Diasca>

Use as reference for:

- long-term PDES structure;
- partitioning mindset;
- concurrency beyond one queue.

## Recommended adoption order for `rzd-des-engine`

1. NeXosim / MadSim / turmoil:
   strengthen determinism, failure injection, trace discipline.
2. OSRD / DesRail / NeTrainSim:
   strengthen rail-domain seriousness and outward benchmark story.
3. SimPy / SerializableSimpy / Sim-Diasca:
   shape the roadmap for checkpoint-heavy and eventually partitionable DES.

## Current local status

As of 2026-07-08, `rzd-des-engine` already has:

- synthetic one-road benchmark data;
- deterministic replay bundle exports;
- structured event traces;
- checkpoint/resume parity;
- trace comparison and trace drift classification;
- shoulder-aware synthetic metrics for A/B/C benchmark reporting.

That means the next frontier is not “make it simulate at all”, but “make the
simulation story deeper, more explainable, and more scalable without losing
determinism”.
