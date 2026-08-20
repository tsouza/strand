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

//! Workflow II (RFC 0002 §2): replays TLC-generated action sequences
//! directly against the real `commit`/`commit_deletion_vector`/
//! `read_snapshot` in `strand_core::manifest`, using only that module's
//! public API — this file lives outside `strand-core`'s own crate boundary
//! (a `src/bin/` binary sees exactly what an external consumer would), so
//! it never reaches into `manifest.rs`'s private `read_current`/
//! `propose_snapshot`/`try_read_current`. `_strand/current` and the
//! `_strand/snapshots/` prefix are cited directly from `spec/manifest.md`
//! (normative wire-level names, not a private implementation detail this
//! file is reaching around).
//!
//! # Design: per-writer/per-reader replay, one real call each
//!
//! `manifest.tla`'s writer actions (`ReadCurrent`/`ProposeSnapshot`/
//! `TryAdvancePointer`/`ResolveAmbiguity`) are individually-firable model
//! steps, but the real `commit()` is a single function that runs its own
//! internal retry loop over exactly that action sequence with no external
//! pause point — there is no private per-step entry point to call `this
//! action, then this one` against from outside the crate. This harness
//! resolves that granularity mismatch by replaying **one writer's whole
//! trajectory as one real `commit()` (or `commit_deletion_vector()`) call**
//! against a shared real `InMemoryStore`, with writers replayed in the
//! order each first reaches its own terminal process-counter value (`Done`
//! or `Failed`) in the trace.
//!
//! That ordering choice is load-bearing, not arbitrary: `manifest.tla`'s
//! `TryAdvancePointer` can only take its `stale` branch (real
//! `PreconditionFailed`) because *some other writer's* commit already
//! landed and grew `Len(snapshots)` past what this writer read — and that
//! other writer's own `Done` step is therefore always strictly earlier in
//! trace order than the point where staleness is observed, which is itself
//! strictly earlier than this writer's own terminal step. Replaying in
//! terminal-order means every rival whose landing the model shows mattering
//! has already landed for real, via its own real `commit()` call, by the
//! time the writer that observes staleness runs — so `PreconditionFailed`
//! emerges from the real store's own real ETag comparison, never injected.
//! What this ordering cannot reproduce is a rival landing **mid-flight**,
//! strictly between this writer's own `ReadCurrent` and its own
//! `TryAdvancePointer` within the same cycle (true concurrent interleaving
//! at sub-call granularity) — RFC 0002's own `AmbiguousLanded`/
//! `AmbiguousNotLanded` axis is exactly the sub-case of this the model
//! actually needs (an ambiguous write may or may not have applied), and is
//! reproduced exactly (see `WriterScriptEntry::AdvanceAmbiguous` below); a
//! *literal* mid-cycle rival landing beyond that is out of this harness's
//! scope and is named honestly in the report rather than faked. Since this
//! collapses some model-predicted `stale`-then-retry cycles into a single
//! real attempt that simply starts past the point of staleness, comparison
//! is done at **outcome** level (final `Ok`/`Err`, final version/segments)
//! rather than exact internal retry-count — RFC 0002 §2's own phrasing is
//! "the real code's outcome matches what the spec predicted," which is the
//! level this harness actually checks.
//!
//! Faults that cannot arise from a plain `InMemoryStore` on its own
//! (`Io`/`Ambiguous`) are injected via `ScriptedStore`, a wrapper that
//! intercepts calls positionally against a small per-writer/per-reader
//! script built directly from the trace. Everything else — including every
//! `PreconditionFailed` this harness observes — passes straight through to
//! the real store and is real.

use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, Ordering};

use strand_core::deletion::DeletionVectorRef;
use strand_core::manifest::{
    CommitError, READER_REFRESH_RETRY_LIMIT, ReadError, SegmentRef, SnapshotMetadata, commit,
    commit_deletion_vector, read_snapshot,
};
use strand_core::store::{ConditionalStore, ETag, InMemoryStore, StoreError};

use crate::trace::{ModelState, ReadResult, SnapshotRec};

/// `spec/manifest.md` §2's normative pointer key — cited, not guessed.
const POINTER_KEY: &str = "_strand/current";
/// `spec/manifest.md` §2's normative snapshot-object key prefix.
const SNAPSHOT_PREFIX: &str = "_strand/snapshots/";

// ---------------------------------------------------------------------
// Step 1: diff consecutive states into per-actor action events.
// ---------------------------------------------------------------------

#[derive(Debug, Clone)]
enum Actor {
    Writer(String),
    Reader(String),
}

