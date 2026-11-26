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

//! JSON checkpoints for pi.
//!
//! Series accumulators are exact and can be extended to a higher digit count.
//! Machin's Taylor series is stored at one scale; a different digit count is refused
//! and the file is left unchanged.

use std::path::Path;

use checkpoint::Progress;
use num_bigint::{BigInt, BigUint};
use num_traits::Zero;
use serde::{Deserialize, Serialize};

use super::{
    AtanSplit, CHUD_C, ChudSplit, GUARD_DIGITS, PiAlgo, format_scaled_pi, merge_arctan,
    merge_chudnovsky, scale_for_digits, split_arctan, split_chudnovsky, terms_needed,
};

/// Format `digits` significant digits of pi, reading and writing a JSON checkpoint at `path`.
///
/// A missing file starts from scratch. `chudnovsky` and `binary-split` continue from the
/// saved sums, including when this request asks for more digits than the file holds.
/// `machin` continues only when the file was written for the same digit count.
pub fn format_pi_resumable(digits: u32, algo: PiAlgo, path: &Path) -> Result<String, String> {
    match run(digits, algo, path, None)? {
        Progress::Finished(text) => Ok(text),
        Progress::Paused => {
            Err("internal error: checkpoint paused without a write limit".to_string())
        }
    }
}

fn run(
    digits: u32,
    algo: PiAlgo,
    path: &Path,
    mut limit: Option<u32>,
) -> Result<Progress<String>, String> {
    let scale = scale_for_digits(digits).map_err(str::to_string)?;
    if limit.is_none()
        && let Some(text) = checkpoint::cached_output(
            path,
            "pi",
            &algo.to_string(),
            digits,
            !matches!(algo, PiAlgo::Machin),
        )?
    {
        return Ok(Progress::Finished(text));
    }
    let one = BigUint::from(10u32).pow(scale);
    let scaled = match algo {
        PiAlgo::Machin => match machin(&one, scale, path, &mut limit)? {
            Progress::Paused => return Ok(Progress::Paused),
            Progress::Finished(value) => value,
        },
        PiAlgo::BinarySplit => match binary_split(&one, scale, path, &mut limit)? {
            Progress::Paused => return Ok(Progress::Paused),
            Progress::Finished(value) => value,
        },
        PiAlgo::Chudnovsky => match chudnovsky(&one, scale, path, &mut limit)? {
            Progress::Paused => return Ok(Progress::Paused),
            Progress::Finished(value) => value,
        },
    };
    let text = format_scaled_pi(scaled, digits).map_err(str::to_string)?;
    if limit.is_none() {
        checkpoint::store_output(path, digits, &text)?;
    }
    Ok(Progress::Finished(text))
}

fn save<T: Serialize>(path: &Path, value: &T, limit: &mut Option<u32>) -> Result<bool, String> {
    checkpoint::store_and_count(path, value, limit)
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ChudFile {
    version: u32,
    tool: String,
    algo: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    digits: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    text: Option<String>,
    terms_done: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    root_scale: Option<u32>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "checkpoint::uint::opt"
    )]
    root: Option<BigUint>,
    #[serde(with = "checkpoint::uint")]
    p: BigUint,
    #[serde(with = "checkpoint::uint")]
    q: BigUint,
    #[serde(with = "checkpoint::int")]
    t: BigInt,
}

