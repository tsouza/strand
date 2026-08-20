// Copyright the STRAND authors.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! The RFC 0002 §2 Workflow II DST cross-validation harness: TLC generates
//! real action sequences from `verification/manifest.tla` via its
//! `-simulate ... file=` random-simulation trace-file output (the real,
//! working mechanism this harness picked after checking `tla2tools.jar
//! -help`'s actual flag list — see `docs/roadmap.md`'s M3-3 entry and
//! `rfcs/0002-manifest-formal-verification.md`'s Discussion section for why
//! this mode and not `-dump` or a hand-rolled trace format), and this
//! binary replays each generated trace directly against the real
//! `strand_core::manifest` code (`replay.rs`), reporting any drift per RFC
//! 0002 §3's four-way classification.
//!
//! DST's "a run is a seed, and a seed is exactly replayable" property: the
//! only randomness anywhere in this pipeline is TLC's own `-seed`, printed
//! in every report and defaulted to a fixed literal so a bare invocation is
//! itself reproducible; this harness's own replay logic (`replay.rs`) is
//! fully deterministic given a trace file, so re-running with the same
//! seed (and the same `-workers` count — TLC splits its RNG stream across
//! workers, so reproducing a run exactly requires pinning both) regenerates
//! byte-identical traces and therefore byte-identical verdicts.
//!
//! Usage:
//!   `cargo run -p strand-core --bin dst-manifest-harness -- [--jar PATH]
//!   [--seed N] [--num-per-worker N] [--workers N] [--depth N]
//!   [--traces-dir DIR]`
//!
//! `--traces-dir` skips invoking TLC and replays an already-generated
//! directory instead (e.g. one produced by a prior run, or by the exact
//! `java` command this binary itself prints before running it).

mod replay;
mod trace;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

struct Args {
    jar: PathBuf,
    manifest_tla: PathBuf,
    manifest_cfg: PathBuf,
    seed: i64,
    workers: u32,
    num_per_worker: u32,
    depth: u32,
    traces_dir: Option<PathBuf>,
}

fn repo_root() -> PathBuf {
    // CARGO_MANIFEST_DIR is crates/strand-core at compile time.
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("crates/strand-core has a workspace root two levels up")
        .to_path_buf()
}

fn parse_args() -> Args {
    let root = repo_root();
    let mut jar = dirs_home_cache_tlaplus();
    let mut manifest_tla = root.join("verification/manifest.tla");
    let mut manifest_cfg = root.join("verification/manifest.cfg");
    // Fixed by default so a bare run is reproducible without the caller
    // having to know to pass anything (DST's own "a run is a seed" rule).
    let mut seed: i64 = 20260819;
    let mut workers: u32 = 1;
    let mut num_per_worker: u32 = 200;
    let mut depth: u32 = 30;
    let mut traces_dir = None;

    let mut it = std::env::args().skip(1);
    while let Some(flag) = it.next() {
        let mut val = || it.next().unwrap_or_else(|| panic!("{flag} needs a value"));
        match flag.as_str() {
            "--jar" => jar = PathBuf::from(val()),
            "--manifest-tla" => manifest_tla = PathBuf::from(val()),
            "--manifest-cfg" => manifest_cfg = PathBuf::from(val()),
            "--seed" => seed = val().parse().expect("--seed is an integer"),
            "--workers" => workers = val().parse().expect("--workers is an integer"),
            "--num-per-worker" => {
                num_per_worker = val().parse().expect("--num-per-worker is an integer")
            }
            "--depth" => depth = val().parse().expect("--depth is an integer"),
            "--traces-dir" => traces_dir = Some(PathBuf::from(val())),
            other => panic!("unknown flag {other}"),
        }
    }

    Args {
        jar,
        manifest_tla,
        manifest_cfg,
        seed,
        workers,
        num_per_worker,
        depth,
        traces_dir,
    }
}

fn dirs_home_cache_tlaplus() -> PathBuf {
    let home = std::env::var("HOME").expect("HOME is set");
    PathBuf::from(home).join(".cache/tlaplus/tla2tools.jar")
}

