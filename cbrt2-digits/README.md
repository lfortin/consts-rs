# cbrt2-digits

Command-line tool that prints the cube root of 2 to a requested number of significant digits.

## Build

From the workspace root:

```bash
cargo build --release -p cbrt2-digits
```

The binary is `target/release/cbrt2`.

## Usage

```bash
cbrt2 50
cbrt2 --algo halley 50
cbrt2 -a newton 50
cbrt2 --version
cbrt2 --help
```

`DIGITS` is required. It is the number of significant digits, from 1 to 1000000. A count of one prints `1`. Two digits print `1.3`, because 1.259… rounds half up. Larger counts print a decimal point after the leading `1`. The last digit is rounded half up.

`--algo` (`-a`) selects the algorithm. `newton` (the default) uses Newton-Raphson. `halley` uses Halley's method. Both print the same digits.

```text
cbrt2 1
1

cbrt2 5
1.2599

cbrt2 10
1.259921050
```

`--version` (`-V`) prints the version from `Cargo.toml` with a `v` prefix, for example `v0.1.0`. `--help` (`-h`) prints usage.

Requests above 1000000 digits are rejected so the program does not run until it exhausts memory.

## License

LGPL-3.0-or-later. See [LICENSE](../LICENSE).