fn chudnovsky(
    one: &BigUint,
    scale: u32,
    path: &Path,
    limit: &mut Option<u32>,
) -> Result<Progress<BigUint>, String> {
    let needed = super::chudnovsky_terms(scale);
    if let Some(value) = chudnovsky_from_saved(one, scale, path, needed)? {
        return Ok(Progress::Finished(value));
    }
    let c = BigUint::from(CHUD_C);
    let q_factor = (&c * &c * c) / 24u32;
    let (mut split, mut terms_done, saved_root) = match checkpoint::load_json::<ChudFile>(path)? {
        None => (
            ChudSplit {
                p: BigUint::from(1u32),
                q: BigUint::from(1u32),
                t: BigInt::from(0),
            },
            0,
            None,
        ),
        Some(file) => {
            checkpoint::require_header(
                path,
                file.version,
                &file.tool,
                &file.algo,
                "pi",
                "chudnovsky",
            )?;
            let root = match (file.root_scale, file.root) {
                (Some(root_scale), Some(root)) => Some((root_scale, root)),
                _ => None,
            };
            (
                ChudSplit {
                    p: file.p,
                    q: file.q,
                    t: file.t,
                },
                file.terms_done,
                root,
            )
        }
    };

    while terms_done < needed {
        let end = checkpoint::next_doubling(u64::from(terms_done), u64::from(needed)) as u32;
        let block = split_chudnovsky(terms_done, end, &q_factor);
        split = merge_chudnovsky(split, block);
        terms_done = end;
        if (terms_done < needed || limit.is_some())
            && save(path, &chud_file(&split, terms_done, None), limit)?
        {
            return Ok(Progress::Paused);
        }
    }

    let (value, root) = chudnovsky_value(one, scale, &split, saved_root)?;
    if save(
        path,
        &chud_file(&split, terms_done, Some((scale, root))),
        limit,
    )? {
        return Ok(Progress::Paused);
    }
    Ok(Progress::Finished(value))
}

fn chud_file(split: &ChudSplit, terms_done: u32, root: Option<(u32, BigUint)>) -> ChudFile {
    let (root_scale, root) = match root {
        Some((root_scale, root)) => (Some(root_scale), Some(root)),
        None => (None, None),
    };
    ChudFile {
        version: checkpoint::VERSION,
        tool: "pi".to_string(),
        algo: "chudnovsky".to_string(),
        digits: None,
        text: None,
        terms_done,
        root_scale,
        root,
        p: split.p.clone(),
        q: split.q.clone(),
        t: split.t.clone(),
    }
}

fn chudnovsky_value(
    one: &BigUint,
    scale: u32,
    split: &ChudSplit,
    saved_root: Option<(u32, BigUint)>,
) -> Result<(BigUint, BigUint), String> {
    let t = split
        .t
        .to_biguint()
        .ok_or("checkpoint Chudnovsky sum is negative")?;
    let n = BigUint::from(10005u32) * one * one;
    let sqrt_scaled = match saved_root {
        Some((root_scale, root)) => lift_root(&n, root, root_scale, scale),
        None => n.sqrt(),
    };
    let value = &split.q * 426_880u32 * &sqrt_scaled / t;
    Ok((value, sqrt_scaled))
}

/// Lift `floor(sqrt(10005 * 10^(2*root_scale)))` to `scale`.
fn lift_root(n: &BigUint, root: BigUint, root_scale: u32, scale: u32) -> BigUint {
    if root_scale == scale {
        return root;
    }
    if root_scale > scale {
        let shifted = root / BigUint::from(10u32).pow(root_scale - scale);
        return checkpoint::isqrt_from_seed(n, shifted, scale, scale);
    }
    let shifted = root * BigUint::from(10u32).pow(scale - root_scale);
    checkpoint::isqrt_from_seed(n, shifted, root_scale, scale)
}