/// Extracts `DeleteWriter = wN` from `manifest.cfg` — the one CONSTANT this
/// harness needs to know externally, since `commit_deletion_vector`'s
/// distinguishing real-code shape (never appending) can't be told apart
/// from `commit`'s from pc transitions alone. A missing/unparseable line is
/// a hard error: replaying every writer as append-shaped against a
/// DeleteWriter's real revise-shaped trajectory would silently produce
/// false drift, which is worse than failing loudly.
fn extract_delete_writer(cfg_text: &str) -> String {
    for line in cfg_text.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix("DeleteWriter") {
            let rest = rest.trim_start();
            if let Some(rest) = rest.strip_prefix('=') {
                return rest.trim().to_string();
            }
        }
    }
    panic!(
        "manifest.cfg has no `DeleteWriter = ...` line — cannot tell which writer is revise-shaped"
    );
}

fn run_tlc(args: &Args, out_dir: &Path) {
    std::fs::create_dir_all(out_dir).expect("create trace output directory");
    let prefix = out_dir.join("tr");
    let simulate_arg = format!(
        "num={},file={}",
        args.num_per_worker,
        prefix.to_str().expect("trace path prefix is valid UTF-8")
    );
    let mut cmd = Command::new("java");
    cmd.arg("-XX:+UseParallelGC")
        .arg("-jar")
        .arg(&args.jar)
        .arg("-workers")
        .arg(args.workers.to_string())
        .arg("-config")
        .arg(&args.manifest_cfg)
        .arg("-simulate")
        .arg(&simulate_arg)
        .arg("-depth")
        .arg(args.depth.to_string())
        .arg("-seed")
        .arg(args.seed.to_string())
        .arg(&args.manifest_tla);

    println!("Generating traces (Workflow II, RFC 0002 §2):");
    println!(
        "  java -XX:+UseParallelGC -jar {} -workers {} -config {} -simulate {} -depth {} \
         -seed {} {}",
        args.jar.display(),
        args.workers,
        args.manifest_cfg.display(),
        simulate_arg,
        args.depth,
        args.seed,
        args.manifest_tla.display()
    );
    println!(
        "  (this exact command line regenerates byte-identical traces; re-running it, and \
         this binary, is the replay of this same DST seed)"
    );

    let status = cmd.status().expect("java is on PATH and the jar exists");
    if !status.success() {
        panic!("TLC exited with {status}; see its output above");
    }
}

#[derive(Default)]
struct Counts {
    matched: u32,
    skipped: u32,
    drift: u32,
}

impl Counts {
    fn record(&mut self, v: &replay::Verdict) {
        match v {
            replay::Verdict::Matched => self.matched += 1,
            replay::Verdict::Skipped(_) => self.skipped += 1,
            replay::Verdict::Drift(_) => self.drift += 1,
        }
    }
}

