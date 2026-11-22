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
golden -a newton 50
golden --version
golden --help
```

`DIGITS` is required. It is the number of significant digits, from 1 to 1000000. A count of one prints `2`, because 1.618… rounds half up. Larger counts print a decimal point after the leading `1`. The last digit is rounded half up.

`--algo` (`-a`) selects the algorithm. `newton` (the default) takes the square root of 5 by Newton-Raphson. `isqrt` uses integer square root. Both then compute (1 + √5) / 2 and print the same digits.

```text
golden 1
2

golden 5
1.6180

golden 10
1.618033989
```

`--version` (`-V`) prints the version from `Cargo.toml` with a `v` prefix, for example `v0.1.0`. `--help` (`-h`) prints usage.

Requests above 1000000 digits are rejected so the program does not run until it exhausts memory.

## License

LGPL-3.0-or-later. See [LICENSE](../LICENSE).
