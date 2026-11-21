# pi-digits

Command-line tool that prints pi to a requested number of significant digits.

## Build

```bash
cargo build --release
```

The binary is `target/release/pi`.

## Usage

```bash
pi 50
pi --version
pi --help
```

`DIGITS` is required. It is the number of significant digits, from 1 to 100000. A count of one prints `3`. Larger counts print a decimal point after the leading `3`. The last digit is rounded half up.

```text
pi 1
3

pi 5
3.1416

pi 10
3.141592654
```

`--version` (`-V`) prints the version from `Cargo.toml`. `--help` (`-h`) prints usage.

Requests above 100000 digits are rejected so the program does not run until it exhausts memory.

## License

LGPL-3.0-or-later. See [COPYING.LESSER](COPYING.LESSER) and [COPYING](COPYING).
