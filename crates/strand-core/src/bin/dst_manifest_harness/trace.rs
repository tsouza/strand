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

//! Parses the trace files TLC's `-simulate ... file=X` mode writes: one
//! `.tla`-shaped module per generated trace, containing a numbered sequence
//! of `STATE_N == /\ var = value ...` records — the full model state after
//! each step, not an action label. There is no TLC output mode that names
//! which action fired at each step (confirmed against `tla2tools.jar
//! -help`'s real flag list while building this harness: `-dump` writes the
//! whole reachable graph, not a single trace; `-simulate`'s trace files are
//! the closest real, working mechanism, and RFC 0002 §2 itself only commits
//! to "TLC... generates a large set of valid action sequences," not a
//! specific mechanism) — so this module reconstructs which action fired at
//! each step by diffing consecutive states' process-counter variables
//! (`replay::classify`), not by reading an action name out of the trace.

use std::collections::BTreeMap;
use std::fmt;

/// One parsed TLA+ value, general enough for every shape `manifest.tla`'s
/// variables take: records (`[field |-> v, ...]`), functions written as
/// `k :> v @@ ...` (TLC's own pretty-printing of a finite function, used
/// here for `wPc`/`wLocal`/`rPc`/`rLocal`), sequences (`<<a, b, c>>`), plain
/// identifiers (model values like `NoProposalVal`, or bare strings like
/// process-counter tags once quoted), quoted strings, and numbers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Value {
    Str(String),
    Num(i64),
    Ident(String),
    Seq(Vec<Value>),
    Record(Vec<(String, Value)>),
    Func(Vec<(Value, Value)>),
}

impl Value {
    fn as_str_or_ident(&self) -> Option<&str> {
        match self {
            Value::Str(s) | Value::Ident(s) => Some(s.as_str()),
            _ => None,
        }
    }

    fn as_num(&self) -> Option<i64> {
        match self {
            Value::Num(n) => Some(*n),
            _ => None,
        }
    }

    fn field(&self, name: &str) -> Option<&Value> {
        match self {
            Value::Record(fields) => fields.iter().find(|(k, _)| k == name).map(|(_, v)| v),
            _ => None,
        }
    }

    fn as_seq(&self) -> Option<&[Value]> {
        match self {
            Value::Seq(items) => Some(items),
            _ => None,
        }
    }

    /// Reads a `Func` value (TLC's `k :> v @@ ...` finite-function
    /// pretty-print) as a map from each key's own string/ident form to its
    /// value. `wPc`/`rPc`/`wLocal`/`rLocal` all take this shape, keyed by
    /// writer/reader id (`w1`, `r1`, ...).
    fn as_func_map(&self) -> BTreeMap<String, Value> {
        match self {
            Value::Func(entries) => entries
                .iter()
                .filter_map(|(k, v)| k.as_str_or_ident().map(|k| (k.to_string(), v.clone())))
                .collect(),
            _ => BTreeMap::new(),
        }
    }
}

#[derive(Debug)]
pub struct ParseError(pub String);

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

// ---------------------------------------------------------------------
// Tokenizer + recursive-descent parser for one TLA+ value expression.
// Scoped deliberately narrow: this is not a general TLA+ parser, only the
// value grammar TLC's own pretty-printer emits for this specific model's
// variable shapes (records, `:>`/`@@` functions, `<<>>` sequences, quoted
// strings, integers, bare identifiers). Grounded against real tla2tools.jar
// output captured while building this harness, not against a remembered
// grammar (CLAUDE.md §3).
// ---------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
enum Tok {
    LBracket,
    RBracket,
    LParen,
    RParen,
    LSeq,
    RSeq,
    Comma,
    MapsTo,  // |->
    ArrowFn, // :>
    Concat,  // @@
    Str(String),
    Num(i64),
    Ident(String),
}

