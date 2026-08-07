# rzd-des-engine Audit Summary

**Date:** 2026-07-08
**Overall Grade:** B+ (Production-Ready with Improvements)

---

## 🎯 Quick Verdict

**The engine is solid.** Clean Rust, fast simulation, reproducible benchmarks, excellent docs. But the **CLI needs refactoring** (3,850 lines in one file) and **export is slow** (27–153s vs. <20s simulation).

---

## ✅ What's Working Well

| Area | Status | Evidence |
|------|--------|----------|
| **Security** | 🟢 Clean | No `unsafe`, no leaks, minimal deps |
| **Core Engine** | 🟢 Excellent | 40–50k events/sec, deterministic, tested |
| **Benchmarks** | 🟢 Strong | 8 profiles, SHA-256 verification, reproducible |
| **Documentation** | 🟢 Excellent | 4 architecture docs, clear methodology |
| **Testing** | 🟢 Good | 37+ tests, property-based, integration |
| **Code Quality** | 🟢 Clean | Zero clippy warnings, no TODOs |

---

## ⚠️ What Needs Fixing

| Issue | Severity | Impact | Effort | Priority |
|-------|----------|--------|--------|----------|
| **Monolithic CLI** | 🟡 Medium | Hard to test/maintain | 2–3 days | **High** |
| **Export slowness** | 🟡 Medium | UX bottleneck (27–153s) | 1–2 days | **High** |
| **Dead code** | 🟡 Medium | Confusing codebase | 0.5 days | **High** |
| **Missing CI benchmarks** | 🟡 Medium | No regression detection | 1 day | Medium |
| **No SBOM** | 🟡 Medium | Contest requirement | 0.5 days | Medium |
| **Input validation** | 🟡 Medium | Security hardening | 1 day | Medium |

---

## 🔴 Top 3 Action Items

### 1. Refactor CLI (2–3 days)

**Problem:** `cli/src/main.rs` is 3,850 lines with 14 subcommands, inline logic, and embedded tests.

**Solution:**
```
cli/src/
├── main.rs              [100 LoC - arg parsing]
├── commands/            [run.rs, benchmark.rs, etc.]
├── export/              [ced.rs, tu3.rs, trace.rs]
└── util/                [progress.rs, hash.rs]
```

**Benefit:** Testable, maintainable, follows Rust best practices.

---

### 2. Optimize Export Performance (1–2 days)

**Problem:**
- CED JSON: 26–153s for 500k violations
- Event Trace: 51s for 783k events
- Simulation: only 11–19s

**Solution:** Parallel batch serialization + streaming writes

```rust
use rayon::prelude::*;
use std::io::BufWriter;

fn export_ced_parallel(violations: &[Violation], path: &Path) -> Result<()> {
    let batches: Vec<_> = violations
        .par_chunks(10_000)
        .map(|chunk| serde_json::to_value(chunk))
        .collect()?;

    let file = BufWriter::new(File::create(path)?);
    // Stream write batches...
}
```

**Target:** <5s for 500k violations (10× improvement)

---

### 3. Remove Dead Code (0.5 days)

**Found:**
- `crates/rzd-core/src/memory_pool.rs` (137 lines, unused)
- `crates/rzd-core/src/errors.rs::RzdError` (only used internally)
- `crates/rzd-core/src/whatif.rs` (291 lines, no CLI/WASM exposure)

**Action:** Delete or move to `deprecated/` with roadmap comments.

---

## 📊 Performance Summary

### Simulation (✅ Target Met)

| Profile | Events | Duration | Throughput | Target |
|---------|--------|----------|------------|--------|
| Competition | 523,900 | 11.6s | 45k/s | ✅ <500ms for 100k |
| RZD-scale | 783,726 | 19.1s | 41k/s | ✅ <500ms for 100k |

### Export (⚠️ Bottleneck)

| Export | Lines | Time | Status |
|--------|-------|------|--------|
| TU-3 TSV | 1,502 | 0.23s | ✅ Fast |
| Violations | 500k | 27.5s | ⚠️ Slow |
| CED JSON | 5M | 26.9s | ⚠️ Slow |
| Event Trace | 783k | 51.1s | ⚠️ Slow |

---

## 🔒 Security Checklist

- [x] No `unsafe` code
- [x] No credential leaks in Git
- [x] Clean dependency tree (minimal, crates.io only)
- [x] Synthetic data only (no real PII)
- [ ] Input validation (JSON schema checks)
- [ ] Path sanitization (prevent traversal)
- [ ] Dependency pinning (exact versions)
- [ ] SSH host key verification

---

