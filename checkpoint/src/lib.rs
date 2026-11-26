// Copyright (C) 2026 lfortin
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU Lesser General Public License as published
// by the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful, but
// WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// GNU Lesser General Public License for more details.
//
// You should have received a copy of the GNU Lesser General Public License
// along with this program in the file LICENSE. If not, see
// <https://www.gnu.org/licenses/>.

//! Pretty-printed JSON checkpoints.
//!
//! Big integers are `0x` hexadecimal strings. Older decimal strings still load.
//! The file is replaced by writing
//! `*.json.tmp` and renaming it only after that temp file is fully synced.
//! A leftover temp file is never read.

use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};

use num_bigint::{BigInt, BigUint};
use num_integer::Integer;
use num_traits::Num;
use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// Checkpoint schema version written by this crate.
pub const VERSION: u32 = 1;

/// Result of a resumable run that may stop after a limited number of writes.
pub enum Progress<T> {
    /// The requested digits are ready.
    Finished(T),
    /// The write budget was spent. The checkpoint holds a consistent prefix.
    Paused,
}

/// Next completed-count at which a checkpoint is written.
///
/// Counts move `0 → 1 → 2 → 4 → 8 → …` and then stop on `needed` when that
/// target is not itself a power of two.
pub fn next_doubling(done: u64, needed: u64) -> u64 {
    if done >= needed {
        needed
    } else if done == 0 {
        1.min(needed)
    } else {
        done.saturating_mul(2).min(needed)
    }
}

/// `true` after a successful write that spends the last unit of an active budget.
///
/// `None` means there is no budget and the run should continue.
pub fn note_write(limit: &mut Option<u32>) -> bool {
    let Some(left) = limit else {
        return false;
    };
    *left = left.saturating_sub(1);
    *left == 0
}

/// Write `value` as pretty JSON, then tell the caller whether the write budget is spent.
pub fn store_and_count<T: Serialize + ?Sized>(
    path: &Path,
    value: &T,
    limit: &mut Option<u32>,
) -> Result<bool, String> {
    store_json(path, value)?;
    Ok(note_write(limit))
}

/// Atomically replace `path` with a pretty-printed JSON document.
pub fn store_json<T: Serialize + ?Sized>(path: &Path, value: &T) -> Result<(), String> {
    let tmp = temp_path(path);
    let write_result = (|| {
        let mut file = File::create(&tmp)
            .map_err(|err| format!("could not create checkpoint {}: {err}", tmp.display()))?;
        serde_json::to_writer_pretty(&mut file, value)
            .map_err(|err| format!("could not write checkpoint {}: {err}", tmp.display()))?;
        file.write_all(b"\n")
            .map_err(|err| format!("could not write checkpoint {}: {err}", tmp.display()))?;
        file.sync_all()
            .map_err(|err| format!("could not write checkpoint {}: {err}", tmp.display()))?;
        Ok(())
    })();
    if let Err(err) = write_result {
        let _ = std::fs::remove_file(&tmp);
        return Err(err);
    }
    std::fs::rename(&tmp, path)
        .map_err(|err| format!("could not replace checkpoint {}: {err}", path.display()))
}

/// Load a checkpoint. A missing file is `Ok(None)`. A temp file is never opened.
pub fn load_json<T: for<'de> Deserialize<'de>>(path: &Path) -> Result<Option<T>, String> {
    match File::open(path) {
        Ok(file) => serde_json::from_reader(file)
            .map(Some)
            .map_err(|err| format!("could not read checkpoint {}: {err}", path.display())),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(err) => Err(format!(
            "could not read checkpoint {}: {err}",
            path.display()
        )),
    }
}

/// Finished digits already stored for this tool and algorithm.
///
/// When `allow_shorter` is set and the file holds more digits than `digits`,
/// the saved expansion is rounded half up to `digits`. The series integers are
/// not parsed.
pub fn cached_output(
    path: &Path,
    expected_tool: &str,
    expected_algo: &str,
    digits: u32,
    allow_shorter: bool,
) -> Result<Option<String>, String> {
    let Some(saved) = read_saved_digits(path)? else {
        return Ok(None);
    };
    require_header(
        path,
        saved.version,
        &saved.tool,
        &saved.algo,
        expected_tool,
        expected_algo,
    )?;
    let (Some(stored), Some(text)) = (saved.digits, saved.text) else {
        return Ok(None);
    };
    if stored < digits || text.is_empty() {
        return Ok(None);
    }
    if stored > digits && !allow_shorter {
        return Ok(None);
    }
    Ok(round_saved(&text, stored, digits))
}