fn tokenize(input: &str) -> Result<Vec<Tok>, ParseError> {
    let chars: Vec<char> = input.chars().collect();
    let mut i = 0;
    let mut toks = Vec::new();
    while i < chars.len() {
        let c = chars[i];
        match c {
            ' ' | '\t' | '\n' | '\r' => i += 1,
            '[' => {
                toks.push(Tok::LBracket);
                i += 1;
            }
            ']' => {
                toks.push(Tok::RBracket);
                i += 1;
            }
            '(' => {
                toks.push(Tok::LParen);
                i += 1;
            }
            ')' => {
                toks.push(Tok::RParen);
                i += 1;
            }
            ',' => {
                toks.push(Tok::Comma);
                i += 1;
            }
            '<' if chars.get(i + 1) == Some(&'<') => {
                toks.push(Tok::LSeq);
                i += 2;
            }
            '>' if chars.get(i + 1) == Some(&'>') => {
                toks.push(Tok::RSeq);
                i += 2;
            }
            '|' if chars.get(i + 1) == Some(&'-') && chars.get(i + 2) == Some(&'>') => {
                toks.push(Tok::MapsTo);
                i += 3;
            }
            ':' if chars.get(i + 1) == Some(&'>') => {
                toks.push(Tok::ArrowFn);
                i += 2;
            }
            '@' if chars.get(i + 1) == Some(&'@') => {
                toks.push(Tok::Concat);
                i += 2;
            }
            '"' => {
                let mut s = String::new();
                i += 1;
                while i < chars.len() && chars[i] != '"' {
                    s.push(chars[i]);
                    i += 1;
                }
                i += 1; // closing quote
                toks.push(Tok::Str(s));
            }
            c if c.is_ascii_digit()
                || (c == '-' && chars.get(i + 1).is_some_and(|d| d.is_ascii_digit())) =>
            {
                let start = i;
                i += 1;
                while i < chars.len() && chars[i].is_ascii_digit() {
                    i += 1;
                }
                let text: String = chars[start..i].iter().collect();
                toks.push(Tok::Num(
                    text.parse()
                        .map_err(|e| ParseError(format!("bad number {text:?}: {e}")))?,
                ));
            }
            c if c.is_alphanumeric() || c == '_' => {
                let start = i;
                while i < chars.len() && (chars[i].is_alphanumeric() || chars[i] == '_') {
                    i += 1;
                }
                toks.push(Tok::Ident(chars[start..i].iter().collect()));
            }
            other => {
                return Err(ParseError(format!(
                    "unexpected character {other:?} in value"
                )));
            }
        }
    }
    Ok(toks)
}

struct Parser {
    toks: Vec<Tok>,
    pos: usize,
}

impl Parser {
    fn peek(&self) -> Option<&Tok> {
        self.toks.get(self.pos)
    }

    fn next(&mut self) -> Option<Tok> {
        let t = self.toks.get(self.pos).cloned();
        self.pos += 1;
        t
    }

    fn expect(&mut self, tok: &Tok) -> Result<(), ParseError> {
        match self.next() {
            Some(t) if &t == tok => Ok(()),
            other => Err(ParseError(format!("expected {tok:?}, found {other:?}"))),
        }
    }

    fn parse_value(&mut self) -> Result<Value, ParseError> {
        match self.peek() {
            Some(Tok::LBracket) => self.parse_record(),
            Some(Tok::LSeq) => self.parse_seq(),
            Some(Tok::LParen) => self.parse_func(),
            Some(Tok::Str(_)) => {
                let Some(Tok::Str(s)) = self.next() else {
                    unreachable!()
                };
                Ok(Value::Str(s))
            }
            Some(Tok::Num(_)) => {
                let Some(Tok::Num(n)) = self.next() else {
                    unreachable!()
                };
                Ok(Value::Num(n))
            }
            Some(Tok::Ident(_)) => {
                let Some(Tok::Ident(s)) = self.next() else {
                    unreachable!()
                };
                Ok(Value::Ident(s))
            }
            other => Err(ParseError(format!(
                "unexpected token starting a value: {other:?}"
            ))),
        }
    }

    fn parse_record(&mut self) -> Result<Value, ParseError> {
        self.expect(&Tok::LBracket)?;
        let mut fields = Vec::new();
        if self.peek() != Some(&Tok::RBracket) {
            loop {
                let name = match self.next() {
                    Some(Tok::Ident(s)) => s,
                    other => {
                        return Err(ParseError(format!("expected field name, found {other:?}")));
                    }
                };
                self.expect(&Tok::MapsTo)?;
                let val = self.parse_value()?;
                fields.push((name, val));
                if self.peek() == Some(&Tok::Comma) {
                    self.next();
                    continue;
                }
                break;
            }
        }
        self.expect(&Tok::RBracket)?;
        Ok(Value::Record(fields))
    }

    fn parse_seq(&mut self) -> Result<Value, ParseError> {
        self.expect(&Tok::LSeq)?;
        let mut items = Vec::new();
        if self.peek() != Some(&Tok::RSeq) {
            loop {
                items.push(self.parse_value()?);
                if self.peek() == Some(&Tok::Comma) {
                    self.next();
                    continue;
                }
                break;
            }
        }
        self.expect(&Tok::RSeq)?;
        Ok(Value::Seq(items))
    }