#[derive(Debug, Clone)]
struct ActionEvent {
    step: usize,
    actor: Actor,
    from_pc: String,
    to_pc: String,
}

/// At most one writer-or-reader pc changes per TLC step, since `Next` is a
/// disjunction of per-process actions (true interleaving semantics, never
/// two processes stepping at once). A step where nothing at all changed is
/// the invisible `ReadCurrent` `Expired` self-loop (`manifest.tla`'s own
/// comment: "a true self-loop... cannot introduce a new reachable state")
/// and produces no event.
fn diff_states(states: &[ModelState]) -> Result<Vec<ActionEvent>, String> {
    let mut events = Vec::new();
    for i in 1..states.len() {
        let prev = &states[i - 1];
        let cur = &states[i];
        let mut found = Vec::new();
        for (w, pc) in &cur.w_pc {
            if prev.w_pc.get(w) != Some(pc) {
                found.push(ActionEvent {
                    step: i,
                    actor: Actor::Writer(w.clone()),
                    from_pc: prev.w_pc.get(w).cloned().unwrap_or_default(),
                    to_pc: pc.clone(),
                });
            }
        }
        for (r, pc) in &cur.r_pc {
            if prev.r_pc.get(r) != Some(pc) {
                found.push(ActionEvent {
                    step: i,
                    actor: Actor::Reader(r.clone()),
                    from_pc: prev.r_pc.get(r).cloned().unwrap_or_default(),
                    to_pc: pc.clone(),
                });
            }
        }
        if found.len() > 1 {
            return Err(format!(
                "step {i}: more than one actor's pc changed at once ({found:?}) — \
                 violates the single-interleaving-step assumption this harness relies on"
            ));
        }
        events.extend(found);
    }
    Ok(events)
}

// ---------------------------------------------------------------------
// Step 2: per-writer trajectory -> a replayable script + expected outcome.
// ---------------------------------------------------------------------

#[derive(Debug, Clone)]
pub enum WriterScriptEntry {
    ReadFail,
    ProposeFail,
    AdvanceSuccess,
    AdvanceFail,
    AdvanceAmbiguous { landed: bool, resolve_fails: bool },
}

#[derive(Debug, Clone)]
pub enum WriterTerminal {
    Done(SnapshotRec),
    Failed,
    /// The trajectory never reached a terminal pc within this trace (cut
    /// off by depth, or — `DeleteWriter`-specific — permanently blocked on
    /// `ProposeDeletionVectorCommit`'s `Len(priorSegs) >= 1` guard because
    /// no segment ever existed). Not replayed; not counted as pass or fail.
    Incomplete,
}

#[derive(Debug, Clone)]
pub struct WriterTrajectory {
    pub writer: String,
    pub last_step: usize,
    pub script: Vec<WriterScriptEntry>,
    pub terminal: WriterTerminal,
    /// Derived directly from the trace's own recorded `wLocal[w]`, not from
    /// `RowIdCounts`'s formula in `manifest.tla` — this harness never needs
    /// to know which writer is `DistinguishedWriter`, only what the trace
    /// itself shows this writer actually proposed.
    pub row_id_count: Option<u64>,
}

