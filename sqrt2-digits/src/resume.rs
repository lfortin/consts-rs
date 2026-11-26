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

//! JSON checkpoints for the square root of 2.
//!
//! Newton stores `floor(√2 × 10^scale)`. The continued fraction stores
//! `(1 + √2)^k = p + q√2`. Both extend to a higher digit count.

use std::path::Path;

use checkpoint::Progress;
use num_bigint::BigUint;
use serde::{Deserialize, Serialize};

use super::{Sqrt2Algo, convergent_bits, format_scaled_sqrt2, pell_pow, scale_for_digits};

/// Format `digits` significant digits of √2, reading and writing a JSON checkpoint at `path`.
pub fn format_sqrt2_resumable(digits: u32, algo: Sqrt2Algo, path: &Path) -> Result<String, String> {
    match run(digits, algo, path, None)? {
        Progress::Finished(text) => Ok(text),
        Progress::Paused => {
            Err("internal error: checkpoint paused without a write limit".to_string())
        }
    }
}

fn run(
    digits: u32,
    algo: Sqrt2Algo,
    path: &Path,
    mut limit: Option<u32>,
) -> Result<Progress<String>, String> {
    let scale = scale_for_digits(digits).map_err(str::to_string)?;
    let name = algo.to_string();
    if limit.is_none()
        && let Some(text) = checkpoint::cached_output(path, "sqrt2", &name, digits, true)?
    {
        return Ok(Progress::Finished(text));
    }
    let one = BigUint::from(10u32).pow(scale);
    let (scaled, persist) = match algo {
        Sqrt2Algo::Newton => match sqrt_root(2, scale, path, "sqrt2", &name, &mut limit)? {
            Progress::Paused => return Ok(Progress::Paused),
            Progress::Finished(saved) => (saved.root, saved.persist),
        },
        Sqrt2Algo::Cf => match sqrt2_cf(&one, scale, path, &name, &mut limit)? {
            Progress::Paused => return Ok(Progress::Paused),
            Progress::Finished(value) => (value, false),
        },
    };
    let (text, seed) = if persist {
        checkpoint::display_and_seed(&scaled, digits, super::GUARD_DIGITS)?
    } else {
        (
            format_scaled_sqrt2(scaled, digits).map_err(str::to_string)?,
            String::new(),
        )
    };
    if limit.is_none() {
        if persist {
            let file = RootFile {
                version: checkpoint::VERSION,
                tool: "sqrt2".to_string(),
                algo: name,
                digits: Some(digits),
                text: Some(text.clone()),
                seed: Some(seed),
                scale,
                root: None,
            };
            checkpoint::store_json(path, &file)?;
        } else {
            checkpoint::store_output(path, digits, &text)?;
        }
    }
    Ok(Progress::Finished(text))
}

