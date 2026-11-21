// Copyright (C) 2026 pi-digits authors
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
// You should have received copies of the GNU Lesser General Public License
// and the GNU General Public License along with this program. If not, see
// <https://www.gnu.org/licenses/>.

use clap::error::ErrorKind;
use clap::{CommandFactory, Parser};
use pi_digits::{MAX_DIGITS, format_pi};

#[derive(Parser)]
#[command(name = "pi", version, about = "Print digits of pi")]
struct Cli {
    /// Number of significant digits to print (1 to 100000)
    #[arg(value_parser = digits_in_range)]
    digits: u32,
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
    let cli = match Cli::try_parse() {
        Ok(cli) => cli,
        Err(err) => exit_with_cli_error(err),
    };
    match format_pi(cli.digits) {
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
