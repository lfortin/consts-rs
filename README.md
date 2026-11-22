# consts-rs

Workspace of command-line tools that print mathematical constants to a requested number of significant digits.

| Tool | Crate | Binary |
| --- | --- | --- |
| Pi | [`pi-digits`](pi-digits/README.md) | `pi` |
| Golden ratio | [`golden-digits`](golden-digits/README.md) | `golden` |
| Square root of 2 | [`sqrt2-digits`](sqrt2-digits/README.md) | `sqrt2` |
| Cube root of 2 | [`cbrt2-digits`](cbrt2-digits/README.md) | `cbrt2` |
| Euler's number | [`e-digits`](e-digits/README.md) | `e` |

## Build

Every crate uses `edition = "2024"`, which needs Rust 1.85 or newer.

```bash
cargo build --release
```

The binaries are `target/release/pi`, `target/release/golden`, `target/release/sqrt2`, `target/release/cbrt2`, and `target/release/e`.

Each command takes a required digit count from 1 to 1000000, plus `--version` (`-V`) and `--help` (`-h`). Every command also takes `--algo` (`-a`).

`pi` accepts `machin` (the default) and `binary-split`, both Machin's formula: term-by-term Taylor series, or binary splitting of that series. It also accepts `chudnovsky`, the Chudnovsky series, summed with binary splitting. `golden` accepts `newton` (the default) and `isqrt`, both (1 + √5) / 2 from a square root of 5 (Newton-Raphson, or integer square root). It also accepts `cf`, the continued fraction φ = [1; 1, 1, …]. `sqrt2` accepts `newton` (the default) or `cf`. `cbrt2` accepts `newton` (the default) or `halley`. `e` accepts `taylor` (the default) and `binary-split`, both the series e = Σ 1/n!: term-by-term division, or binary splitting of that series. It also accepts `cf`, the continued fraction e = [2; 1, 2, 1, 1, 4, …].

```bash
pi 50
golden 50
sqrt2 50
cbrt2 50
e 50
```

## Note

This project was created using agentic tools.

## License

LGPL-3.0-or-later. See [LICENSE](LICENSE).
