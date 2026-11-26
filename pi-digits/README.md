# pi-digits

Command-line tool that prints pi to a requested number of significant digits.

## Build

From the workspace root:

```bash
cargo build --release -p pi-digits
```

The binary is `target/release/pi`.

## Usage

```bash
pi 50
pi --algo binary-split 50
pi --algo chudnovsky 50
pi -a machin 50
pi --version
pi --help
pi --resumable 50
```

`DIGITS` is required. It is the number of significant digits, from 1 to 1000000000. A count of one prints `3`. Larger counts print a decimal point after the leading `3`. The last digit is rounded half up.

`--algo` (`-a`) selects how pi is computed. `chudnovsky` (the default) uses the Chudnovsky series for 1/π, summed by binary splitting. `machin` and `binary-split` both use Machin's formula, π/4 = 4·arctan(1/5) − arctan(1/239). `machin` sums each arctangent Taylor series term by term. `binary-split` sums that same series with binary splitting. All three print the same digits.

```text
pi 1
3

pi 5
3.1416

pi 10
3.141592654
```

`--version` (`-V`) prints the version from `Cargo.toml` with a `v` prefix, for example `v0.1.0`. `--help` (`-h`) prints usage.

Requests above 1000000000 digits are rejected so the program does not run until it exhausts memory.

## Resume

`--resumable` writes the working integers to `pi-<algo>.json` in the current directory (`pi-chudnovsky.json`, `pi-binary-split.json`, or `pi-machin.json`) and continues from that file on a later run. Without the flag, the file is ignored.

The file is pretty-printed JSON. `version`, `tool`, and `algo` identify it. Counters such as `terms_done`, `scale`, `k`, and `steps` are numbers. Every big integer is a hexadecimal string with a `0x` prefix, and a leading `-` when it is negative.

The file is rewritten when the completed term count or Taylor iteration count doubles (1, 2, 4, 8, …), and once more when the accumulator for this request is finished, before the digits are printed. A crash or kill does not write. The next run keeps the last fully renamed file and repeats the work done since that write. A leftover `pi-<algo>.json.tmp` is ignored. The file stays after a successful print. It also stores the finished digits. Repeating that count, or any smaller count, prints from those saved digits and does not recompute them. A later run can still ask for more digits. `machin` still refuses a different digit count.

`chudnovsky` and `binary-split` can continue to a higher digit count. `machin` resumes only for the same digit count; a different count is an error and leaves the file unchanged. Two processes must not share one checkpoint file.

## License

LGPL-3.0-or-later. See [LICENSE](../LICENSE).
