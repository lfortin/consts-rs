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

//! JSON checkpoints for Euler's number.
//!
//! The factorial split and the continued fraction are exact and extend to a
//! higher digit count. The Taylor series is stored at one scale.

use std::path::Path;

use checkpoint::Progress;
use num_bigint::BigUint;
use num_traits::Zero;
use serde::{Deserialize, Serialize};

use super::{
    CfMat, EAlgo, GUARD_DIGITS, apply_quotient, convergent_bits, format_scaled_e, partial_quotient,
    scale_for_digits, split_e_cf, split_factorial, term_count,
};

/// Format `digits` significant digits of e, reading and writing a JSON checkpoint at `path`.
///
/// `binary-split` and `cf` continue from the saved integers, including to a higher digit
/// count. `taylor` continues only for the same digit count.
pub fn format_e_resumable(digits: u32, algo: EAlgo, path: &Path) -> Result<String, String> {
    match run(digits, algo, path, None)? {
        Progress::Finished(text) => Ok(text),
        Progress::Paused => {
            Err("internal error: checkpoint paused without a write limit".to_string())
        }
    }
}

fn run(
    digits: u32,
    algo: EAlgo,
    path: &Path,
    mut limit: Option<u32>,
) -> Result<Progress<String>, String> {
    let scale = scale_for_digits(digits).map_err(str::to_string)?;
    let name = algo.to_string();
    if limit.is_none()
        && let Some(text) =
            checkpoint::cached_output(path, "e", &name, digits, !matches!(algo, EAlgo::Taylor))?
    {
        return Ok(Progress::Finished(text));
    }
    let one = BigUint::from(10u32).pow(scale);
    let scaled = match algo {
        EAlgo::Taylor => match taylor(&one, scale, path, &name, &mut limit)? {
            Progress::Paused => return Ok(Progress::Paused),
            Progress::Finished(value) => value,
        },
        EAlgo::BinarySplit => match factorial(&one, scale, path, &name, &mut limit)? {
            Progress::Paused => return Ok(Progress::Paused),
            Progress::Finished(value) => value,
        },
        EAlgo::Cf => match continued(&one, scale, path, &name, &mut limit)? {
            Progress::Paused => return Ok(Progress::Paused),
            Progress::Finished(value) => value,
        },
    };
    let text = format_scaled_e(scaled, digits).map_err(str::to_string)?;
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
struct FactorialFile {
    version: u32,
    tool: String,
    algo: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    digits: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    text: Option<String>,
    terms_done: u32,
    #[serde(with = "checkpoint::uint")]
    q: BigUint,
    #[serde(with = "checkpoint::uint")]
    t: BigUint,
}

fn factorial(
    one: &BigUint,
    scale: u32,
    path: &Path,
    algo: &str,
    limit: &mut Option<u32>,
) -> Result<Progress<BigUint>, String> {
    let needed = term_count(scale);
    let mut state = match checkpoint::load_json::<FactorialFile>(path)? {
        None => FactorialFile {
            version: checkpoint::VERSION,
            tool: "e".to_string(),
            algo: algo.to_string(),
            digits: None,
            text: None,
            terms_done: 0,
            q: BigUint::from(1u32),
            t: BigUint::from(0u32),
        },
        Some(file) => {
            checkpoint::require_header(path, file.version, &file.tool, &file.algo, "e", algo)?;
            file
        }
    };

    while state.terms_done < needed {
        let end = checkpoint::next_doubling(u64::from(state.terms_done), u64::from(needed)) as u32;
        let block = split_factorial(state.terms_done, end);
        state.t = state.t * &block.q + block.t;
        state.q *= block.q;
        state.terms_done = end;
        if save(path, &state, limit)? {
            return Ok(Progress::Paused);
        }
    }

    Ok(Progress::Finished((&state.t * one) / state.q))
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct TaylorFile {
    version: u32,
    tool: String,
    algo: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    digits: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    text: Option<String>,
    scale: u32,
    #[serde(with = "checkpoint::uint")]
    term: BigUint,
    #[serde(with = "checkpoint::uint")]
    total: BigUint,
    k: u32,
    steps: u32,
    done: bool,
}

fn taylor(
    one: &BigUint,
    scale: u32,
    path: &Path,
    algo: &str,
    limit: &mut Option<u32>,
) -> Result<Progress<BigUint>, String> {
    let mut state = match checkpoint::load_json::<TaylorFile>(path)? {
        None => TaylorFile {
            version: checkpoint::VERSION,
            tool: "e".to_string(),
            algo: algo.to_string(),
            digits: None,
            text: None,
            scale,
            term: one.clone(),
            total: one.clone(),
            k: 1,
            steps: 0,
            done: false,
        },
        Some(file) => {
            checkpoint::require_header(path, file.version, &file.tool, &file.algo, "e", algo)?;
            if file.scale != scale {
                return Err(scale_locked(path, algo, file.scale));
            }
            file
        }
    };

    while !state.done {
        let target = checkpoint::next_doubling(u64::from(state.steps), u64::MAX);
        while u64::from(state.steps) < target {
            if state.k == 0 {
                return Err("checkpoint Taylor index is zero".to_string());
            }
            state.term /= state.k;
            if state.term.is_zero() {
                state.done = true;
                break;
            }
            state.total += &state.term;
            state.k = state.k.saturating_add(1);
            if state.steps == u32::MAX {
                state.done = true;
                break;
            }
            state.steps += 1;
        }
        if save(path, &state, limit)? {
            return Ok(Progress::Paused);
        }
    }

    Ok(Progress::Finished(state.total))
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct CfFile {
    version: u32,
    tool: String,
    algo: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    digits: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    text: Option<String>,
    terms_done: u32,
    #[serde(with = "checkpoint::uint")]
    p: BigUint,
    #[serde(with = "checkpoint::uint")]
    r: BigUint,
    #[serde(with = "checkpoint::uint")]
    q: BigUint,
    #[serde(with = "checkpoint::uint")]
    s: BigUint,
}

fn continued(
    one: &BigUint,
    scale: u32,
    path: &Path,
    algo: &str,
    limit: &mut Option<u32>,
) -> Result<Progress<BigUint>, String> {
    let needed_bits = convergent_bits(scale);
    let mut terms_done;
    let mut mat;
    match checkpoint::load_json::<CfFile>(path)? {
        None => {
            terms_done = 0;
            mat = CfMat {
                p: BigUint::from(1u32),
                r: BigUint::from(0u32),
                q: BigUint::from(0u32),
                s: BigUint::from(1u32),
            };
        }
        Some(file) => {
            checkpoint::require_header(path, file.version, &file.tool, &file.algo, "e", algo)?;
            terms_done = file.terms_done;
            mat = CfMat {
                p: file.p,
                r: file.r,
                q: file.q,
                s: file.s,
            };
        }
    }

    loop {
        if terms_done > 0 && mat.q.bits() >= needed_bits {
            break;
        }
        if terms_done > 0 && u64::from(terms_done) > needed_bits {
            while mat.q.bits() < needed_bits {
                apply_quotient(&mut mat, partial_quotient(terms_done));
                terms_done = terms_done.saturating_add(1);
            }
            if save(path, &cf_file(algo, terms_done, &mat), limit)? {
                return Ok(Progress::Paused);
            }
            break;
        }
        let end = checkpoint::next_doubling(u64::from(terms_done), u64::MAX) as u32;
        let right = split_e_cf(terms_done, end);
        mat = mul_cf(mat, right);
        terms_done = end;
        if save(path, &cf_file(algo, terms_done, &mat), limit)? {
            return Ok(Progress::Paused);
        }
    }

    Ok(Progress::Finished((&mat.p * one) / mat.q))
}

fn cf_file(algo: &str, terms_done: u32, mat: &CfMat) -> CfFile {
    CfFile {
        version: checkpoint::VERSION,
        tool: "e".to_string(),
        algo: algo.to_string(),
        digits: None,
        text: None,
        terms_done,
        p: mat.p.clone(),
        r: mat.r.clone(),
        q: mat.q.clone(),
        s: mat.s.clone(),
    }
}

fn mul_cf(left: CfMat, right: CfMat) -> CfMat {
    CfMat {
        p: &left.p * &right.p + &left.r * &right.q,
        r: &left.p * &right.r + &left.r * &right.s,
        q: &left.q * &right.p + &left.s * &right.q,
        s: &left.q * &right.r + &left.s * &right.s,
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
    use crate::{EAlgo, format_e_with_algo};

    fn scratch(label: &str) -> std::path::PathBuf {
        static N: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let n = N.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("consts-e-{label}-{}-{n}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        dir.join("checkpoint.json")
    }

    fn pause(digits: u32, algo: EAlgo, path: &Path, writes: u32) {
        match run(digits, algo, path, Some(writes)).unwrap() {
            Progress::Paused | Progress::Finished(_) => {}
        }
    }

    #[test]
    fn resumable_matches_a_fresh_run() {
        for algo in [EAlgo::BinarySplit, EAlgo::Cf, EAlgo::Taylor] {
            let path = scratch("fresh");
            let text = format_e_resumable(25, algo, &path).unwrap();
            assert_eq!(text, format_e_with_algo(25, algo).unwrap(), "{algo:?}");
        }
    }

    #[test]
    fn extend_and_shrink_match_a_fresh_run() {
        for algo in [EAlgo::BinarySplit, EAlgo::Cf] {
            let path = scratch("extend");
            format_e_resumable(12, algo, &path).unwrap();
            let extended = format_e_resumable(30, algo, &path).unwrap();
            assert_eq!(extended, format_e_with_algo(30, algo).unwrap(), "{algo:?}");
            let shrunk = format_e_resumable(8, algo, &path).unwrap();
            assert_eq!(shrunk, format_e_with_algo(8, algo).unwrap(), "{algo:?}");
        }
    }

    #[test]
    fn pause_then_resume_matches_a_fresh_run() {
        for algo in [EAlgo::BinarySplit, EAlgo::Cf, EAlgo::Taylor] {
            let path = scratch("pause");
            pause(20, algo, &path, 1);
            assert!(path.is_file(), "{algo:?}");
            let text = format_e_resumable(20, algo, &path).unwrap();
            assert_eq!(text, format_e_with_algo(20, algo).unwrap(), "{algo:?}");
        }
    }

    #[test]
    fn taylor_rejects_a_different_digit_count_without_rewriting() {
        let path = scratch("lock");
        format_e_resumable(8, EAlgo::Taylor, &path).unwrap();
        let before = std::fs::read(&path).unwrap();
        for digits in [5u32, 12] {
            let err = format_e_resumable(digits, EAlgo::Taylor, &path).unwrap_err();
            assert!(err.contains("saved at 8 digits"), "digits {digits}: {err}");
            assert_eq!(std::fs::read(&path).unwrap(), before);
        }
    }
}