/// Round a saved significant-digit string half up to `to_digits`.
///
/// `None` means the text is not a `from_digits` expansion, or rounding would
/// add a digit.
fn round_saved(text: &str, from_digits: u32, to_digits: u32) -> Option<String> {
    if to_digits == 0 || to_digits > from_digits {
        return None;
    }
    let mut sig: Vec<u8> = text.bytes().filter(|byte| *byte != b'.').collect();
    if sig.len() != from_digits as usize || !sig.iter().all(u8::is_ascii_digit) {
        return None;
    }
    if to_digits < from_digits && sig[to_digits as usize] >= b'5' {
        let mut index = to_digits as usize;
        let mut carry = true;
        while carry {
            if index == 0 {
                return None;
            }
            index -= 1;
            if sig[index] < b'9' {
                sig[index] += 1;
                carry = false;
            } else {
                sig[index] = b'0';
            }
        }
    }
    sig.truncate(to_digits as usize);
    let body = std::str::from_utf8(&sig).ok()?;
    if to_digits == 1 {
        Some(body.to_string())
    } else {
        let mut out = String::with_capacity(body.len() + 1);
        out.push(body.as_bytes()[0] as char);
        out.push('.');
        out.push_str(&body[1..]);
        Some(out)
    }
}

/// Header fields a resume can use without parsing the series integers.
pub struct CheckpointSummary {
    /// Schema version.
    pub version: u32,
    /// Binary that wrote the file, such as `"pi"`.
    pub tool: String,
    /// Algorithm name, such as `"chudnovsky"`.
    pub algo: String,
    /// Significant digits stored in [`Self::text`], when a run has finished.
    pub digits: Option<u32>,
    /// Finished digits, when a run has finished.
    pub text: Option<String>,
    /// Unrounded fixed-point digits, including guard digits, when a run saved them.
    pub seed: Option<String>,
    /// Series terms or convergent steps already merged, when that field exists.
    pub terms_done: Option<u32>,
}

/// Read the checkpoint header, including `terms_done` when present.
///
/// Large integer fields are skipped.
pub fn read_summary(path: &Path) -> Result<Option<CheckpointSummary>, String> {
    read_saved_digits(path)
}

/// Read only the header and the saved digits. Large integer fields are skipped.
fn read_saved_digits(path: &Path) -> Result<Option<CheckpointSummary>, String> {
    let bytes = match std::fs::read(path) {
        Ok(bytes) => bytes,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(err) => {
            return Err(format!(
                "could not read checkpoint {}: {err}",
                path.display()
            ));
        }
    };
    let mut cur = Cursor {
        bytes: &bytes,
        i: 0,
        path,
    };
    cur.skip_ws();
    cur.eat(b'{')?;
    let mut version = None;
    let mut tool = None;
    let mut algo = None;
    let mut digits = None;
    let mut text = None;
    let mut seed = None;
    let mut terms_done = None;
    loop {
        cur.skip_ws();
        if cur.peek() == Some(b'}') {
            break;
        }
        let key = cur.string(true)?.unwrap_or_default();
        cur.skip_ws();
        cur.eat(b':')?;
        cur.skip_ws();
        match key.as_str() {
            "version" => version = Some(cur.u32()?),
            "digits" => digits = Some(cur.u32()?),
            "tool" => tool = cur.string(true)?,
            "algo" => algo = cur.string(true)?,
            "text" => text = cur.string(true)?,
            "seed" => seed = cur.string(true)?,
            "terms_done" => terms_done = Some(cur.u32()?),
            _ => cur.skip_value()?,
        }
        cur.skip_ws();
        match cur.peek() {
            Some(b',') => cur.i += 1,
            Some(b'}') => break,
            _ => {
                return Err(format!(
                    "could not read checkpoint {}: expected ',' or '}}'",
                    path.display()
                ));
            }
        }
    }
    let (Some(version), Some(tool), Some(algo)) = (version, tool, algo) else {
        return Err(format!(
            "checkpoint {} is missing version, tool, or algo",
            path.display()
        ));
    };
    Ok(Some(CheckpointSummary {
        version,
        tool,
        algo,
        digits,
        text,
        seed,
        terms_done,
    }))
}