fn build_writer_trajectories(
    states: &[ModelState],
    events: &[ActionEvent],
) -> Vec<WriterTrajectory> {
    let mut by_writer: std::collections::BTreeMap<String, Vec<&ActionEvent>> =
        std::collections::BTreeMap::new();
    for ev in events {
        if let Actor::Writer(w) = &ev.actor {
            by_writer.entry(w.clone()).or_default().push(ev);
        }
    }

    let mut out = Vec::new();
    for (w, evs) in by_writer {
        let mut script = Vec::new();
        let mut terminal = WriterTerminal::Incomplete;
        let mut last_step = 0;
        let mut row_id_count = None;
        for ev in evs {
            last_step = ev.step;
            match (ev.from_pc.as_str(), ev.to_pc.as_str()) {
                ("Read", "Propose") => {
                    // ReadCurrent success. Not scripted; captures the base
                    // nextRowId this cycle read, for row_id_count derivation.
                }
                ("Read", "Failed") => {
                    script.push(WriterScriptEntry::ReadFail);
                    terminal = WriterTerminal::Failed;
                }
                ("Propose", "Advance") => {
                    // ProposeSnapshot/ProposeDeletionVectorCommit success.
                    // wLocal[w].proposed is now set; derive row_id_count as
                    // proposed.nextRowId - (nextRowId read at this cycle's
                    // ReadCurrent), for append-shaped writers. Left None for
                    // revise-shaped commits (DeleteWriter), which don't use it.
                    if let Some(wl) = states[ev.step].w_local.get(&w)
                        && let Some(proposed) = &wl.proposed
                    {
                        // best-effort: only meaningful the first time it's set
                        row_id_count
                            .get_or_insert(proposed.next_row_id.saturating_sub(wl.next_row_id));
                    }
                }
                ("Propose", "Failed") => {
                    script.push(WriterScriptEntry::ProposeFail);
                    terminal = WriterTerminal::Failed;
                }
                ("Advance", "Done") => {
                    script.push(WriterScriptEntry::AdvanceSuccess);
                    let proposed = states[ev.step]
                        .w_local
                        .get(&w)
                        .and_then(|wl| wl.proposed.clone());
                    terminal = match proposed {
                        Some(rec) => WriterTerminal::Done(rec),
                        None => WriterTerminal::Incomplete,
                    };
                }
                ("Advance", "Failed") => {
                    script.push(WriterScriptEntry::AdvanceFail);
                    terminal = WriterTerminal::Failed;
                }
                ("Advance", "ResolveAmbiguity") => {
                    // Outcome decided by the following ResolveAmbiguity->*
                    // event; nothing to push yet.
                }
                ("Advance", "Read") => {
                    // AdvanceStale: real staleness, reproduced by replay
                    // ordering (see module doc), never scripted.
                }
                ("ResolveAmbiguity", "Done") => {
                    script.push(WriterScriptEntry::AdvanceAmbiguous {
                        landed: true,
                        resolve_fails: false,
                    });
                    let proposed = states[ev.step]
                        .w_local
                        .get(&w)
                        .and_then(|wl| wl.proposed.clone());
                    terminal = match proposed {
                        Some(rec) => WriterTerminal::Done(rec),
                        None => WriterTerminal::Incomplete,
                    };
                }
                ("ResolveAmbiguity", "Read") => {
                    script.push(WriterScriptEntry::AdvanceAmbiguous {
                        landed: false,
                        resolve_fails: false,
                    });
                }
                ("ResolveAmbiguity", "Failed") => {
                    script.push(WriterScriptEntry::AdvanceAmbiguous {
                        landed: false,
                        resolve_fails: true,
                    });
                    terminal = WriterTerminal::Failed;
                }
                (from, to) => {
                    eprintln!(
                        "warning: writer {w} step {}: unrecognized pc transition {from} -> {to}, ignored",
                        ev.step
                    );
                }
            }
        }
        out.push(WriterTrajectory {
            writer: w,
            last_step,
            script,
            terminal,
            row_id_count,
        });
    }
    out
}

// ---------------------------------------------------------------------
// Step 3: per-reader trajectory.
// ---------------------------------------------------------------------

#[derive(Debug, Clone)]
pub enum ReaderScriptEntry {
    PointerDefiniteFailure,
    SnapshotExpired,
    SnapshotDefiniteFailure,
}

#[derive(Debug, Clone)]
pub enum ReaderTerminal {
    NoCommitsYet,
    Found(SnapshotRec),
    RetriesExhausted,
    DefiniteFailure,
    Incomplete,
}

#[derive(Debug, Clone)]
pub struct ReaderTrajectory {
    pub reader: String,
    pub script: Vec<ReaderScriptEntry>,
    pub terminal: ReaderTerminal,
    /// The version to seed the throwaway store at, so a `Found`/expired
    /// retry sequence has something real to (successfully or transiently)
    /// read. `None` when nothing was ever read (immediate DefiniteFailure).
    pub seed_version: Option<u64>,
}

