use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use fpx_convert::error::{FpxError, Result};

enum Args {
    FilePaths { input: PathBuf, output: PathBuf },
    Stdio,
}

const USAGE: &str =
    "Usage:\n  fpx-convert <input.fpx> <output.png>\n  fpx-convert --stdin --stdout";

fn parse_args(raw: &[String]) -> std::result::Result<Args, &'static str> {
    match raw {
        [a, b] if (a == "--stdin" && b == "--stdout") || (a == "--stdout" && b == "--stdin") => {
            Ok(Args::Stdio)
        }
        [a, _] if a.starts_with("--") => Err(USAGE),
        [input, output] => Ok(Args::FilePaths {
            input: input.into(),
            output: output.into(),
        }),
        _ => Err(USAGE),
    }
}

fn main() -> ExitCode {
    let raw: Vec<String> = std::env::args().skip(1).collect();
    let args = match parse_args(&raw) {
        Ok(args) => args,
        Err(usage) => {
            eprintln!("{usage}");
            return ExitCode::from(2);
        }
    };

    let result = match args {
        Args::FilePaths { input, output } => run_file(&input, &output),
        Args::Stdio => run_stdio(),
    };

    if let Err(err) = result {
        eprintln!("fpx-convert: {err}");
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}

fn run_file(input: &Path, output: &Path) -> Result<()> {
    let bytes = std::fs::read(input).map_err(|source| FpxError::OpenInput {
        path: input.to_path_buf(),
        source,
    })?;
    let file = std::fs::File::create(output)?;
    fpx_convert::convert(&bytes, std::io::BufWriter::new(file))
}

fn run_stdio() -> Result<()> {
    let mut bytes = Vec::new();
    std::io::stdin().lock().read_to_end(&mut bytes)?;
    let stdout = std::io::stdout();
    let mut lock = stdout.lock();
    fpx_convert::convert(&bytes, &mut lock)?;
    lock.flush()?;
    Ok(())
}