/// Rounded display text and the unrounded fixed-point digits of `scaled`.
///
/// `scaled` is an integer with `guard_digits` digits below the last displayed digit.
/// The seed keeps those digits so a later Newton step can double the precision once.
pub fn display_and_seed(
    scaled: &BigUint,
    digits: u32,
    guard_digits: u32,
) -> Result<(String, String), String> {
    if guard_digits == 0 || digits == 0 {
        return Err("internal error: rounding produced an unexpected length".to_string());
    }
    let guard = BigUint::from(10u32).pow(guard_digits);
    let half = &guard / 2u32;
    let (kept, remainder) = scaled.div_rem(&guard);
    let kept_text = kept.to_string();
    if kept_text.len() != digits as usize {
        return Err("internal error: rounding produced an unexpected length".to_string());
    }
    let rem_text = remainder.to_string();
    if rem_text.len() > guard_digits as usize {
        return Err("internal error: rounding produced an unexpected length".to_string());
    }
    let mut seed = String::with_capacity(digits as usize + guard_digits as usize);
    seed.push_str(&kept_text);
    seed.extend(std::iter::repeat_n(
        '0',
        guard_digits as usize - rem_text.len(),
    ));
    seed.push_str(&rem_text);
    let mut chars = kept_text.into_bytes();
    if remainder >= half {
        let mut carry = true;
        let mut index = chars.len();
        while carry && index > 0 {
            index -= 1;
            if chars[index] == b'9' {
                chars[index] = b'0';
            } else {
                chars[index] += 1;
                carry = false;
            }
        }
        if carry {
            return Err("internal error: rounding produced an unexpected length".to_string());
        }
    }
    let mut text = String::from_utf8(chars)
        .map_err(|_| "internal error: rounding produced an unexpected length".to_string())?;
    if digits != 1 {
        text.insert(1, '.');
    }
    Ok((text, seed))
}

/// Decimal fields selected by [`load_decimals`].
pub struct DecimalFields {
    /// Values for the unsigned keys, in request order.
    pub uints: Vec<BigUint>,
    /// Values for the signed keys, in request order.
    pub ints: Vec<BigInt>,
}

/// Parse selected decimal-string fields, skipping every other value.
///
/// A missing file is `Ok(None)`. A missing key is an error.
pub fn load_decimals(
    path: &Path,
    uint_keys: &[&str],
    int_keys: &[&str],
) -> Result<Option<DecimalFields>, String> {
    let bytes = match std::fs::read(path) {
        Ok(bytes) => bytes,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(err) => {
            return Err(format!(
                "could not read checkpoint {}: {err}",
                path.display()
            ));
        }
    };
    let mut wanted = Vec::with_capacity(uint_keys.len() + int_keys.len());
    for key in uint_keys {
        wanted.push(Wanted {
            key,
            signed: false,
            range: None,
        });
    }
    for key in int_keys {
        wanted.push(Wanted {
            key,
            signed: true,
            range: None,
        });
    }
    let mut cur = Cursor {
        bytes: &bytes,
        i: 0,
        path,
    };
    cur.skip_ws();
    cur.eat(b'{')?;
    loop {
        cur.skip_ws();
        if cur.peek() == Some(b'}') {
            break;
        }
        let key = cur.string(true)?.unwrap_or_default();
        cur.skip_ws();
        cur.eat(b':')?;
        cur.skip_ws();
        if let Some(item) = wanted.iter_mut().find(|item| *item.key == key) {
            item.range = Some(cur.quoted_range()?);
        } else {
            cur.skip_value()?;
        }
        cur.skip_ws();
        match cur.peek() {
            Some(b',') => cur.i += 1,
            Some(b'}') => break,
            _ => return Err(cur.err("expected ',' or '}'")),
        }
    }
    if wanted.iter().any(|item| item.range.is_none()) {
        return Err(format!(
            "checkpoint {} is missing a requested integer field",
            path.display()
        ));
    }

    let mut parsed: Vec<BigInt> = Vec::with_capacity(wanted.len());
    let bytes = bytes.as_slice();
    std::thread::scope(|scope| {
        let mut jobs = Vec::with_capacity(wanted.len());
        for item in &wanted {
            let (start, end) = item.range.unwrap();
            let signed = item.signed;
            jobs.push(scope.spawn(move || {
                let text = std::str::from_utf8(&bytes[start..end])
                    .map_err(|_| format!("could not read checkpoint {}", path.display()))?;
                if signed {
                    parse_int_stored(text)
                } else {
                    parse_uint_stored(text).map(BigInt::from)
                }
            }));
        }
        for job in jobs {
            parsed.push(
                job.join()
                    .map_err(|_| format!("could not read checkpoint {}", path.display()))??,
            );
        }
        Ok::<(), String>(())
    })?;

    let mut uints = Vec::with_capacity(uint_keys.len());
    let mut ints = Vec::with_capacity(int_keys.len());
    for (item, value) in wanted.into_iter().zip(parsed) {
        if item.signed {
            ints.push(value);
        } else {
            uints.push(
                value
                    .to_biguint()
                    .ok_or_else(|| format!("could not read checkpoint {}", path.display()))?,
            );
        }
    }
    Ok(Some(DecimalFields { uints, ints }))
}

struct Wanted<'a> {
    key: &'a str,
    signed: bool,
    range: Option<(usize, usize)>,
}

struct Cursor<'a> {
    bytes: &'a [u8],
    i: usize,
    path: &'a Path,
}