fn build_reader_trajectories(
    states: &[ModelState],
    events: &[ActionEvent],
) -> Vec<ReaderTrajectory> {
    let mut by_reader: std::collections::BTreeMap<String, Vec<&ActionEvent>> =
        std::collections::BTreeMap::new();
    for ev in events {
        if let Actor::Reader(r) = &ev.actor {
            by_reader.entry(r.clone()).or_default().push(ev);
        }
    }

    let mut out = Vec::new();
    for (r, evs) in by_reader {
        let mut script = Vec::new();
        let mut terminal = ReaderTerminal::Incomplete;
        let mut seed_version = None;
        for ev in evs {
            match (ev.from_pc.as_str(), ev.to_pc.as_str()) {
                ("ReadPtr", "Done") => terminal = ReaderTerminal::NoCommitsYet,
                ("ReadPtr", "ReadSnap") => {
                    if let Some(rl) = states[ev.step].r_local.get(&r) {
                        seed_version = Some(rl.ptr_version);
                    }
                }
                ("ReadPtr", "Failed_DefiniteFailure") => {
                    script.push(ReaderScriptEntry::PointerDefiniteFailure);
                    terminal = ReaderTerminal::DefiniteFailure;
                }
                ("ReadSnap", "Done") => {
                    let result = states[ev.step].r_local.get(&r).map(|rl| rl.result.clone());
                    terminal = match result {
                        Some(ReadResult::Snapshot(rec)) => ReaderTerminal::Found(rec),
                        _ => ReaderTerminal::Incomplete,
                    };
                }
                ("ReadSnap", "ReadPtr") => {
                    script.push(ReaderScriptEntry::SnapshotExpired);
                }
                ("ReadSnap", "Failed_RetriesExhausted") => {
                    script.push(ReaderScriptEntry::SnapshotExpired);
                    terminal = ReaderTerminal::RetriesExhausted;
                }
                ("ReadSnap", "Failed_DefiniteFailure") => {
                    script.push(ReaderScriptEntry::SnapshotDefiniteFailure);
                    terminal = ReaderTerminal::DefiniteFailure;
                }
                (from, to) => {
                    eprintln!(
                        "warning: reader {r} step {}: unrecognized pc transition {from} -> {to}, ignored",
                        ev.step
                    );
                }
            }
        }
        out.push(ReaderTrajectory {
            reader: r,
            script,
            terminal,
            seed_version,
        });
    }
    out
}

// ---------------------------------------------------------------------
// Step 4: the scripted ConditionalStore wrapper writers replay against.
// ---------------------------------------------------------------------

struct ScriptedWriterStore<'a> {
    inner: &'a InMemoryStore,
    script: RefCell<VecDeque<WriterScriptEntry>>,
    /// Set immediately after this wrapper returns `Ambiguous` from a
    /// pointer-CAS call; consumed by the very next `get(POINTER_KEY)` call,
    /// which `propose_snapshot`'s real disambiguation logic always issues
    /// immediately afterward. `Some(resolve_fails)`.
    awaiting_resolution: Cell<Option<bool>>,
    pub drift_notes: RefCell<Vec<String>>,
}

impl ConditionalStore for ScriptedWriterStore<'_> {
    fn get(&self, key: &str) -> Result<Option<(Vec<u8>, ETag)>, StoreError> {
        if key == POINTER_KEY {
            if let Some(resolve_fails) = self.awaiting_resolution.take() {
                if resolve_fails {
                    return Err(StoreError::Io(
                        "scripted: ambiguity-resolution read failed".into(),
                    ));
                }
                return self.inner.get(key);
            }
            let mut script = self.script.borrow_mut();
            if matches!(script.front(), Some(WriterScriptEntry::ReadFail)) {
                script.pop_front();
                return Err(StoreError::Io(
                    "scripted: ReadCurrent DefiniteFailure".into(),
                ));
            }
        }
        self.inner.get(key)
    }

    fn put_if_absent(&self, key: &str, bytes: &[u8]) -> Result<ETag, StoreError> {
        if key == POINTER_KEY {
            return self.pointer_write(|| self.inner.put_if_absent(key, bytes));
        }
        if key.starts_with(SNAPSHOT_PREFIX) {
            let mut script = self.script.borrow_mut();
            if matches!(script.front(), Some(WriterScriptEntry::ProposeFail)) {
                script.pop_front();
                return Err(StoreError::Io(
                    "scripted: ProposeSnapshot DefiniteFailure".into(),
                ));
            }
        }
        // Any other key (e.g. a DeleteWriter's deletion-vector-bytes
        // object) is not part of the protocol's own CAS mechanics; pass
        // straight through, unscripted.
        self.inner.put_if_absent(key, bytes)
    }

    fn put_if_match(&self, key: &str, bytes: &[u8], etag: &ETag) -> Result<ETag, StoreError> {
        if key == POINTER_KEY {
            return self.pointer_write(|| self.inner.put_if_match(key, bytes, etag));
        }
        self.inner.put_if_match(key, bytes, etag)
    }
}