/// Extend a finished series without parsing `p` or starting the square root over.
///
/// The saved digits already approximate pi, so one or two Newton steps finish
/// `sqrt(10005 * 10^(2*scale))` when no further Chudnovsky terms are required.
fn chudnovsky_from_saved(
    one: &BigUint,
    scale: u32,
    path: &Path,
    needed: u32,
) -> Result<Option<BigUint>, String> {
    let Some(saved) = checkpoint::read_summary(path)? else {
        return Ok(None);
    };
    checkpoint::require_header(
        path,
        saved.version,
        &saved.tool,
        &saved.algo,
        "pi",
        "chudnovsky",
    )?;
    if saved.terms_done.is_none_or(|done| done < needed) {
        return Ok(None);
    }
    let (Some(saved_digits), Some(text)) = (saved.digits, saved.text) else {
        return Ok(None);
    };
    let Some(fields) = checkpoint::load_decimals(path, &["q"], &["t"])? else {
        return Ok(None);
    };
    let uints = fields.uints;
    let ints = fields.ints;
    let t = ints
        .into_iter()
        .next()
        .and_then(|value| value.to_biguint())
        .ok_or("checkpoint Chudnovsky sum is negative")?;
    let q = uints
        .into_iter()
        .next()
        .ok_or("checkpoint Chudnovsky q is missing")?;
    Ok(Some(chudnovsky_value_seeded(
        one,
        &q,
        &t,
        &text,
        saved_digits,
        scale,
    )?))
}

fn chudnovsky_value_seeded(
    one: &BigUint,
    q: &BigUint,
    t: &BigUint,
    text: &str,
    saved_digits: u32,
    scale: u32,
) -> Result<BigUint, String> {
    let n = BigUint::from(10005u32) * one * one;
    let sqrt_scaled =
        sqrt_from_saved_pi(&n, q, t, text, saved_digits, scale).unwrap_or_else(|| n.sqrt());
    Ok(q * 426_880u32 * sqrt_scaled / t)
}