impl Cursor<'_> {
    fn err(&self, message: &str) -> String {
        format!(
            "could not read checkpoint {}: {message}",
            self.path.display()
        )
    }

    fn peek(&self) -> Option<u8> {
        self.bytes.get(self.i).copied()
    }

    fn skip_ws(&mut self) {
        while matches!(self.peek(), Some(b' ' | b'\n' | b'\r' | b'\t')) {
            self.i += 1;
        }
    }

    fn eat(&mut self, expected: u8) -> Result<(), String> {
        if self.peek() == Some(expected) {
            self.i += 1;
            Ok(())
        } else {
            Err(self.err("unexpected JSON"))
        }
    }

    fn value_bounds(&mut self) -> Result<(usize, usize), String> {
        let start = self.i;
        self.skip_value()?;
        Ok((start, self.i))
    }

    fn quoted_range(&mut self) -> Result<(usize, usize), String> {
        self.eat(b'"')?;
        let start = self.i;
        while let Some(byte) = self.peek() {
            match byte {
                b'\\' => return Err(self.err("unsupported escape in an integer")),
                b'"' => {
                    let end = self.i;
                    self.i += 1;
                    return Ok((start, end));
                }
                _ => self.i += 1,
            }
        }
        Err(self.err("unterminated string"))
    }

    fn string(&mut self, keep: bool) -> Result<Option<String>, String> {
        self.eat(b'"')?;
        let start = self.i;
        let mut escaped = false;
        while let Some(byte) = self.peek() {
            match byte {
                b'\\' => {
                    escaped = true;
                    self.i += 2;
                    if self.i > self.bytes.len() {
                        return Err(self.err("unterminated string"));
                    }
                }
                b'"' => {
                    let end = self.i;
                    self.i += 1;
                    if !keep {
                        return Ok(None);
                    }
                    if !escaped {
                        let text = std::str::from_utf8(&self.bytes[start..end])
                            .map_err(|_| self.err("text is not UTF-8"))?;
                        return Ok(Some(text.to_string()));
                    }
                    return Ok(Some(self.unescape(start, end)?));
                }
                _ => self.i += 1,
            }
        }
        Err(self.err("unterminated string"))
    }

    fn unescape(&self, start: usize, end: usize) -> Result<String, String> {
        let mut out = String::new();
        let mut i = start;
        while i < end {
            if self.bytes[i] == b'\\' {
                i += 1;
                if i >= end {
                    return Err(self.err("unterminated escape"));
                }
                match self.bytes[i] {
                    b'"' => out.push('"'),
                    b'\\' => out.push('\\'),
                    b'/' => out.push('/'),
                    b'n' => out.push('\n'),
                    b'r' => out.push('\r'),
                    b't' => out.push('\t'),
                    _ => return Err(self.err("unsupported escape")),
                }
            } else {
                out.push(self.bytes[i] as char);
            }
            i += 1;
        }
        Ok(out)
    }

    fn u32(&mut self) -> Result<u32, String> {
        let start = self.i;
        while matches!(self.peek(), Some(b'0'..=b'9')) {
            self.i += 1;
        }
        if start == self.i {
            return Err(self.err("expected a number"));
        }
        let text = std::str::from_utf8(&self.bytes[start..self.i])
            .map_err(|_| self.err("number is not UTF-8"))?;
        text.parse::<u32>()
            .map_err(|_| self.err("number is out of range"))
    }

    fn skip_value(&mut self) -> Result<(), String> {
        match self.peek() {
            Some(b'"') => {
                self.string(false)?;
                Ok(())
            }
            Some(b'-' | b'0'..=b'9') => {
                if self.peek() == Some(b'-') {
                    self.i += 1;
                }
                let _ = self.u32()?;
                Ok(())
            }
            Some(b'{') => self.skip_container(b'{', b'}'),
            Some(b'[') => self.skip_container(b'[', b']'),
            Some(b't') => self.eat_literal(b"true"),
            Some(b'f') => self.eat_literal(b"false"),
            Some(b'n') => self.eat_literal(b"null"),
            _ => Err(self.err("unexpected JSON value")),
        }
    }

    fn skip_container(&mut self, open: u8, close: u8) -> Result<(), String> {
        self.eat(open)?;
        let mut depth = 1u32;
        let mut in_string = false;
        let mut escaped = false;
        while self.i < self.bytes.len() {
            let byte = self.bytes[self.i];
            self.i += 1;
            if in_string {
                if escaped {
                    escaped = false;
                } else if byte == b'\\' {
                    escaped = true;
                } else if byte == b'"' {
                    in_string = false;
                }
                continue;
            }
            match byte {
                b'"' => in_string = true,
                b if b == open => depth += 1,
                b if b == close => {
                    depth -= 1;
                    if depth == 0 {
                        return Ok(());
                    }
                }
                _ => {}
            }
        }
        Err(self.err("unterminated JSON value"))
    }

    fn eat_literal(&mut self, literal: &[u8]) -> Result<(), String> {
        if self.bytes[self.i..].starts_with(literal) {
            self.i += literal.len();
            Ok(())
        } else {
            Err(self.err("unexpected JSON literal"))
        }
    }
}