impl ScriptedWriterStore<'_> {
    fn pointer_write(
        &self,
        delegate: impl FnOnce() -> Result<ETag, StoreError>,
    ) -> Result<ETag, StoreError> {
        let front = self.script.borrow().front().cloned();
        match front {
            Some(WriterScriptEntry::AdvanceSuccess) => match delegate() {
                Ok(etag) => {
                    self.script.borrow_mut().pop_front();
                    Ok(etag)
                }
                Err(StoreError::PreconditionFailed) => {
                    // Ordering-collapse artifact (module doc): a rival the
                    // trace didn't attribute to this exact cycle still
                    // landed for real by this point. Not consumed — the
                    // real natural retry commit() performs on its own will
                    // try this same scripted entry again.
                    Err(StoreError::PreconditionFailed)
                }
                Err(other) => {
                    self.script.borrow_mut().pop_front();
                    self.drift_notes.borrow_mut().push(format!(
                        "AdvanceSuccess scripted but real store returned {other:?}"
                    ));
                    Err(other)
                }
            },
            Some(WriterScriptEntry::AdvanceFail) => {
                self.script.borrow_mut().pop_front();
                Err(StoreError::Io(
                    "scripted: TryAdvancePointer DefiniteFailure".into(),
                ))
            }
            Some(WriterScriptEntry::AdvanceAmbiguous {
                landed,
                resolve_fails,
            }) => {
                self.script.borrow_mut().pop_front();
                self.awaiting_resolution.set(Some(resolve_fails));
                if landed {
                    match delegate() {
                        Ok(_) => {}
                        Err(StoreError::PreconditionFailed) => {
                            self.drift_notes.borrow_mut().push(
                                "AmbiguousLanded scripted but the real underlying write was \
                                 stale (PreconditionFailed) — replay-ordering assumption did \
                                 not hold for this occurrence"
                                    .into(),
                            );
                        }
                        Err(other) => {
                            self.drift_notes.borrow_mut().push(format!(
                                "AmbiguousLanded scripted, real write errored: {other:?}"
                            ));
                        }
                    }
                }
                Err(StoreError::Ambiguous(
                    "scripted: TryAdvancePointer Ambiguous".into(),
                ))
            }
            Some(WriterScriptEntry::ReadFail | WriterScriptEntry::ProposeFail) | None => delegate(),
        }
    }
}

/// Real, placeholder-content `SegmentRef`s from the model's abstract
/// `SegRec`s (which carry only `base`/`count`/`delVer`, per
/// `manifest.tla`'s own comment that no invariant here depends on actual
/// content). Paths are unique per (version, index) so no real collision is
/// possible across a trace's seeded history.
fn to_real_segments(version: u64, segs: &[crate::trace::SegRec]) -> Vec<SegmentRef> {
    segs.iter()
        .enumerate()
        .map(|(i, s)| SegmentRef {
            path: format!("segments/model-v{version}-{i}.bin"),
            row_id_base: s.base,
            row_id_count: s.count,
            byte_length: 1,
            checksum: 0,
            deletion_vector: if s.del_ver > 0 {
                Some(DeletionVectorRef {
                    path: format!("deletions/model-v{version}-{i}.bin"),
                    byte_length: 1,
                    checksum: 0,
                })
            } else {
                None
            },
        })
        .collect()
}

fn to_real_snapshot(rec: &SnapshotRec) -> SnapshotMetadata {
    SnapshotMetadata {
        version: rec.version,
        next_row_id: rec.next_row_id,
        segments: to_real_segments(rec.version, &rec.segments),
        committed_at_millis: 0,
    }
}

/// Writes `snapshot` for real, performing exactly the two writes
/// `propose_snapshot` would (`spec/manifest.md` §2 step 2-3), using only
/// the public `ConditionalStore` API — the harness's equivalent of a rival
/// writer's commit having already landed, without re-deriving that rival's
/// own fault sequence (its own trajectory is independently replayed
/// elsewhere in the same corpus; only its *observable effect* matters here).
fn seed_commit(store: &InMemoryStore, snapshot: &SnapshotMetadata) {
    let path = format!(
        "_strand/snapshots/{:020}-seed{}.json",
        snapshot.version, snapshot.version
    );
    let bytes = serde_json::to_vec(snapshot).expect("snapshot metadata serializes");
    store
        .put_if_absent(&path, &bytes)
        .expect("seed snapshot path is unique per version");
    match store
        .get(POINTER_KEY)
        .expect("seed pointer read never fails")
    {
        Some((_, etag)) => {
            store
                .put_if_match(POINTER_KEY, path.as_bytes(), &etag)
                .expect("seed pointer CAS uses a freshly-read etag");
        }
        None => {
            store
                .put_if_absent(POINTER_KEY, path.as_bytes())
                .expect("seed pointer create-if-absent on an empty table");
        }
    }
}

// ---------------------------------------------------------------------
// Step 5: comparisons and the public per-trajectory replay entry points.
// ---------------------------------------------------------------------

#[derive(Debug)]
pub enum Verdict {
    Matched,
    Skipped(String),
    Drift(String),
}

