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

//! JSON checkpoints for the square root of 3.
//!
//! Newton stores `floor(√3 × 10^scale)`. The continued fraction stores
//! `(2 + √3)^k = p + q√3`. Both extend to a higher digit count.

use std::path::Path;

use checkpoint::Progress;
use num_bigint::BigUint;
use serde::{Deserialize, Serialize};

use super::{Sqrt3Algo, convergent_bits, format_scaled_sqrt3, pell_pow, scale_for_digits};

/// Format `digits` significant digits of √3, reading and writing a JSON checkpoint at `path`.
pub fn format_sqrt3_resumable(digits: u32, algo: Sqrt3Algo, path: &Path) -> Result<String, String> {
    match run(digits, algo, path, None)? {
        Progress::Finished(text) => Ok(text),
        Progress::Paused => {
            Err("internal error: checkpoint paused without a write limit".to_string())
        }
    }
}

fn run(
    digits: u32,
    algo: Sqrt3Algo,
    path: &Path,
    mut limit: Option<u32>,
) -> Result<Progress<String>, String> {
    let scale = scale_for_digits(digits).map_err(str::to_string)?;
    let name = algo.to_string();
    if limit.is_none()
        && let Some(text) = checkpoint::cached_output(path, "sqrt3", &name, digits, true)?
    {
        return Ok(Progress::Finished(text));
    }
    let one = BigUint::from(10u32).pow(scale);
    let (scaled, persist) = match algo {
        Sqrt3Algo::Newton => match sqrt_root(scale, path, &name, &mut limit)? {
            Progress::Paused => return Ok(Progress::Paused),
            Progress::Finished(saved) => (saved.root, saved.persist),
        },
        Sqrt3Algo::Cf => match sqrt3_cf(&one, scale, path, &name, &mut limit)? {
            Progress::Paused => return Ok(Progress::Paused),
            Progress::Finished(value) => (value, false),
        },
    };
    let (text, seed) = if persist {
        checkpoint::display_and_seed(&scaled, digits, super::GUARD_DIGITS)?
    } else {
        (
            format_scaled_sqrt3(scaled, digits).map_err(str::to_string)?,
            String::new(),
        )
    };
    if limit.is_none() {
        if persist {
            let file = RootFile {
                version: checkpoint::VERSION,
                tool: "sqrt3".to_string(),
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
    scale: u32,
    path: &Path,
    algo: &str,
    limit: &mut Option<u32>,
) -> Result<Progress<SavedRoot>, String> {
    if limit.is_none()
        && let Some(saved) = checkpoint::read_summary(path)?
    {
        checkpoint::require_header(path, saved.version, &saved.tool, &saved.algo, "sqrt3", algo)?;
        let requested = scale + 1 - super::GUARD_DIGITS;
        if let Some(seed) = saved.seed
            && let Some(root) = lift_exact(&seed, scale)
        {
            return Ok(Progress::Finished(SavedRoot {
                root,
                persist: true,
            }));
        }
        if let (Some(saved_digits), Some(text)) = (saved.digits, saved.text)
            && (1..requested).contains(&saved_digits)
            && let Some(root) = lift_printed(&text, saved_digits, scale)
        {
            return Ok(Progress::Finished(SavedRoot {
                root,
                persist: true,
            }));
        }
    }
    let (mut scale_now, mut root) = match checkpoint::load_json::<RootFile>(path)? {
        None => (0, BigUint::from(1u32)),
        Some(file) => {
            checkpoint::require_header(path, file.version, &file.tool, &file.algo, "sqrt3", algo)?;
            match file.root {
                Some(root) => (file.scale, root),
                None => (0, BigUint::from(1u32)),
            }
        }
    };

    if scale_now > scale {
        let seed = root / BigUint::from(10u32).pow(scale_now - scale);
        let root = checkpoint::isqrt_from_seed(&square_radicand(scale), seed, scale, scale);
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
        let rad = square_radicand(next);
        let seed = root * BigUint::from(10u32).pow(next - scale_now);
        root = checkpoint::isqrt_from_seed(&rad, seed, correct, next);
        scale_now = next;
        if scale_now < scale || limit.is_some() {
            let file = RootFile {
                version: checkpoint::VERSION,
                tool: "sqrt3".to_string(),
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

fn lift_exact(seed: &str, scale: u32) -> Option<BigUint> {
    if seed.is_empty() || seed.bytes().any(|byte| !byte.is_ascii_digit()) {
        return None;
    }
    let correct = u32::try_from(seed.len()).ok()?.saturating_sub(1).max(1);
    let value = checkpoint::parse_uint_decimal(seed).ok()?;
    let shift = scale.checked_sub(correct)?;
    let shifted = value * BigUint::from(10u32).pow(shift);
    Some(checkpoint::isqrt_from_seed(
        &square_radicand(scale),
        shifted,
        correct,
        scale,
    ))
}

fn lift_printed(text: &str, saved_digits: u32, scale: u32) -> Option<BigUint> {
    let value = checkpoint::significant_integer(text, saved_digits)?;
    let shift = scale.checked_sub(saved_digits - 1)?;
    let seed = value * BigUint::from(10u32).pow(shift);
    let correct = saved_digits.saturating_sub(1).max(1);
    Some(checkpoint::isqrt_from_seed(
        &square_radicand(scale),
        seed,
        correct,
        scale,
    ))
}

fn square_radicand(scale: u32) -> BigUint {
    BigUint::from(3u32) * BigUint::from(10u32).pow(scale.saturating_mul(2))
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

fn sqrt3_cf(
    one: &BigUint,
    scale: u32,
    path: &Path,
    algo: &str,
    limit: &mut Option<u32>,
) -> Result<Progress<BigUint>, String> {
    let needed_bits = convergent_bits(scale);
    let k_target = ((needed_bits + 2) * 5264 / 10_000 + 1).max(1);
    let mut state = match checkpoint::load_json::<PellFile>(path)? {
        None => PellFile {
            version: checkpoint::VERSION,
            tool: "sqrt3".to_string(),
            algo: algo.to_string(),
            digits: None,
            text: None,
            k: 0,
            p: BigUint::from(1u32),
            q: BigUint::from(0u32),
        },
        Some(file) => {
            checkpoint::require_header(path, file.version, &file.tool, &file.algo, "sqrt3", algo)?;
            file
        }
    };

    loop {
        if state.k >= k_target && state.q.bits() >= needed_bits {
            break;
        }
        if state.k >= k_target {
            while state.q.bits() < needed_bits {
                let p_next = (&state.p << 1) + (&state.q << 1) + &state.q;
                let q_next = &state.p + (&state.q << 1);
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
        // (p + q√3)(p2 + q2√3) = (p p2 + 3 q q2) + (p q2 + q p2)√3.
        let p_next = &state.p * &p2 + &state.q * &q2 * 3u32;
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
    use crate::{Sqrt3Algo, format_sqrt3_with_algo};

    fn scratch(label: &str) -> std::path::PathBuf {
        static N: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let n = N.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let dir =
            std::env::temp_dir().join(format!("consts-sqrt3-{label}-{}-{n}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        dir.join("checkpoint.json")
    }

    fn pause(digits: u32, algo: Sqrt3Algo, path: &Path, writes: u32) {
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
                let rad = BigUint::from(3u32) * BigUint::from(10u32).pow(next * 2);
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
        for algo in [Sqrt3Algo::Newton, Sqrt3Algo::Cf] {
            let path = scratch("fresh");
            let text = format_sqrt3_resumable(25, algo, &path).unwrap();
            assert_eq!(text, format_sqrt3_with_algo(25, algo).unwrap(), "{algo:?}");
        }
    }

    #[test]
    fn extend_and_shrink_match_a_fresh_run() {
        for algo in [Sqrt3Algo::Newton, Sqrt3Algo::Cf] {
            let path = scratch("extend");
            format_sqrt3_resumable(12, algo, &path).unwrap();
            let extended = format_sqrt3_resumable(30, algo, &path).unwrap();
            assert_eq!(extended, format_sqrt3_with_algo(30, algo).unwrap());
            let shrunk = format_sqrt3_resumable(8, algo, &path).unwrap();
            assert_eq!(shrunk, format_sqrt3_with_algo(8, algo).unwrap());
        }
    }

    #[test]
    fn pause_then_resume_matches_a_fresh_run() {
        for algo in [Sqrt3Algo::Newton, Sqrt3Algo::Cf] {
            let path = scratch("pause");
            pause(20, algo, &path, 1);
            assert!(path.is_file());
            let text = format_sqrt3_resumable(20, algo, &path).unwrap();
            assert_eq!(text, format_sqrt3_with_algo(20, algo).unwrap());
        }
    }
}