/// Record the formatted result on an existing checkpoint.
///
/// A result for fewer digits does not replace one for more digits. The series
/// integers stay as they are on disk; this does not parse them.
pub fn store_output(path: &Path, digits: u32, text: &str) -> Result<(), String> {
    let bytes = match std::fs::read(path) {
        Ok(bytes) => bytes,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(err) => {
            return Err(format!(
                "could not read checkpoint {}: {err}",
                path.display()
            ));
        }
    };
    let mut cur = Cursor {
        bytes: &bytes,
        i: 0,
        path,
    };
    cur.skip_ws();
    let brace = cur.i;
    cur.eat(b'{')?;
    let mut digits_at: Option<(usize, usize)> = None;
    let mut text_at: Option<(usize, usize)> = None;
    let mut empty = false;
    loop {
        cur.skip_ws();
        if cur.peek() == Some(b'}') {
            empty = digits_at.is_none() && text_at.is_none() && cur.i == brace + 1
                || (digits_at.is_none() && text_at.is_none() && only_ws(&bytes[brace + 1..cur.i]));
            break;
        }
        let key = cur.string(true)?.unwrap_or_default();
        cur.skip_ws();
        cur.eat(b':')?;
        cur.skip_ws();
        let (start, end) = cur.value_bounds()?;
        match key.as_str() {
            "digits" => {
                let token = std::str::from_utf8(&bytes[start..end])
                    .map_err(|_| format!("could not read checkpoint {}", path.display()))?;
                if token.bytes().all(|byte| byte.is_ascii_digit()) {
                    let prev: u32 = token.parse().map_err(|_| {
                        format!("could not read checkpoint {}", path.display())
                    })?;
                    if prev > digits {
                        return Ok(());
                    }
                }
                digits_at = Some((start, end));
            }
            "text" => text_at = Some((start, end)),
            _ => {}
        }
        cur.skip_ws();
        match cur.peek() {
            Some(b',') => cur.i += 1,
            Some(b'}') => break,
            _ => return Err(cur.err("expected ',' or '}'")),
        }
    }

    let quoted = json_string(text);
    if empty {
        let fresh = format!("{{\n  \"digits\": {digits},\n  \"text\": {quoted}\n}}\n");
        return write_bytes(path, fresh.as_bytes());
    }

    let digits_token = digits.to_string();
    let mut prefix = String::new();
    if digits_at.is_none() {
        prefix.push_str("\n  \"digits\": ");
        prefix.push_str(&digits_token);
        prefix.push(',');
    }
    if text_at.is_none() {
        prefix.push_str("\n  \"text\": ");
        prefix.push_str(&quoted);
        prefix.push(',');
    }
    let mut reps = Vec::new();
    if let Some((start, end)) = digits_at {
        reps.push((start, end, digits_token.as_str()));
    }
    if let Some((start, end)) = text_at {
        reps.push((start, end, quoted.as_str()));
    }
    reps.sort_by_key(|item| item.0);

    let mut out = Vec::with_capacity(bytes.len() + prefix.len() + quoted.len());
    out.extend_from_slice(&bytes[..=brace]);
    out.extend_from_slice(prefix.as_bytes());
    let mut cursor = brace + 1;
    for (start, end, replacement) in reps {
        if start < cursor {
            return Err(format!(
                "could not read checkpoint {}",
                path.display()
            ));
        }
        out.extend_from_slice(&bytes[cursor..start]);
        out.extend_from_slice(replacement.as_bytes());
        cursor = end;
    }
    out.extend_from_slice(&bytes[cursor..]);
    write_bytes(path, &out)
}

fn only_ws(bytes: &[u8]) -> bool {
    bytes
        .iter()
        .all(|byte| matches!(byte, b' ' | b'\n' | b'\r' | b'\t'))
}

fn json_string(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 2);
    out.push('"');
    for byte in text.bytes() {
        match byte {
            b'"' => out.push_str("\\\""),
            b'\\' => out.push_str("\\\\"),
            b'\n' => out.push_str("\\n"),
            b'\r' => out.push_str("\\r"),
            b'\t' => out.push_str("\\t"),
            byte if byte < 0x20 => {
                out.push_str(&format!("\\u{byte:04x}"));
            }
            byte => out.push(byte as char),
        }
    }
    out.push('"');
    out
}