fn segments_match(real: &[SegmentRef], model: &[crate::trace::SegRec]) -> Result<(), String> {
    if real.len() != model.len() {
        return Err(format!(
            "segment count mismatch: real {} vs model {}",
            real.len(),
            model.len()
        ));
    }
    for (i, (r, m)) in real.iter().zip(model.iter()).enumerate() {
        if r.row_id_base != m.base || r.row_id_count != m.count {
            return Err(format!(
                "segment {i} mismatch: real (base={}, count={}) vs model (base={}, count={})",
                r.row_id_base, r.row_id_count, m.base, m.count
            ));
        }
        let real_has_dv = r.deletion_vector.is_some();
        let model_has_dv = m.del_ver > 0;
        if real_has_dv != model_has_dv {
            return Err(format!(
                "segment {i} deletion_vector presence mismatch: real {real_has_dv} vs model \
                 delVer={} (>0 means present)",
                m.del_ver
            ));
        }
    }
    Ok(())
}

fn snapshot_matches(real: &SnapshotMetadata, model: &SnapshotRec) -> Result<(), String> {
    if real.version != model.version {
        return Err(format!(
            "version mismatch: real {} vs model {}",
            real.version, model.version
        ));
    }
    if real.next_row_id != model.next_row_id {
        return Err(format!(
            "next_row_id mismatch: real {} vs model {}",
            real.next_row_id, model.next_row_id
        ));
    }
    segments_match(&real.segments, &model.segments)
}

/// Replays one writer's whole trajectory as one real `commit()` (or
/// `commit_deletion_vector()`, for `delete_writer`) call against `shared`.
pub fn replay_writer(
    shared: &InMemoryStore,
    traj: &WriterTrajectory,
    is_delete_writer: bool,
) -> Verdict {
    let (WriterTerminal::Done(_) | WriterTerminal::Failed) = &traj.terminal else {
        return Verdict::Skipped("trajectory never reached a terminal pc in this trace".into());
    };

    let wrapper = ScriptedWriterStore {
        inner: shared,
        script: RefCell::new(traj.script.iter().cloned().collect()),
        awaiting_resolution: Cell::new(None),
        drift_notes: RefCell::new(Vec::new()),
    };

    let result = if is_delete_writer {
        // A real target segment is only actually needed if this writer's
        // OWN internal read is scripted to succeed for real — a
        // `ReadFail`-terminal trajectory fails at `read_current` before
        // `segment_path` is ever inspected (`manifest.rs`'s
        // `commit_deletion_vector`), so a missing real segment there is
        // harmless, not a reason to skip replay. When a real segment truly
        // is required and is genuinely missing, the placeholder path below
        // makes that surface honestly as a real `SegmentNotFound` outcome
        // to compare against the trace's own prediction, rather than being
        // silently absorbed as "skipped."
        let target = match read_snapshot(shared) {
            Ok(Some(s)) => s
                .segments
                .first()
                .map(|s| s.path.clone())
                .unwrap_or_else(|| "segments/dst-no-real-segment-yet.bin".to_string()),
            _ => "segments/dst-no-real-segment-yet.bin".to_string(),
        };
        commit_deletion_vector(&wrapper, &target, |seg| {
            let mut bitmap = roaring::RoaringBitmap::new();
            bitmap.insert(0);
            let bytes = strand_core::deletion::build_deletion_vector(&bitmap, seg.row_id_count)
                .expect("valid row_id_count");
            let checksum = strand_core::deletion::checksum(&bytes);
            static DV_NONCE: AtomicU64 = AtomicU64::new(0);
            let dv_path = format!(
                "deletions/dst-{}.bin",
                DV_NONCE.fetch_add(1, Ordering::Relaxed)
            );
            wrapper
                .put_if_absent(&dv_path, &bytes)
                .expect("deletion-vector object path is fresh");
            DeletionVectorRef {
                path: dv_path,
                byte_length: bytes.len() as u64,
                checksum,
            }
        })
    } else {
        let row_id_count = traj.row_id_count.unwrap_or(1).max(1);
        commit(&wrapper, move |next_row_id| {
            vec![SegmentRef {
                path: format!("segments/dst-{next_row_id}-{row_id_count}.bin"),
                row_id_base: next_row_id,
                row_id_count,
                byte_length: 1,
                checksum: 0,
                deletion_vector: None,
            }]
        })
    };

    let notes = wrapper.drift_notes.into_inner();
    let verdict = match (&result, &traj.terminal) {
        (Ok(snapshot), WriterTerminal::Done(model)) => match snapshot_matches(snapshot, model) {
            Ok(()) => Verdict::Matched,
            Err(msg) => Verdict::Drift(format!(
                "writer {}: trace predicted Done matching {model:?}, real commit() returned Ok \
                 with a DIFFERENT snapshot: {msg}",
                traj.writer
            )),
        },
        (Err(CommitError::Io(msg)), WriterTerminal::Failed) => {
            let _ = msg;
            Verdict::Matched
        }
        (Ok(snapshot), WriterTerminal::Failed) => Verdict::Drift(format!(
            "writer {}: trace predicted Failed (a definite backend error), real commit() \
             returned Ok({:?}) instead — the real code silently succeeded where the spec said \
             it must fail",
            traj.writer, snapshot.version
        )),
        (Err(e), WriterTerminal::Done(model)) => Verdict::Drift(format!(
            "writer {}: trace predicted Done matching {model:?}, real commit() returned Err({e:?}) \
             instead",
            traj.writer
        )),
        (Err(e), WriterTerminal::Failed) => Verdict::Drift(format!(
            "writer {}: trace predicted Failed (a definite backend Io error), real commit() \
             returned a DIFFERENT error variant instead: {e:?}",
            traj.writer
        )),
        (_, WriterTerminal::Incomplete) => unreachable!("filtered above"),
    };

    if !notes.is_empty() && matches!(verdict, Verdict::Matched) {
        return Verdict::Skipped(format!(
            "final outcome matched, but replay-ordering assumptions were not fully clean: {notes:?}"
        ));
    }
    verdict
}