fn save<T: Serialize>(path: &Path, value: &T, limit: &mut Option<u32>) -> Result<bool, String> {
    checkpoint::store_and_count(path, value, limit)
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RootFile {
    version: u32,
    tool: String,
    algo: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    digits: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    text: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    seed: Option<String>,
    scale: u32,
    #[serde(default, skip_serializing_if = "Option::is_none", with = "checkpoint::uint::opt")]
    root: Option<BigUint>,
}

struct SavedRoot {
    root: BigUint,
    /// `root` is ahead of the file. The caller stores it once, with the digits.
    persist: bool,
}

fn sqrt_root(
    coeff: u32,
    scale: u32,
    path: &Path,
    tool: &str,
    algo: &str,
    limit: &mut Option<u32>,
) -> Result<Progress<SavedRoot>, String> {
    if limit.is_none()
        && let Some(saved) = checkpoint::read_summary(path)?
    {
        checkpoint::require_header(path, saved.version, &saved.tool, &saved.algo, tool, algo)?;
        let requested = scale + 1 - super::GUARD_DIGITS;
        if let Some(seed) = saved.seed
            && let Some(root) = lift_exact(coeff, &seed, scale)
        {
            return Ok(Progress::Finished(SavedRoot {
                root,
                persist: true,
            }));
        }
        if let (Some(saved_digits), Some(text)) = (saved.digits, saved.text)
            && (1..requested).contains(&saved_digits)
            && let Some(root) = lift_printed(coeff, &text, saved_digits, scale)
        {
            return Ok(Progress::Finished(SavedRoot {
                root,
                persist: true,
            }));
        }
    }
    let (mut scale_now, mut root) = match checkpoint::load_json::<RootFile>(path)? {
        None => (0, small_isqrt(coeff)),
        Some(file) => {
            checkpoint::require_header(path, file.version, &file.tool, &file.algo, tool, algo)?;
            match file.root {
                Some(root) => (file.scale, root),
                None => (0, small_isqrt(coeff)),
            }
        }
    };

    if scale_now > scale {
        let seed = root / BigUint::from(10u32).pow(scale_now - scale);
        let root = checkpoint::isqrt_from_seed(&square_radicand(coeff, scale), seed, scale, scale);
        return Ok(Progress::Finished(SavedRoot {
            root,
            persist: false,
        }));
    }
    if scale_now == scale && scale > 0 {
        return Ok(Progress::Finished(SavedRoot {
            root,
            persist: false,
        }));
    }

    while scale_now < scale {
        let next = if scale_now == 0 {
            1.min(scale)
        } else {
            scale_now.saturating_mul(2).min(scale)
        };
        if next <= scale_now {
            break;
        }
        let correct = scale_now.max(1);
        let rad = square_radicand(coeff, next);
        let seed = root * BigUint::from(10u32).pow(next - scale_now);
        root = checkpoint::isqrt_from_seed(&rad, seed, correct, next);
        scale_now = next;
        // The finished root is stored once, together with the digits.
        if scale_now < scale || limit.is_some() {
            let file = RootFile {
                version: checkpoint::VERSION,
                tool: tool.to_string(),
                algo: algo.to_string(),
                digits: None,
                text: None,
                seed: None,
                scale: scale_now,
                root: Some(root.clone()),
            };
            if save(path, &file, limit)? {
                return Ok(Progress::Paused);
            }
        }
    }
    Ok(Progress::Finished(SavedRoot {
        root,
        persist: limit.is_none(),
    }))
}

fn lift_exact(coeff: u32, seed: &str, scale: u32) -> Option<BigUint> {
    if seed.is_empty() || seed.bytes().any(|byte| !byte.is_ascii_digit()) {
        return None;
    }
    let correct = u32::try_from(seed.len()).ok()?.saturating_sub(1).max(1);
    let value = checkpoint::parse_uint_decimal(seed).ok()?;
    let shift = scale.checked_sub(correct)?;
    let shifted = value * BigUint::from(10u32).pow(shift);
    Some(checkpoint::isqrt_from_seed(
        &square_radicand(coeff, scale),
        shifted,
        correct,
        scale,
    ))
}

fn lift_printed(coeff: u32, text: &str, saved_digits: u32, scale: u32) -> Option<BigUint> {
    let value = checkpoint::significant_integer(text, saved_digits)?;
    let shift = scale.checked_sub(saved_digits - 1)?;
    let seed = value * BigUint::from(10u32).pow(shift);
    let correct = saved_digits.saturating_sub(1).max(1);
    Some(checkpoint::isqrt_from_seed(
        &square_radicand(coeff, scale),
        seed,
        correct,
        scale,
    ))
}

fn square_radicand(coeff: u32, scale: u32) -> BigUint {
    BigUint::from(coeff) * BigUint::from(10u32).pow(scale.saturating_mul(2))
}

fn small_isqrt(n: u32) -> BigUint {
    let mut x = 0u32;
    while x < u32::MAX && (x + 1).saturating_mul(x + 1) <= n {
        x += 1;
    }
    BigUint::from(x)
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct PellFile {
    version: u32,
    tool: String,
    algo: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    digits: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    text: Option<String>,
    k: u64,
    #[serde(with = "checkpoint::uint")]
    p: BigUint,
    #[serde(with = "checkpoint::uint")]
    q: BigUint,
}

fn sqrt2_cf(
    one: &BigUint,
    scale: u32,
    path: &Path,
    algo: &str,
    limit: &mut Option<u32>,
) -> Result<Progress<BigUint>, String> {
    let needed_bits = convergent_bits(scale);
    let k_target = ((needed_bits + 2) * 7865 / 10_000 + 1).max(1);
    let mut state = match checkpoint::load_json::<PellFile>(path)? {
        None => PellFile {
            version: checkpoint::VERSION,
            tool: "sqrt2".to_string(),
            algo: algo.to_string(),
            digits: None,
            text: None,
            k: 0,
            p: BigUint::from(1u32),
            q: BigUint::from(0u32),
        },
        Some(file) => {
            checkpoint::require_header(path, file.version, &file.tool, &file.algo, "sqrt2", algo)?;
            file
        }
    };

    loop {
        if state.k >= k_target && state.q.bits() >= needed_bits {
            break;
        }
        if state.k >= k_target {
            while state.q.bits() < needed_bits {
                let p_next = &state.p + (&state.q << 1);
                let q_next = &state.p + &state.q;
                state.p = p_next;
                state.q = q_next;
                state.k += 1;
            }
            if save(path, &state, limit)? {
                return Ok(Progress::Paused);
            }
            break;
        }
        let next = checkpoint::next_doubling(state.k, k_target);
        let (p2, q2) = pell_pow(next - state.k);
        let p_next = &state.p * &p2 + ((&state.q * &q2) << 1);
        let q_next = &state.p * &q2 + &state.q * &p2;
        state.p = p_next;
        state.q = q_next;
        state.k = next;
        if save(path, &state, limit)? {
            return Ok(Progress::Paused);
        }
    }

    Ok(Progress::Finished((&state.p * one) / &state.q))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Sqrt2Algo, format_sqrt2_with_algo};

    fn scratch(label: &str) -> std::path::PathBuf {
        static N: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let n = N.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let dir =
            std::env::temp_dir().join(format!("consts-sqrt2-{label}-{}-{n}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        dir.join("checkpoint.json")
    }

    fn pause(digits: u32, algo: Sqrt2Algo, path: &Path, writes: u32) {
        match run(digits, algo, path, Some(writes)).unwrap() {
            Progress::Paused | Progress::Finished(_) => {}
        }
    }

    #[test]
    fn lifted_roots_match_integer_sqrt() {
        for scale in [1u32, 2, 5, 20, 40, 50] {
            let mut x = BigUint::from(1u32);
            let mut s = 0u32;
            while s < scale {
                let next = if s == 0 { 1 } else { (s * 2).min(scale) };
                let rad = BigUint::from(2u32) * BigUint::from(10u32).pow(next * 2);
                x = checkpoint::isqrt_from_seed(
                    &rad,
                    x * BigUint::from(10u32).pow(next - s),
                    s.max(1),
                    next,
                );
                s = next;
                assert_eq!(x, rad.sqrt(), "scale {s}");
            }
        }
    }

    #[test]
    fn resumable_matches_a_fresh_run() {
        for algo in [Sqrt2Algo::Newton, Sqrt2Algo::Cf] {
            let path = scratch("fresh");
            let text = format_sqrt2_resumable(25, algo, &path).unwrap();
            assert_eq!(text, format_sqrt2_with_algo(25, algo).unwrap(), "{algo:?}");
        }
    }

    #[test]
    fn extend_and_shrink_match_a_fresh_run() {
        for algo in [Sqrt2Algo::Newton, Sqrt2Algo::Cf] {
            let path = scratch("extend");
            format_sqrt2_resumable(12, algo, &path).unwrap();
            let extended = format_sqrt2_resumable(30, algo, &path).unwrap();
            assert_eq!(extended, format_sqrt2_with_algo(30, algo).unwrap());
            let shrunk = format_sqrt2_resumable(8, algo, &path).unwrap();
            assert_eq!(shrunk, format_sqrt2_with_algo(8, algo).unwrap());
        }
    }

    #[test]
    fn pause_then_resume_matches_a_fresh_run() {
        for algo in [Sqrt2Algo::Newton, Sqrt2Algo::Cf] {
            let path = scratch("pause");
            pause(20, algo, &path, 1);
            assert!(path.is_file());
            let text = format_sqrt2_resumable(20, algo, &path).unwrap();
            assert_eq!(text, format_sqrt2_with_algo(20, algo).unwrap());
        }
    }
}