fn write_bytes(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let tmp = temp_path(path);
    let write_result = (|| {
        let mut file = File::create(&tmp)
            .map_err(|err| format!("could not create checkpoint {}: {err}", tmp.display()))?;
        file.write_all(bytes)
            .map_err(|err| format!("could not write checkpoint {}: {err}", tmp.display()))?;
        file.sync_all()
            .map_err(|err| format!("could not write checkpoint {}: {err}", tmp.display()))?;
        Ok(())
    })();
    if let Err(err) = write_result {
        let _ = std::fs::remove_file(&tmp);
        return Err(err);
    }
    std::fs::rename(&tmp, path)
        .map_err(|err| format!("could not replace checkpoint {}: {err}", path.display()))
}

/// Integer square root of `n` from a seed that is already correct to about `correct` digits.
///
/// Each Newton step doubles the correct digits, up to `scale`. The last adjustment is at most one.
pub fn isqrt_from_seed(n: &BigUint, mut x: BigUint, mut correct: u32, scale: u32) -> BigUint {
    if x == BigUint::from(0u32) {
        return n.sqrt();
    }
    let mut steps = 0u32;
    while correct < scale && steps < 64 {
        let next = (&x + n / &x) >> 1;
        if next == x {
            break;
        }
        x = next;
        correct = correct.saturating_mul(2).max(correct + 1);
        steps += 1;
    }
    polish_isqrt(n, x).unwrap_or_else(|| n.sqrt())
}

fn polish_isqrt(n: &BigUint, mut x: BigUint) -> Option<BigUint> {
    let mut square = &x * &x;
    for _ in 0..4 {
        if &square > n {
            if x == BigUint::from(0u32) {
                return None;
            }
            let delta = (&x << 1) - 1u32;
            if square < delta {
                return None;
            }
            x -= 1u32;
            square -= delta;
        } else {
            let next_square = &square + (&x << 1) + 1u32;
            if &next_square > n {
                return Some(x);
            }
            x += 1u32;
            square = next_square;
        }
    }
    if &square <= n {
        let next_square = &square + (&x << 1) + 1u32;
        if &next_square > n {
            return Some(x);
        }
    }
    None
}

/// Reject a checkpoint written for a different version, tool, or algorithm.
pub fn require_header(
    path: &Path,
    version: u32,
    tool: &str,
    algo: &str,
    expected_tool: &str,
    expected_algo: &str,
) -> Result<(), String> {
    if version != VERSION {
        return Err(format!(
            "checkpoint {} has version {version}; this program reads version {VERSION}",
            path.display()
        ));
    }
    if tool != expected_tool || algo != expected_algo {
        return Err(format!(
            "checkpoint {} is for {tool} {algo}; this run is {expected_tool} {expected_algo}",
            path.display()
        ));
    }
    Ok(())
}

fn temp_path(path: &Path) -> PathBuf {
    let mut name = path
        .file_name()
        .map(|name| name.to_os_string())
        .unwrap_or_default();
    name.push(".tmp");
    path.with_file_name(name)
}

/// Parse a decimal integer.
///
/// Multi-million-digit values are built by balanced products of 9-digit
/// chunks. `BigUint`'s own decimal parser multiplies one chunk at a time.
pub fn parse_uint_decimal(text: &str) -> Result<BigUint, String> {
    let bytes = text.as_bytes();
    if bytes.is_empty() || bytes.iter().any(|byte| !byte.is_ascii_digit()) {
        return Err("invalid decimal integer".to_string());
    }
    if bytes.len() <= 19 {
        let mut value = 0u64;
        for &byte in bytes {
            value = value * 10 + u64::from(byte - b'0');
        }
        return Ok(BigUint::from(value));
    }

    const CHUNK: usize = 9;
    let pad = (CHUNK - bytes.len() % CHUNK) % CHUNK;
    let mut chunks = Vec::with_capacity((bytes.len() + pad) / CHUNK);
    let mut acc = 0u32;
    for (index, &byte) in bytes.iter().enumerate() {
        acc = acc * 10 + u32::from(byte - b'0');
        if (index + pad + 1).is_multiple_of(CHUNK) {
            chunks.push(BigUint::from(acc));
            acc = 0;
        }
    }
    let mut base = BigUint::from(1_000_000_000u32);
    while chunks.len() > 1 {
        let mut pairs = Vec::with_capacity(chunks.len() / 2);
        while chunks.len() >= 2 {
            let low = chunks.pop().unwrap();
            let high = chunks.pop().unwrap();
            pairs.push(high * &base + low);
        }
        pairs.reverse();
        chunks.append(&mut pairs);
        base = &base * &base;
    }
    Ok(chunks.pop().unwrap_or_else(|| BigUint::from(0u32)))
}

