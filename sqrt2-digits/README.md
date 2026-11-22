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
```

`DIGITS` is required. It is the number of significant digits, from 1 to 100000. A count of one prints `1`. Larger counts print a decimal point after the leading `1`. The last digit is rounded half up.

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

Requests above 100000 digits are rejected so the program does not run until it exhausts memory.

## License

LGPL-3.0-or-later. See [LICENSE](../LICENSE).