    /// `( key :> value @@ key :> value ... )` — TLC's pretty-print of a
    /// finite function, always parenthesized in this model's own output.
    fn parse_func(&mut self) -> Result<Value, ParseError> {
        self.expect(&Tok::LParen)?;
        let mut entries = Vec::new();
        loop {
            let key = self.parse_value()?;
            self.expect(&Tok::ArrowFn)?;
            let val = self.parse_value()?;
            entries.push((key, val));
            if self.peek() == Some(&Tok::Concat) {
                self.next();
                continue;
            }
            break;
        }
        self.expect(&Tok::RParen)?;
        Ok(Value::Func(entries))
    }
}

fn parse_value_text(text: &str) -> Result<Value, ParseError> {
    let toks = tokenize(text)?;
    let mut p = Parser { toks, pos: 0 };
    let v = p.parse_value()?;
    if p.pos != p.toks.len() {
        return Err(ParseError(format!(
            "trailing tokens after value: {:?}",
            &p.toks[p.pos..]
        )));
    }
    Ok(v)
}

// ---------------------------------------------------------------------
// Typed extraction: Value -> the specific shapes manifest.tla declares.
// ---------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SegRec {
    pub base: u64,
    pub count: u64,
    pub del_ver: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SnapshotRec {
    pub version: u64,
    pub next_row_id: u64,
    pub segments: Vec<SegRec>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WLocal {
    pub base_version: u64,
    pub next_row_id: u64,
    pub proposed: Option<SnapshotRec>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReadResult {
    None,
    NoCommitsYet,
    Snapshot(SnapshotRec),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RLocal {
    pub retries: u64,
    pub ptr_version: u64,
    pub result: ReadResult,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ModelState {
    pub w_pc: BTreeMap<String, String>,
    pub w_local: BTreeMap<String, WLocal>,
    pub r_pc: BTreeMap<String, String>,
    pub r_local: BTreeMap<String, RLocal>,
    pub snapshots: Vec<SnapshotRec>,
}

fn value_to_seg(v: &Value) -> Result<SegRec, ParseError> {
    Ok(SegRec {
        base: v
            .field("base")
            .and_then(Value::as_num)
            .ok_or_else(|| ParseError("segment missing base".into()))? as u64,
        count: v
            .field("count")
            .and_then(Value::as_num)
            .ok_or_else(|| ParseError("segment missing count".into()))? as u64,
        del_ver: v
            .field("delVer")
            .and_then(Value::as_num)
            .ok_or_else(|| ParseError("segment missing delVer".into()))? as u64,
    })
}

fn value_to_snapshot(v: &Value) -> Result<SnapshotRec, ParseError> {
    let version = v
        .field("version")
        .and_then(Value::as_num)
        .ok_or_else(|| ParseError("snapshot missing version".into()))? as u64;
    let next_row_id =
        v.field("nextRowId")
            .and_then(Value::as_num)
            .ok_or_else(|| ParseError("snapshot missing nextRowId".into()))? as u64;
    let segments = v
        .field("segments")
        .and_then(Value::as_seq)
        .ok_or_else(|| ParseError("snapshot missing segments".into()))?
        .iter()
        .map(value_to_seg)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(SnapshotRec {
        version,
        next_row_id,
        segments,
    })
}

/// `NoProposalVal`/sentinel idents parse as bare identifiers; anything else
/// under `proposed`/`result` is a real record.
fn value_to_optional_snapshot(v: &Value) -> Result<Option<SnapshotRec>, ParseError> {
    match v {
        Value::Ident(_) => Ok(None),
        Value::Record(_) => Ok(Some(value_to_snapshot(v)?)),
        other => Err(ParseError(format!(
            "unexpected proposed/result shape: {other:?}"
        ))),
    }
}

fn value_to_wlocal(v: &Value) -> Result<WLocal, ParseError> {
    Ok(WLocal {
        base_version: v
            .field("baseVersion")
            .and_then(Value::as_num)
            .ok_or_else(|| ParseError("wLocal missing baseVersion".into()))?
            as u64,
        next_row_id: v
            .field("nextRowId")
            .and_then(Value::as_num)
            .ok_or_else(|| ParseError("wLocal missing nextRowId".into()))?
            as u64,
        proposed: value_to_optional_snapshot(
            v.field("proposed")
                .ok_or_else(|| ParseError("wLocal missing proposed".into()))?,
        )?,
    })
}

fn value_to_rlocal(v: &Value) -> Result<RLocal, ParseError> {
    let result_val = v
        .field("result")
        .ok_or_else(|| ParseError("rLocal missing result".into()))?;
    let result = match result_val {
        Value::Ident(s) if s == "NoResultVal" => ReadResult::None,
        Value::Ident(s) if s == "NoCommitsYetVal" => ReadResult::NoCommitsYet,
        Value::Record(_) => ReadResult::Snapshot(value_to_snapshot(result_val)?),
        other => {
            return Err(ParseError(format!(
                "unexpected rLocal.result shape: {other:?}"
            )));
        }
    };
    Ok(RLocal {
        retries: v
            .field("retries")
            .and_then(Value::as_num)
            .ok_or_else(|| ParseError("rLocal missing retries".into()))? as u64,
        ptr_version: v
            .field("ptrVersion")
            .and_then(Value::as_num)
            .ok_or_else(|| ParseError("rLocal missing ptrVersion".into()))?
            as u64,
        result,
    })
}

fn build_state(assignments: &[(String, String)]) -> Result<ModelState, ParseError> {
    let mut state = ModelState::default();
    for (name, text) in assignments {
        let value = parse_value_text(text)
            .map_err(|e| ParseError(format!("parsing {name} = {text}: {e}")))?;
        match name.as_str() {
            "wPc" => {
                for (k, v) in value.as_func_map() {
                    let tag = v
                        .as_str_or_ident()
                        .ok_or_else(|| ParseError("wPc entry not a tag".into()))?
                        .to_string();
                    state.w_pc.insert(k, tag);
                }
            }
            "rPc" => {
                for (k, v) in value.as_func_map() {
                    let tag = v
                        .as_str_or_ident()
                        .ok_or_else(|| ParseError("rPc entry not a tag".into()))?
                        .to_string();
                    state.r_pc.insert(k, tag);
                }
            }
            "wLocal" => {
                for (k, v) in value.as_func_map() {
                    state.w_local.insert(k, value_to_wlocal(&v)?);
                }
            }
            "rLocal" => {
                for (k, v) in value.as_func_map() {
                    state.r_local.insert(k, value_to_rlocal(&v)?);
                }
            }
            "snapshots" => {
                let seq = value
                    .as_seq()
                    .ok_or_else(|| ParseError("snapshots is not a sequence".into()))?;
                state.snapshots = seq
                    .iter()
                    .map(value_to_snapshot)
                    .collect::<Result<Vec<_>, _>>()?;
            }
            other => {
                return Err(ParseError(format!(
                    "unknown top-level trace variable {other:?}"
                )));
            }
        }
    }
    Ok(state)
}

/// Parses one TLC `-simulate ... file=` trace module into its ordered
/// sequence of states. `STATE_N == ` headers mark the start of each state;
/// continuation lines (TLC wraps long function/record literals) are
/// distinguished from a new top-level assignment by NOT starting with `/\ `
/// at column 0 — confirmed against real trace output, not assumed.
pub fn parse_trace_file(text: &str) -> Result<Vec<ModelState>, ParseError> {
    let mut logical_lines: Vec<String> = Vec::new();
    let mut in_states = false;
    for line in text.lines() {
        let trimmed_start = line.trim_start();
        if trimmed_start.starts_with("STATE_") && trimmed_start.contains("==") {
            in_states = true;
            logical_lines.push(String::new()); // sentinel separating states, replaced below
            continue;
        }
        if trimmed_start.starts_with("====") {
            break;
        }
        if !in_states {
            continue;
        }
        if line.starts_with("/\\ ") {
            logical_lines.push(line.trim_start_matches("/\\ ").to_string());
        } else if let Some(last) = logical_lines.last_mut()
            && !last.is_empty()
        {
            last.push(' ');
            last.push_str(trimmed_start);
        }
    }

    // Re-split on the empty-string sentinels into one Vec<String> of
    // "ident = value" lines per state.
    let mut states_raw: Vec<Vec<String>> = Vec::new();
    for line in logical_lines {
        if line.is_empty() {
            states_raw.push(Vec::new());
        } else if let Some(cur) = states_raw.last_mut() {
            cur.push(line);
        }
    }

    let mut states = Vec::new();
    for raw in states_raw {
        if raw.is_empty() {
            continue;
        }
        let mut assignments = Vec::new();
        for line in raw {
            let Some((name, text)) = line.split_once('=') else {
                return Err(ParseError(format!("malformed assignment line: {line:?}")));
            };
            assignments.push((name.trim().to_string(), text.trim().to_string()));
        }
        states.push(build_state(&assignments)?);
    }
    Ok(states)
}