/// Integer made from the significant digits in `text`.
///
/// A single decimal point is ignored. `digits` is the required count.
pub fn significant_integer(text: &str, digits: u32) -> Option<BigUint> {
    let mut sig = String::with_capacity(digits as usize);
    let mut dot = false;
    for byte in text.bytes() {
        match byte {
            b'0'..=b'9' => sig.push(byte as char),
            b'.' if !dot => dot = true,
            _ => return None,
        }
    }
    if sig.len() != digits as usize {
        return None;
    }
    parse_uint_decimal(&sig).ok()
}

/// Parse a decimal integer with an optional leading `-`.
pub fn parse_int_decimal(text: &str) -> Result<BigInt, String> {
    let (negative, digits) = match text.strip_prefix('-') {
        Some(digits) => (true, digits),
        None => (false, text),
    };
    let magnitude = parse_uint_decimal(digits)?;
    if negative && magnitude != BigUint::from(0u32) {
        Ok(-BigInt::from(magnitude))
    } else {
        Ok(BigInt::from(magnitude))
    }
}

/// Read a stored integer. `0x` prefixes a hexadecimal value; otherwise it is decimal.
pub fn parse_uint_stored(text: &str) -> Result<BigUint, String> {
    if let Some(hex) = text.strip_prefix("0x").or_else(|| text.strip_prefix("0X")) {
        if hex.is_empty() || !hex.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err("invalid hexadecimal integer".to_string());
        }
        return BigUint::from_str_radix(hex, 16)
            .map_err(|_| "invalid hexadecimal integer".to_string());
    }
    parse_uint_decimal(text)
}

/// Read a stored signed integer, with an optional leading `-`.
pub fn parse_int_stored(text: &str) -> Result<BigInt, String> {
    let (negative, digits) = match text.strip_prefix('-') {
        Some(digits) => (true, digits),
        None => (false, text),
    };
    let magnitude = parse_uint_stored(digits)?;
    if negative && magnitude != BigUint::from(0u32) {
        Ok(-BigInt::from(magnitude))
    } else {
        Ok(BigInt::from(magnitude))
    }
}

fn format_uint_stored(value: &BigUint) -> String {
    format!("0x{}", value.to_str_radix(16))
}

fn format_int_stored(value: &BigInt) -> String {
    let text = value.to_str_radix(16);
    if let Some(magnitude) = text.strip_prefix('-') {
        format!("-0x{magnitude}")
    } else {
        format!("0x{text}")
    }
}

/// Hexadecimal-string serde for [`BigUint`]. Decimal strings still load.
pub mod uint {
    use super::*;

    pub fn serialize<S: Serializer>(value: &BigUint, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&format_uint_stored(value))
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<BigUint, D::Error> {
        let text = String::deserialize(deserializer)?;
        parse_uint_stored(&text).map_err(D::Error::custom)
    }

    /// Serde for an optional integer that is omitted when absent.
    pub mod opt {
        use super::*;

        pub fn serialize<S: Serializer>(
            value: &Option<BigUint>,
            serializer: S,
        ) -> Result<S::Ok, S::Error> {
            match value {
                Some(value) => serializer.serialize_str(&crate::format_uint_stored(value)),
                None => serializer.serialize_none(),
            }
        }

        pub fn deserialize<'de, D: Deserializer<'de>>(
            deserializer: D,
        ) -> Result<Option<BigUint>, D::Error> {
            let text = Option::<String>::deserialize(deserializer)?;
            text.map(|text| crate::parse_uint_stored(&text).map_err(D::Error::custom))
                .transpose()
        }
    }
}

/// Hexadecimal-string serde for [`BigInt`], with a leading `-` when negative.
pub mod int {
    use super::*;