struct ScriptedReaderStore<'a> {
    inner: &'a InMemoryStore,
    script: RefCell<VecDeque<ReaderScriptEntry>>,
}

impl ConditionalStore for ScriptedReaderStore<'_> {
    fn get(&self, key: &str) -> Result<Option<(Vec<u8>, ETag)>, StoreError> {
        if key == POINTER_KEY {
            let mut script = self.script.borrow_mut();
            if matches!(
                script.front(),
                Some(ReaderScriptEntry::PointerDefiniteFailure)
            ) {
                script.pop_front();
                return Err(StoreError::Io(
                    "scripted: ReadPointer DefiniteFailure".into(),
                ));
            }
            return self.inner.get(key);
        }
        let mut script = self.script.borrow_mut();
        match script.front() {
            Some(ReaderScriptEntry::SnapshotExpired) => {
                script.pop_front();
                return Ok(None);
            }
            Some(ReaderScriptEntry::SnapshotDefiniteFailure) => {
                script.pop_front();
                return Err(StoreError::Io(
                    "scripted: ReadSnapshotObject DefiniteFailure".into(),
                ));
            }
            _ => {}
        }
        self.inner.get(key)
    }

    fn put_if_absent(&self, key: &str, bytes: &[u8]) -> Result<ETag, StoreError> {
        self.inner.put_if_absent(key, bytes)
    }

    fn put_if_match(&self, key: &str, bytes: &[u8], etag: &ETag) -> Result<ETag, StoreError> {
        self.inner.put_if_match(key, bytes, etag)
    }
}

