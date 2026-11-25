// Copyright (C) 2026 lfortin
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU Lesser General Public License as published
// by the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful, but
// WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// GNU Lesser General Public License for more details.
//
// You should have received a copy of the GNU Lesser General Public License
// along with this program in the file LICENSE. If not, see
// <https://www.gnu.org/licenses/>.

use clap::error::ErrorKind;
use clap::{CommandFactory, Parser};
use sqrt3_digits::{MAX_DIGITS, Sqrt3Algo, format_sqrt3_with_algo};

const VERSION: &str = concat!("v", env!("CARGO_PKG_VERSION"));

#[derive(Parser)]
#[command(
    name = "sqrt3",
    about = "Print digits of the square root of 3",
    disable_version_flag = true
)]
struct Cli {
    /// Print version
    #[arg(short = 'V', long = "version")]
    version: bool,

    /// Algorithm used to compute the square root of 3
    #[arg(short, long, value_enum, default_value_t = Sqrt3Algo::Newton)]
    algo: Sqrt3Algo,

    /// Number of significant digits to print (1 to 1000000000)
    #[arg(value_parser = digits_in_range)]
    digits: u32,
}

fn is_version_flag(arg: &str) -> bool {
    arg == "--version" || arg == "-V"
}

fn digits_in_range(raw: &str) -> Result<u32, String> {
    let digits: u32 = raw
        .parse()
        .map_err(|_| format!("{raw} is not a whole number of digits"))?;
    if (1..=MAX_DIGITS).contains(&digits) {
        Ok(digits)
    } else {
        Err(format!("digit count must be from 1 to {MAX_DIGITS}"))
    }
}

fn main() {
    if std::env::args().skip(1).any(|arg| is_version_flag(&arg)) {
        println!("{VERSION}");
        return;
    }
    let cli = match Cli::try_parse() {
        Ok(cli) => cli,
        Err(err) => exit_with_cli_error(err),
    };
    if cli.version {
        println!("{VERSION}");
        return;
    }
    match format_sqrt3_with_algo(cli.digits, cli.algo) {
        Ok(text) => println!("{text}"),
        Err(message) => {
            eprintln!("error: {message}");
            let mut command = Cli::command();
            eprintln!("\n{}", command.render_usage());
            std::process::exit(1);
        }
    }
}

fn exit_with_cli_error(err: clap::Error) -> ! {
    if matches!(
        err.kind(),
        ErrorKind::DisplayHelp | ErrorKind::DisplayVersion
    ) {
        err.exit();
    }

    let rendered = err.render().to_string();
    eprint!("{rendered}");
    if !rendered.contains("Usage:") {
        let mut command = Cli::command();
        eprintln!("\n{}", command.render_usage());
    }
    std::process::exit(err.exit_code());
}