fn sqrt_from_saved_pi(
    n: &BigUint,
    q: &BigUint,
    t: &BigUint,
    text: &str,
    saved_digits: u32,
    scale: u32,
) -> Option<BigUint> {
    if saved_digits == 0 {
        return None;
    }
    let mut sig = Vec::with_capacity(text.len());
    for byte in text.bytes() {
        if byte == b'.' {
            continue;
        }
        if !byte.is_ascii_digit() {
            return None;
        }
        sig.push(byte);
    }
    if sig.len() != saved_digits as usize {
        return None;
    }
    let pi = checkpoint::parse_uint_decimal(std::str::from_utf8(&sig).ok()?).ok()?;
    let shift = scale.checked_sub(saved_digits - 1)?;
    let approx = pi * BigUint::from(10u32).pow(shift);
    let denom = q * 426_880u32;
    if denom == BigUint::from(0u32) {
        return None;
    }
    let seed = approx * t / denom;
    Some(checkpoint::isqrt_from_seed(n, seed, saved_digits, scale))
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct AtanSeriesFile {
    terms_done: u32,
    #[serde(with = "checkpoint::uint")]
    p: BigUint,
    #[serde(with = "checkpoint::uint")]
    q: BigUint,
    #[serde(with = "checkpoint::int")]
    t: BigInt,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct BinarySplitFile {
    version: u32,
    tool: String,
    algo: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    digits: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    text: Option<String>,
    atan5: AtanSeriesFile,
    atan239: AtanSeriesFile,
}

fn binary_split(
    one: &BigUint,
    scale: u32,
    path: &Path,
    limit: &mut Option<u32>,
) -> Result<Progress<BigUint>, String> {
    let mut state = match checkpoint::load_json::<BinarySplitFile>(path)? {
        None => BinarySplitFile {
            version: checkpoint::VERSION,
            tool: "pi".to_string(),
            algo: "binary-split".to_string(),
            digits: None,
            text: None,
            atan5: atan_identity(),
            atan239: atan_identity(),
        },
        Some(file) => {
            checkpoint::require_header(
                path,
                file.version,
                &file.tool,
                &file.algo,
                "pi",
                "binary-split",
            )?;
            file
        }
    };

    if advance_atan(true, 5, terms_needed(5, scale), path, &mut state, limit)? {
        return Ok(Progress::Paused);
    }
    if advance_atan(
        false,
        239,
        terms_needed(239, scale),
        path,
        &mut state,
        limit,
    )? {
        return Ok(Progress::Paused);
    }

    let atan5 = atan_value(5, one, &state.atan5)?;
    let atan239 = atan_value(239, one, &state.atan239)?;
    Ok(Progress::Finished((atan5 * 4u32 - atan239) * 4u32))
}

/// Extend one arctangent series. `leading` selects `atan(1/5)`.
/// `true` means the write budget was spent.
fn advance_atan(
    leading: bool,
    x: u32,
    needed: u32,
    path: &Path,
    file: &mut BinarySplitFile,
    limit: &mut Option<u32>,
) -> Result<bool, String> {
    let x_int = BigUint::from(x);
    let y = &x_int * &x_int;
    while series(file, leading).terms_done < needed {
        let start = series(file, leading).terms_done;
        let end = checkpoint::next_doubling(u64::from(start), u64::from(needed)) as u32;
        let block = split_arctan(start, end, &y);
        let merged = merge_arctan(series_split(series(file, leading)), block);
        let series = series_mut(file, leading);
        series.p = merged.p;
        series.q = merged.q;
        series.t = merged.t;
        series.terms_done = end;
        if save(path, file, limit)? {
            return Ok(true);
        }
    }
    Ok(false)
}

fn series(file: &BinarySplitFile, leading: bool) -> &AtanSeriesFile {
    if leading { &file.atan5 } else { &file.atan239 }
}

fn series_mut(file: &mut BinarySplitFile, leading: bool) -> &mut AtanSeriesFile {
    if leading {
        &mut file.atan5
    } else {
        &mut file.atan239
    }
}

fn atan_identity() -> AtanSeriesFile {
    AtanSeriesFile {
        terms_done: 0,
        p: BigUint::from(1u32),
        q: BigUint::from(1u32),
        t: BigInt::from(0),
    }
}

fn series_split(series: &AtanSeriesFile) -> AtanSplit {
    AtanSplit {
        p: series.p.clone(),
        q: series.q.clone(),
        t: series.t.clone(),
    }
}

fn atan_value(x: u32, one: &BigUint, series: &AtanSeriesFile) -> Result<BigUint, String> {
    let t = series
        .t
        .to_biguint()
        .ok_or("checkpoint arctangent sum is negative")?;
    let x_int = BigUint::from(x);
    let numerator = t * one * &x_int;
    let denominator = &series.q * &series.p;
    Ok(numerator / denominator)
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct TaylorFile {
    #[serde(with = "checkpoint::uint")]
    term: BigUint,
    #[serde(with = "checkpoint::uint")]
    total: BigUint,
    k: u32,
    subtract: bool,
    steps: u32,
    done: bool,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct MachinFile {
    version: u32,
    tool: String,
    algo: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    digits: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    text: Option<String>,
    scale: u32,
    atan5: TaylorFile,
    atan239: TaylorFile,
}

fn machin(
    one: &BigUint,
    scale: u32,
    path: &Path,
    limit: &mut Option<u32>,
) -> Result<Progress<BigUint>, String> {
    let mut state = match checkpoint::load_json::<MachinFile>(path)? {
        None => MachinFile {
            version: checkpoint::VERSION,
            tool: "pi".to_string(),
            algo: "machin".to_string(),
            digits: None,
            text: None,
            scale,
            atan5: taylor_start(5, one),
            atan239: taylor_start(239, one),
        },
        Some(file) => {
            checkpoint::require_header(path, file.version, &file.tool, &file.algo, "pi", "machin")?;
            if file.scale != scale {
                return Err(scale_locked(path, "machin", file.scale));
            }
            file
        }
    };

    if advance_taylor(true, 5, path, &mut state, limit)? {
        return Ok(Progress::Paused);
    }
    if advance_taylor(false, 239, path, &mut state, limit)? {
        return Ok(Progress::Paused);
    }

    let pi = ((&state.atan5.total * 4u32) - &state.atan239.total) * 4u32;
    Ok(Progress::Finished(pi))
}

fn taylor_start(x: u32, one: &BigUint) -> TaylorFile {
    let term = one / BigUint::from(x);
    TaylorFile {
        total: term.clone(),
        term,
        k: 3,
        subtract: true,
        steps: 0,
        done: false,
    }
}

fn advance_taylor(
    leading: bool,
    x: u32,
    path: &Path,
    file: &mut MachinFile,
    limit: &mut Option<u32>,
) -> Result<bool, String> {
    let x_squared = BigUint::from(x) * x;
    while !taylor(file, leading).done {
        let target = checkpoint::next_doubling(u64::from(taylor(file, leading).steps), u64::MAX);
        while u64::from(taylor(file, leading).steps) < target {
            let series = taylor_mut(file, leading);
            series.term /= &x_squared;
            if series.term.is_zero() {
                series.done = true;
                break;
            }
            let delta = &series.term / series.k;
            if delta.is_zero() {
                series.done = true;
                break;
            }
            if series.subtract {
                if series.total < delta {
                    return Err(
                        "checkpoint Machin total underflowed while subtracting a term".to_string(),
                    );
                }
                series.total -= &delta;
            } else {
                series.total += &delta;
            }
            series.subtract = !series.subtract;
            series.k = series.k.saturating_add(2);
            if series.steps == u32::MAX {
                series.done = true;
                break;
            }
            series.steps += 1;
        }
        if save(path, file, limit)? {
            return Ok(true);
        }
    }
    Ok(false)
}

fn taylor(file: &MachinFile, leading: bool) -> &TaylorFile {
    if leading { &file.atan5 } else { &file.atan239 }
}

fn taylor_mut(file: &mut MachinFile, leading: bool) -> &mut TaylorFile {
    if leading {
        &mut file.atan5
    } else {
        &mut file.atan239
    }
}

fn scale_locked(path: &Path, algo: &str, saved_scale: u32) -> String {
    let saved_digits = saved_scale + 1 - GUARD_DIGITS;
    format!(
        "{algo} checkpoint {} was saved at {saved_digits} digits; this algorithm cannot continue to a different digit count",
        path.display()
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{PiAlgo, format_pi_with_algo};

    fn scratch(label: &str) -> std::path::PathBuf {
        static N: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let n = N.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let dir =
            std::env::temp_dir().join(format!("consts-pi-{label}-{}-{n}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        dir.join("checkpoint.json")
    }

    fn pause(digits: u32, algo: PiAlgo, path: &Path, writes: u32) {
        match run(digits, algo, path, Some(writes)).unwrap() {
            Progress::Paused | Progress::Finished(_) => {}
        }
    }

    #[test]
    fn resumable_matches_a_fresh_run() {
        for algo in [PiAlgo::Chudnovsky, PiAlgo::BinarySplit, PiAlgo::Machin] {
            let path = scratch("fresh");
            let text = format_pi_resumable(25, algo, &path).unwrap();
            assert_eq!(text, format_pi_with_algo(25, algo).unwrap(), "{algo:?}");
        }
    }

    #[test]
    fn more_digits_with_enough_terms_match_a_fresh_run() {
        let path = scratch("same-terms");
        format_pi_resumable(40, PiAlgo::Chudnovsky, &path).unwrap();
        let mut value: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        value["p"] = serde_json::Value::from("0");
        std::fs::write(&path, serde_json::to_string_pretty(&value).unwrap()).unwrap();
        assert_eq!(
            format_pi_resumable(50, PiAlgo::Chudnovsky, &path).unwrap(),
            format_pi_with_algo(50, PiAlgo::Chudnovsky).unwrap()
        );
    }

    #[test]
    fn extend_and_shrink_match_a_fresh_run() {
        for algo in [PiAlgo::Chudnovsky, PiAlgo::BinarySplit] {
            let path = scratch("extend");
            format_pi_resumable(12, algo, &path).unwrap();
            let extended = format_pi_resumable(30, algo, &path).unwrap();
            assert_eq!(
                extended,
                format_pi_with_algo(30, algo).unwrap(),
                "{algo:?} extend"
            );
            let shrunk = format_pi_resumable(8, algo, &path).unwrap();
            assert_eq!(
                shrunk,
                format_pi_with_algo(8, algo).unwrap(),
                "{algo:?} shrink"
            );
        }
    }

    #[test]
    fn pause_then_resume_matches_a_fresh_run() {
        for algo in [PiAlgo::Chudnovsky, PiAlgo::BinarySplit, PiAlgo::Machin] {
            let path = scratch("pause");
            pause(20, algo, &path, 1);
            assert!(path.is_file(), "{algo:?} wrote no checkpoint");
            let text = format_pi_resumable(20, algo, &path).unwrap();
            assert_eq!(text, format_pi_with_algo(20, algo).unwrap(), "{algo:?}");
        }
    }

    #[test]
    fn machin_rejects_a_different_digit_count_without_rewriting() {
        let path = scratch("lock");
        format_pi_resumable(8, PiAlgo::Machin, &path).unwrap();
        let before = std::fs::read(&path).unwrap();
        for digits in [5u32, 12] {
            let err = format_pi_resumable(digits, PiAlgo::Machin, &path).unwrap_err();
            assert!(err.contains("saved at 8 digits"), "digits {digits}: {err}");
            assert_eq!(std::fs::read(&path).unwrap(), before);
        }
        let same = format_pi_resumable(8, PiAlgo::Machin, &path).unwrap();
        assert_eq!(same, format_pi_with_algo(8, PiAlgo::Machin).unwrap());
    }

    #[test]
    fn repeat_of_the_same_count_uses_the_saved_digits() {
        let path = scratch("replay");
        let text = format_pi_resumable(20, PiAlgo::Chudnovsky, &path).unwrap();
        let mut value: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        value["p"] = serde_json::Value::from("0");
        value["q"] = serde_json::Value::from("0");
        value["t"] = serde_json::Value::from("0");
        value["terms_done"] = serde_json::Value::from(1);
        std::fs::write(&path, serde_json::to_string_pretty(&value).unwrap()).unwrap();
        assert_eq!(
            format_pi_resumable(20, PiAlgo::Chudnovsky, &path).unwrap(),
            text
        );
        for digits in [1u32, 2, 5, 10, 15] {
            assert_eq!(
                format_pi_resumable(digits, PiAlgo::Chudnovsky, &path).unwrap(),
                format_pi_with_algo(digits, PiAlgo::Chudnovsky).unwrap(),
                "{digits} digits"
            );
        }
    }

    #[test]
    fn rejects_a_foreign_or_unknown_checkpoint() {
        let path = scratch("foreign");
        std::fs::write(
            &path,
            "{\n  \"version\": 1,\n  \"tool\": \"e\",\n  \"algo\": \"chudnovsky\",\n  \"terms_done\": 1,\n  \"p\": \"1\",\n  \"q\": \"1\",\n  \"t\": \"1\"\n}\n",
        )
        .unwrap();
        let before = std::fs::read(&path).unwrap();
        let err = format_pi_resumable(10, PiAlgo::Chudnovsky, &path).unwrap_err();
        assert!(err.contains("this run is pi chudnovsky"), "{err}");
        assert_eq!(std::fs::read(&path).unwrap(), before);

        std::fs::write(
            &path,
            "{\n  \"version\": 1,\n  \"tool\": \"pi\",\n  \"algo\": \"chudnovsky\",\n  \"terms_done\": 1,\n  \"p\": \"1\",\n  \"q\": \"1\",\n  \"t\": \"1\",\n  \"extra\": true\n}\n",
        )
        .unwrap();
        let before = std::fs::read(&path).unwrap();
        assert!(format_pi_resumable(10, PiAlgo::Chudnovsky, &path).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), before);
    }
}
