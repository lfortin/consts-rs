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
pi -a machin 50
pi --version
pi --help
```

`DIGITS` is required. It is the number of significant digits, from 1 to 1000000. A count of one prints `3`. Larger counts print a decimal point after the leading `3`. The last digit is rounded half up.

`--algo` (`-a`) selects the algorithm. `machin` (the default) uses Machin's formula with an alternating Taylor series. `binary-split` evaluates the same series with binary splitting. Both print the same digits.

```text
pi 1
3

pi 5
3.1416

pi 10
3.141592654
```

`--version` (`-V`) prints the version from `Cargo.toml` with a `v` prefix, for example `v0.1.0`. `--help` (`-h`) prints usage.

Requests above 1000000 digits are rejected so the program does not run until it exhausts memory.

## License

LGPL-3.0-or-later. See [LICENSE](../LICENSE).