fn main() {
    let args = parse_args();

    if !args.jar.exists() {
        eprintln!(
            "tla2tools.jar not found at {} — fetch it per verification/README.md \
             (curl -LO https://github.com/tlaplus/tlaplus/releases/latest/download/tla2tools.jar) \
             or pass --jar",
            args.jar.display()
        );
        std::process::exit(2);
    }

    let cfg_text = std::fs::read_to_string(&args.manifest_cfg)
        .unwrap_or_else(|e| panic!("reading {}: {e}", args.manifest_cfg.display()));
    let delete_writer = extract_delete_writer(&cfg_text);

    let out_dir: PathBuf = match &args.traces_dir {
        Some(d) => d.clone(),
        None => {
            let dir = std::env::temp_dir().join(format!("dst-manifest-harness-seed{}", args.seed));
            run_tlc(&args, &dir);
            dir
        }
    };
    let trace_dir = out_dir.as_path();

    let mut trace_files: Vec<PathBuf> = std::fs::read_dir(trace_dir)
        .unwrap_or_else(|e| panic!("reading trace dir {}: {e}", trace_dir.display()))
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with("tr_"))
        })
        .collect();
    trace_files.sort();

    if trace_files.is_empty() {
        eprintln!("no trace files found under {}", trace_dir.display());
        std::process::exit(2);
    }

    let mut parse_failures: Vec<(PathBuf, String)> = Vec::new();
    let mut replay_failures: Vec<(PathBuf, String)> = Vec::new();
    let mut writer_counts = Counts::default();
    let mut reader_counts = Counts::default();
    let mut drift_details: Vec<String> = Vec::new();
    let mut skip_reasons: BTreeMap<String, u32> = BTreeMap::new();
    let mut total_states = 0usize;
    let mut total_writer_trajectories = 0u32;
    let mut total_reader_trajectories = 0u32;
    let mut writer_fault_trajectories = 0u32;
    let mut reader_fault_trajectories = 0u32;

    for path in &trace_files {
        let text = std::fs::read_to_string(path)
            .unwrap_or_else(|e| panic!("reading {}: {e}", path.display()));
        let states = match trace::parse_trace_file(&text) {
            Ok(s) => s,
            Err(e) => {
                parse_failures.push((path.clone(), e.to_string()));
                continue;
            }
        };
        total_states += states.len();

        let result = match replay::replay_trace(&states, &delete_writer) {
            Ok(r) => r,
            Err(e) => {
                replay_failures.push((path.clone(), e));
                continue;
            }
        };

        for (w, has_fault, verdict) in &result.writers {
            total_writer_trajectories += 1;
            if *has_fault {
                writer_fault_trajectories += 1;
            }
            writer_counts.record(verdict);
            match verdict {
                replay::Verdict::Drift(msg) => {
                    drift_details.push(format!("{}: writer {w}: {msg}", path.display()));
                }
                replay::Verdict::Skipped(reason) => {
                    *skip_reasons.entry(reason.clone()).or_default() += 1;
                }
                replay::Verdict::Matched => {}
            }
        }
        for (r, has_fault, verdict) in &result.readers {
            total_reader_trajectories += 1;
            if *has_fault {
                reader_fault_trajectories += 1;
            }
            reader_counts.record(verdict);
            match verdict {
                replay::Verdict::Drift(msg) => {
                    drift_details.push(format!("{}: reader {r}: {msg}", path.display()));
                }
                replay::Verdict::Skipped(reason) => {
                    *skip_reasons.entry(reason.clone()).or_default() += 1;
                }
                replay::Verdict::Matched => {}
            }
        }
    }

    println!();
    println!("==================== DST cross-validation report ====================");
    println!(
        "Seed: {} | workers: {} | traces requested per worker: {} | depth: {}",
        args.seed, args.workers, args.num_per_worker, args.depth
    );
    println!("Trace files found: {}", trace_files.len());
    println!(
        "Trace files parsed successfully: {}",
        trace_files.len() - parse_failures.len()
    );
    println!("Total states across all parsed traces: {total_states}");
    println!();
    println!(
        "Writer trajectories replayed: {total_writer_trajectories} (matched {}, skipped {}, DRIFT {})",
        writer_counts.matched, writer_counts.skipped, writer_counts.drift
    );
    println!(
        "Reader trajectories replayed: {total_reader_trajectories} (matched {}, skipped {}, DRIFT {})",
        reader_counts.matched, reader_counts.skipped, reader_counts.drift
    );
    println!(
        "  of which required injecting at least one RFC 0002 §4 fault outcome (Io/Ambiguous/Expired, \
         not a plain uncontended success): writers {writer_fault_trajectories}/{total_writer_trajectories}, \
         readers {reader_fault_trajectories}/{total_reader_trajectories}"
    );

    if !parse_failures.is_empty() {
        println!();
        println!("Parse failures ({}):", parse_failures.len());
        for (p, e) in parse_failures.iter().take(10) {
            println!("  {}: {e}", p.display());
        }
        if parse_failures.len() > 10 {
            println!("  ... and {} more", parse_failures.len() - 10);
        }
    }
    if !replay_failures.is_empty() {
        println!();
        println!(
            "Replay-harness failures ({}) — a bug in THIS harness, not necessarily the protocol:",
            replay_failures.len()
        );
        for (p, e) in replay_failures.iter().take(10) {
            println!("  {}: {e}", p.display());
        }
    }

    if !skip_reasons.is_empty() {
        println!();
        println!("Skip reasons (trajectories not replayed, not counted as pass or fail):");
        for (reason, count) in &skip_reasons {
            println!("  x{count}: {reason}");
        }
    }

    if !drift_details.is_empty() {
        println!();
        println!("DRIFT — real code's outcome did not match the spec's prediction (RFC 0002 §3):");
        for d in &drift_details {
            println!("  - {d}");
        }
        println!();
        println!(
            "See RFC 0002 §3 for the four-way classification (Type-I real bug / Type-II spec \
             too loose / tracer artifact / fault-model mismatch) and classify each instance \
             above by hand before changing any code."
        );
        std::process::exit(1);
    }

    println!();
    println!(
        "No drift: every replayed writer and reader trajectory's real outcome matched what the TLC-generated trace predicted."
    );
}
