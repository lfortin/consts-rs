# sqrt2-digits

Command-line tool that prints the square root of 2 to a requested number of significant digits.

## Build

From the workspace root:

```bash
cargo build --release -p sqrt2-digits
```

The binary is `target/release/sqrt2`.

## Usage

```bash
sqrt2 50
sqrt2 --algo cf 50
sqrt2 -a newton 50
sqrt2 --version
sqrt2 --help
sqrt2 --resumable 50
```

`DIGITS` is required. It is the number of significant digits, from 1 to 1000000000. A count of one prints `1`. Larger counts print a decimal point after the leading `1`. The last digit is rounded half up.

`--algo` (`-a`) selects the algorithm. `newton` (the default) uses Heron's method. `cf` uses the continued fraction √2 = [1; 2, 2, 2, …]. Both print the same digits.

```text
sqrt2 1
1

sqrt2 5
1.4142

sqrt2 10
1.414213562
```

`--version` (`-V`) prints the version from `Cargo.toml` with a `v` prefix, for example `v0.1.0`. `--help` (`-h`) prints usage.

Requests above 1000000000 digits are rejected so the program does not run until it exhausts memory.

## Resume

`--resumable` writes the working integers to `sqrt2-<algo>.json` in the current directory (`sqrt2-newton.json` or `sqrt2-cf.json`) and continues from that file on a later run. Without the flag, the file is ignored.

The file is pretty-printed JSON. `version`, `tool`, and `algo` identify it. `scale` or `k` is a number. `root`, `p`, and `q` are hexadecimal strings with a `0x` prefix. `newton` stores `floor(√2 × 10^scale)`. `cf` stores `(1 + √2)^k = p + q√2`.

The file is rewritten when the correct-digit count or the convergent index doubles (1, 2, 4, 8, …), and once more when the accumulator for this request is finished, before the digits are printed. A crash or kill does not write. The next run keeps the last fully renamed file and repeats the work done since that write. A leftover `sqrt2-<algo>.json.tmp` is ignored. The file stays after a successful print, so a later run can ask for more digits. Two processes must not share one checkpoint file.

## License

LGPL-3.0-or-later. See [LICENSE](../LICENSE).