/// Replays one reader's whole trajectory as one real `read_snapshot()`
/// call against a dedicated, freshly-seeded throwaway store — reader
/// replay never shares a store with writer replay or other readers (§
/// module doc: readers have no side effects to interleave). `history` is
/// the trace's own final, complete `snapshots` sequence (dense from
/// version 0, per `manifest.tla`'s `VersionsMatchIndex` invariant), used to
/// seed the *real* recorded content at each version rather than a
/// placeholder — so a `Found` verdict can be checked structurally, not just
/// "some snapshot came back."
pub fn replay_reader(traj: &ReaderTrajectory, history: &[SnapshotRec]) -> Verdict {
    let store = InMemoryStore::new();
    match (&traj.terminal, traj.seed_version) {
        (ReaderTerminal::Incomplete, _) => {
            return Verdict::Skipped("trajectory never reached a terminal pc in this trace".into());
        }
        (_, Some(v)) => {
            // `rLocal[r].ptrVersion` is `manifest.tla`'s own
            // `Len(snapshots)` (a TLA+ 1-indexed sequence *length*, per
            // `ReadPointer`'s `rLocal' = [... ptrVersion |-> Len(snapshots)]`
            // and `ReadSnapshotObject`'s own `snapshots[rLocal[r].ptrVersion]`
            // lookup), not a 0-indexed position — `history` here is a
            // 0-indexed Rust `Vec` where `history[k].version == k`
            // (`VersionsMatchIndex`), so the reader's target real entry is
            // at `history[v - 1]`, and `v` total entries (`0..v`) must exist.
            for version in 0..(v as usize) {
                let Some(rec) = history.get(version) else {
                    return Verdict::Skipped(format!(
                        "reader {}: trace's own snapshot history has no entry for version {version}",
                        traj.reader
                    ));
                };
                seed_commit(&store, &to_real_snapshot(rec));
            }
        }
        (ReaderTerminal::NoCommitsYet, None) => {}
        (_, None) => {}
    }

    // RFC 0002's own model/real bound gap (verification/README.md,
    // "Model size"): manifest.cfg's ReaderRetryLimit is 2, a stand-in for
    // the shape of a bound, while the real READER_REFRESH_RETRY_LIMIT is
    // 5. A trajectory whose model-predicted terminal is RetriesExhausted
    // after only 2 Expired reads would NOT exhaust the real, larger bound
    // if replayed with only 2 scripted Expired entries — that would be a
    // known, already-documented divergence surfacing as spurious "drift,"
    // not a new finding. Scale the injected fault count up to the real
    // bound instead, and check the real code respects ITS OWN documented
    // bound — a more meaningful check than mimicking the model's smaller
    // stand-in value.
    let script: VecDeque<ReaderScriptEntry> =
        if matches!(traj.terminal, ReaderTerminal::RetriesExhausted) {
            (0..=READER_REFRESH_RETRY_LIMIT)
                .map(|_| ReaderScriptEntry::SnapshotExpired)
                .collect()
        } else {
            traj.script.iter().cloned().collect()
        };

    let wrapper = ScriptedReaderStore {
        inner: &store,
        script: RefCell::new(script),
    };
    let result = read_snapshot(&wrapper);

    match (&result, &traj.terminal) {
        (Ok(None), ReaderTerminal::NoCommitsYet) => Verdict::Matched,
        (Ok(Some(real_snap)), ReaderTerminal::Found(model)) => {
            match snapshot_matches(real_snap, model) {
                Ok(()) => Verdict::Matched,
                Err(msg) => Verdict::Drift(format!(
                    "reader {}: trace predicted Found matching {model:?}, real read_snapshot() \
                 returned a DIFFERENT snapshot: {msg}",
                    traj.reader
                )),
            }
        }
        (Err(ReadError::RetriesExhausted), ReaderTerminal::RetriesExhausted) => Verdict::Matched,
        (Err(ReadError::Io(_)), ReaderTerminal::DefiniteFailure) => Verdict::Matched,
        (real, model) => Verdict::Drift(format!(
            "reader {}: trace predicted {model:?}, real read_snapshot() returned {real:?}",
            traj.reader
        )),
    }
}

// ---------------------------------------------------------------------
// Top-level: replay one whole trace file.
// ---------------------------------------------------------------------

/// Whether a trajectory's script required injecting at least one fault a
/// plain store could never produce on its own (`Io`/`Ambiguous`) — as
/// opposed to a trivial, uncontended solo commit/read. Reported alongside
/// each verdict so the final report can state, honestly, how much of the
/// replayed corpus actually exercised RFC 0002 §4's fault-outcome branches
/// rather than just the happy path.
fn writer_has_injected_fault(script: &[WriterScriptEntry]) -> bool {
    script
        .iter()
        .any(|e| !matches!(e, WriterScriptEntry::AdvanceSuccess))
}

fn reader_has_injected_fault(script: &[ReaderScriptEntry]) -> bool {
    !script.is_empty()
}

pub struct TraceReplayResult {
    pub writers: Vec<(String, bool, Verdict)>,
    pub readers: Vec<(String, bool, Verdict)>,
}

pub fn replay_trace(
    states: &[ModelState],
    delete_writer: &str,
) -> Result<TraceReplayResult, String> {
    let events = diff_states(states)?;
    let mut writer_trajs = build_writer_trajectories(states, &events);
    let reader_trajs = build_reader_trajectories(states, &events);

    writer_trajs.sort_by_key(|t| t.last_step);

    let shared = InMemoryStore::new();
    let mut writers = Vec::new();
    for traj in &writer_trajs {
        let verdict = replay_writer(&shared, traj, traj.writer == delete_writer);
        writers.push((
            traj.writer.clone(),
            writer_has_injected_fault(&traj.script),
            verdict,
        ));
    }

    // The trace's own final, complete history — dense from version 0 per
    // `VersionsMatchIndex` — used to seed reader replay with real content.
    let history = &states
        .last()
        .map(|s| s.snapshots.clone())
        .unwrap_or_default();

    let mut readers = Vec::new();
    for traj in &reader_trajs {
        readers.push((
            traj.reader.clone(),
            reader_has_injected_fault(&traj.script),
            replay_reader(traj, history),
        ));
    }

    Ok(TraceReplayResult { writers, readers })
}