## 🏗️ Architecture Issues

### CLI Structure (3,850 lines in `main.rs`)

**Contains:**
- 14 subcommands with inline implementations
- Bundle generation, comparison, leaderboard
- Export rendering (CED, TU-3, trace)
- Progress tracking, SHA-256 hashing
- Embedded tests (375 lines)

**Problems:**
- Cannot unit test commands
- Violates single responsibility
- High cognitive load

---

### Unused Code

```rust
// memory_pool.rs - 137 lines, zero usage
pub struct EventPool { ... }
pub struct ThroughputTracker { ... }

// errors.rs - RzdError enum unused (engine uses EngineError)
pub enum RzdError { ... }

// whatif.rs - 291 lines, not exposed via CLI/WASM
pub fn run_what_if(...) { ... }
```

---

## 🎪 Competition Readiness

| Requirement | Status | Action Needed |
|-------------|--------|---------------|
| Reproducible bundles | ✅ Yes | SHA-256 manifest |
| Deterministic simulation | ✅ Yes | Business signature stable |
| Scale ladder | ✅ Yes | 8 profiles (demo → RZD-scale) |
| Shoulder metrics | ✅ Yes | A/B/C breakdown |
| Trace comparison | ✅ Yes | Drift classification |
| SBOM | ❌ No | Add `cargo sbom` |
| External validator | ❌ No | Python script for hash verification |
| Build reproducibility | ⚠️ Partial | Document exact Rust/Node versions |

---

## 📈 Test Coverage

**Current:** 37+ tests across 7 files

**Coverage:**
- ✅ Core engine (handlers, state machine, violations)
- ✅ Scenario generation (property tests)
- ✅ Checkpoint/resume (snapshot parity)
- ✅ Export formats (CED, TU-3, trace)
- ❌ CLI commands (untestable in current structure)
- ❌ Bundle comparison (no property tests)
- ⚠️ WASM (1 integration test file)

**Recommendation:** Add CLI tests after refactoring.

---

## 📚 Documentation Quality

**Excellent:**
- ✅ `README.md` — comprehensive usage, benchmarks, examples
- ✅ `docs/WORLD_CLASS_DES_SCALE_STORY_2026-07-08.md` — context and evidence
- ✅ `docs/PDES_PARTITIONABILITY_AUDIT_2026-07-08.md` — roadmap analysis
- ✅ `docs/OPEN_SOURCE_ENGINE_SHORTLIST_2026-07-08.md` — reference frame
- ✅ `visualization/DEMO-RUNBOOK.md` — presentation script

**Missing:**
- ❌ JSON Schema definitions (network, scenario formats)
- ❌ Recovery runbooks (checkpoint corruption, trace divergence)
- ❌ API documentation (sparse `cargo doc` output)

---

## 🚀 Deployment

**Current:**
- PowerShell scripts (`deploy-main.ps1`, `deploy.ps1`)
- SSH to production host
- Manual builds, no CI/CD

**Issues:**
- No health check after deploy
- No rollback mechanism
- No staging environment
- No deployment notifications

**Recommendation:** Add post-deploy smoke test:
```powershell
ssh $host "curl -sf https://syntog.ru/viz/ || exit 1"
```

---

## 🎯 Recommended Timeline

### Week 1: Core Improvements
- **Day 1–3:** Refactor CLI into modules
- **Day 4–5:** Optimize export performance

### Week 2: Hardening
- **Day 1:** Add CI benchmark regression checks
- **Day 2:** Generate SBOM, external validator
- **Day 3:** Input validation, path sanitization
- **Day 4:** Remove dead code, cleanup
- **Day 5:** Documentation (schemas, recovery)

### Total Effort: ~10 working days to full production grade

---

## 🏆 Final Grades

| Category | Grade | Notes |
|----------|-------|-------|
| **Core Engine** | A | Fast, deterministic, well-tested |
| **CLI & Tooling** | C+ | Functional but needs refactoring |
| **Documentation** | A | Excellent architecture docs |
| **Testing** | B | Good coverage, needs CLI tests |
| **Performance** | B+ | Simulation fast, export slow |
| **Security** | A- | No vulnerabilities, minor hardening |
| **Competition** | B | Strong story, missing SBOM |
| **Overall** | **B+** | **Production-ready with improvements** |

---

## 💡 Key Insight

The **engine is world-class** (A-grade). The **tooling needs cleanup** (C+).

**Fix the CLI structure and export performance, and this becomes an A-grade system.**

---

**Full audit:** `AUDIT_2026-07-08.md` (585 lines)
**Next review:** After CLI refactoring (milestone 2)
