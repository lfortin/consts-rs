# golden-digits

Command-line tool that prints the golden ratio to a requested number of significant digits.

The golden ratio is φ = (1 + √5) / 2.

## Build

From the workspace root:

```bash
cargo build --release -p golden-digits
```

The binary is `target/release/golden`.

## Usage

```bash
golden 50
golden --algo isqrt 50
golden --algo cf 50
golden -a newton 50
golden --version
golden --help
golden --resumable 50
```

`DIGITS` is required. It is the number of significant digits, from 1 to 1000000000. A count of one prints `2`, because 1.618… rounds half up. Larger counts print a decimal point after the leading `1`. The last digit is rounded half up.

`--algo` (`-a`) selects how the golden ratio is computed. `newton` (the default) and `isqrt` both evaluate (1 + √5) / 2. `newton` finds √5 with Newton-Raphson. `isqrt` uses integer square root. `cf` uses the continued fraction φ = [1; 1, 1, …], whose convergents are consecutive Fibonacci ratios. All three print the same digits.

```text
golden 1
2

golden 5
1.6180

golden 10
1.618033989
```

`--version` (`-V`) prints the version from `Cargo.toml` with a `v` prefix, for example `v0.1.0`. `--help` (`-h`) prints usage.

Requests above 1000000000 digits are rejected so the program does not run until it exhausts memory.

## Resume

`--resumable` writes the working integers to `golden-<algo>.json` in the current directory (`golden-newton.json`, `golden-isqrt.json`, or `golden-cf.json`) and continues from that file on a later run. Without the flag, the file is ignored.

The file is pretty-printed JSON. `version`, `tool`, and `algo` identify it. `scale` or `n` is a number. `root`, `p`, and `q` are hexadecimal strings with a `0x` prefix. `newton` and `isqrt` store `floor(√5 × 10^scale)`. `cf` stores the Fibonacci pair `q = F(n)`, `p = F(n+1)`.

The file is rewritten when the correct-digit count or the Fibonacci index doubles (1, 2, 4, 8, …), and once more when the accumulator for this request is finished, before the digits are printed. A crash or kill does not write. The next run keeps the last fully renamed file and repeats the work done since that write. A leftover `golden-<algo>.json.tmp` is ignored. The file stays after a successful print, so a later run can ask for more digits. Two processes must not share one checkpoint file.

## License

LGPL-3.0-or-later. See [LICENSE](../LICENSE).