    pub fn serialize<S: Serializer>(value: &BigInt, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&format_int_stored(value))
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<BigInt, D::Error> {
        let text = String::deserialize(deserializer)?;
        parse_int_stored(&text).map_err(D::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decimal_parser_matches_the_builtin_parser() {
        for digits in [1usize, 8, 9, 10, 18, 19, 20, 21, 36, 100, 250, 1000] {
            let text: String = (0..digits)
                .map(|index| char::from(b'0' + ((index * 7 + 3) % 10) as u8))
                .collect();
            let parsed = parse_uint_decimal(&text).unwrap();
            assert_eq!(parsed, text.parse::<BigUint>().unwrap(), "{digits}");
            let padded = format!("00{text}");
            assert_eq!(
                parse_uint_decimal(&padded).unwrap(),
                padded.parse::<BigUint>().unwrap()
            );
        }
        assert_eq!(parse_uint_decimal("0").unwrap(), BigUint::from(0u32));
        assert!(parse_uint_decimal("").is_err());
        assert!(parse_uint_decimal("12a").is_err());
        assert_eq!(
            parse_int_decimal("-13591409").unwrap(),
            BigInt::from(-13_591_409)
        );
        assert_eq!(parse_int_decimal("-0").unwrap(), BigInt::from(0));
    }

    #[test]
    fn doubling_schedule_hits_powers_of_two_then_the_target() {
        let mut done = 0u64;
        let mut seen = Vec::new();
        while done < 1000 {
            done = next_doubling(done, 1000);
            seen.push(done);
        }
        assert_eq!(seen, vec![1, 2, 4, 8, 16, 32, 64, 128, 256, 512, 1000]);
        assert_eq!(next_doubling(1000, 1000), 1000);
        assert_eq!(next_doubling(1500, 1000), 1000);
    }

    #[test]
    fn lifted_sqrt_matches_the_builtin_sqrt() {
        for scale in [1u32, 2, 5, 8, 20, 40] {
            let n = BigUint::from(5u32) * BigUint::from(10u32).pow(scale * 2);
            let mut x = BigUint::from(2u32);
            let mut correct = 1u32;
            let mut done = 0u32;
            while done < scale {
                let next = if done == 0 { 1 } else { (done * 2).min(scale) };
                let rad = BigUint::from(5u32) * BigUint::from(10u32).pow(next * 2);
                x = isqrt_from_seed(&rad, x * BigUint::from(10u32).pow(next - done), correct, next);
                correct = next.max(1);
                done = next;
                assert_eq!(x, rad.sqrt(), "scale {done}");
            }
            let direct = isqrt_from_seed(&n, BigUint::from(2u32), 1, scale);
            assert_eq!(direct, n.sqrt(), "direct {scale}");
        }
    }

    #[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Sample {
        version: u32,
        tool: String,
        algo: String,
        terms_done: u32,
        #[serde(with = "uint")]
        p: BigUint,
        #[serde(with = "int")]
        t: BigInt,
    }

    #[test]
    fn round_trip_replaces_the_file_and_ignores_a_missing_path() {
        let dir = std::env::temp_dir().join(format!("consts-checkpoint-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("pi-chudnovsky.json");
        assert!(load_json::<Sample>(&path).unwrap().is_none());

        let sample = Sample {
            version: 1,
            tool: "pi".to_string(),
            algo: "chudnovsky".to_string(),
            terms_done: 32,
            p: BigUint::from(1u32),
            t: BigInt::from(-13591409),
        };
        store_json(&path, &sample).unwrap();
        let loaded: Sample = load_json(&path).unwrap().unwrap();
        assert_eq!(loaded, sample);
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains("\"t\": \"-0xcf6371\""));
        assert!(!dir.join("pi-chudnovsky.json.tmp").exists());

        std::fs::write(dir.join("pi-chudnovsky.json.tmp"), "{").unwrap();
        assert_eq!(load_json::<Sample>(&path).unwrap().unwrap(), sample);

        let broken = dir.join("broken.json");
        std::fs::write(&broken, "{").unwrap();
        assert!(load_json::<Sample>(&broken).is_err());

        let extra = dir.join("extra.json");
        std::fs::write(&extra, "{\"version\":1,\"tool\":\"pi\",\"algo\":\"chudnovsky\",\"terms_done\":1,\"p\":\"1\",\"t\":\"1\",\"extra\":true}\n").unwrap();
        assert!(load_json::<Sample>(&extra).is_err());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn cached_output_replays_the_same_count_and_keeps_a_larger_one() {
        let dir =
            std::env::temp_dir().join(format!("consts-checkpoint-out-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("pi-chudnovsky.json");
        store_json(
            &path,
            &serde_json::json!({
                "version": 1,
                "tool": "pi",
                "algo": "chudnovsky",
                "terms_done": 4,
                "p": "1",
                "q": "1",
                "t": "1"
            }),
        )
        .unwrap();
        assert!(
            cached_output(&path, "pi", "chudnovsky", 10, true)
                .unwrap()
                .is_none()
        );
        store_output(&path, 10, "3.141592654").unwrap();
        let raw = std::fs::read_to_string(&path).unwrap();
        assert!(raw.contains("\"p\": \"1\""));
        assert!(raw.contains("\"q\": \"1\""));
        assert!(raw.contains("\"t\": \"1\""));
        assert_eq!(
            cached_output(&path, "pi", "chudnovsky", 10, true)
                .unwrap()
                .as_deref(),
            Some("3.141592654")
        );
        store_output(&path, 4, "3.142").unwrap();
        assert_eq!(
            cached_output(&path, "pi", "chudnovsky", 10, true)
                .unwrap()
                .as_deref(),
            Some("3.141592654")
        );
        assert_eq!(
            cached_output(&path, "pi", "chudnovsky", 5, true)
                .unwrap()
                .as_deref(),
            Some("3.1416")
        );
        assert!(
            cached_output(&path, "pi", "chudnovsky", 5, false)
                .unwrap()
                .is_none()
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}
