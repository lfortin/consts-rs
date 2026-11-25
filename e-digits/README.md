# e-digits

Command-line tool that prints Euler's number to a requested number of significant digits.

## Build

From the workspace root:

```bash
cargo build --release -p e-digits
```

The binary is `target/release/e`.

## Usage

```bash
e 50
e --algo binary-split 50
e --algo cf 50
e -a taylor 50
e --version
e --help
```

`DIGITS` is required. It is the number of significant digits, from 1 to 1000000000. A count of one prints `3`, because 2.718… rounds half up. Larger counts print a decimal point after the leading `2`. The last digit is rounded half up.

`--algo` (`-a`) selects how e is computed. `binary-split` (the default) and `taylor` both use the series e = Σ 1/n!. `binary-split` multiplies those factors in a tree and divides once. `taylor` divides by each integer in turn. `cf` uses the continued fraction e = [2; 1, 2, 1, 1, 4, 1, 1, 6, …]. All three print the same digits.

```text
e 1
3

e 5
2.7183

e 10
2.718281828
```

`--version` (`-V`) prints the version from `Cargo.toml` with a `v` prefix, for example `v0.1.0`. `--help` (`-h`) prints usage.

Requests above 1000000000 digits are rejected so the program does not run until it exhausts memory.

## License

LGPL-3.0-or-later. See [LICENSE](../LICENSE).
