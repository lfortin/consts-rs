# consts-rs

Workspace of command-line tools that print mathematical constants to a requested number of significant digits.

| Tool | Crate | Binary |
| --- | --- | --- |
| Pi | [`pi-digits`](pi-digits/README.md) | `pi` |
| Golden ratio | [`golden-digits`](golden-digits/README.md) | `golden` |
| Square root of 2 | [`sqrt2-digits`](sqrt2-digits/README.md) | `sqrt2` |
| Square root of 3 | [`sqrt3-digits`](sqrt3-digits/README.md) | `sqrt3` |
| Cube root of 2 | [`cbrt2-digits`](cbrt2-digits/README.md) | `cbrt2` |
| Euler's number | [`e-digits`](e-digits/README.md) | `e` |

## Build

Every crate uses `edition = "2024"`, which needs Rust 1.85 or newer.

```bash
cargo build --release
```

The binaries are `target/release/pi`, `target/release/golden`, `target/release/sqrt2`, `target/release/sqrt3`, `target/release/cbrt2`, and `target/release/e`.

Each command takes a required digit count from 1 to 1000000000, plus `--version` (`-V`) and `--help` (`-h`). Every command also takes `--algo` (`-a`), which selects the algorithm, and `--resumable`, which continues from a JSON checkpoint in the current directory.

```bash
pi 50
golden 50
sqrt2 50
sqrt3 50
cbrt2 50
e 50
pi --resumable 50
```

## Resume

`--resumable` writes the working integers to `{binary}-{algo}.json` in the current directory and continues from that file on a later run. The file is pretty-printed JSON. Counters are numbers. Big integers are hexadecimal strings with a `0x` prefix, and a leading `-` when negative. Older decimal strings still load. Without the flag, the file is ignored.

The file is rewritten when the completed term count, convergent index, or correct-digit count doubles (1, 2, 4, 8, …), and once more when that request's accumulator is finished, before the digits are printed. A crash or kill does not flush. The next run loads the last fully renamed file and repeats the work done since that write. A leftover `*.json.tmp` is ignored. The file stays after a successful print. It also stores the finished digits. Repeating that count, or any smaller count, prints from those saved digits and does not recompute them. A later run can still ask for more digits. Square-root and cube-root checkpoints keep the guard digits of that value, so a longer request lifts it instead of starting over.

`pi` with `--algo machin` and `e` with `--algo taylor` resume only for the same digit count. A different count is an error and leaves the file unchanged. The other algorithms can continue to a higher count. Two processes must not share one checkpoint file.

## Note

This project was created using agentic tools.

## License

LGPL-3.0-or-later. See [LICENSE](LICENSE).
