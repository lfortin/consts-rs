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

//! Significant digits of Euler's number, from the series e = Σ 1/k!.

use num_bigint::BigUint;
use num_traits::Zero;

/// Largest digit count the CLI and library will compute.
pub const MAX_DIGITS: u32 = 100_000;

const GUARD_DIGITS: u32 = 20;

/// Format `digits` significant digits of e.
///
/// One digit is `3`, because 2.718… rounds half up. Larger counts include a
/// decimal point after the leading `2`. The last digit is rounded half up.
pub fn format_e(digits: u32) -> Result<String, &'static str> {
    if !(1..=MAX_DIGITS).contains(&digits) {
        return Err("digit count must be from 1 to 100000");
    }

    let scale = digits - 1 + GUARD_DIGITS;
    let mut term = BigUint::from(10u32).pow(scale);
    let mut total = term.clone();
    let mut k = 1u32;
    loop {
        term /= k;
        if term.is_zero() {
            break;
        }
        total += &term;
        k += 1;
    }

    let guard = BigUint::from(10u32).pow(GUARD_DIGITS);
    let half = &guard / 2u32;
    let mut kept = &total / &guard;
    if &total % &guard >= half {
        kept += 1u32;
    }

    let text = kept.to_string();
    if text.len() != digits as usize {
        return Err("internal error: Euler's number rounding produced an unexpected length");
    }

    if digits == 1 {
        Ok(text)
    } else {
        Ok(format!("{}.{}", &text[..1], &text[1..]))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_prefixes() {
        assert_eq!(format_e(1).unwrap(), "3");
        assert_eq!(format_e(2).unwrap(), "2.7");
        assert_eq!(format_e(5).unwrap(), "2.7183");
        assert_eq!(format_e(10).unwrap(), "2.718281828");
        assert_eq!(format_e(20).unwrap(), "2.7182818284590452354");
        assert_eq!(
            format_e(50).unwrap(),
            "2.7182818284590452353602874713526624977572470937000"
        );
    }

    #[test]
    fn rejects_out_of_range() {
        assert!(format_e(0).is_err());
        assert!(format_e(MAX_DIGITS + 1).is_err());
    }
}
