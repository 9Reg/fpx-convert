use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use fpx_convert::error::{FpxError, Result};

enum Args {
    FilePaths { input: PathBuf, output: PathBuf },
    Stdio,
    Help,
    Version,
}

const USAGE: &str =
    "Usage:\n  fpx-convert <input.fpx> <output.png>\n  fpx-convert --stdin --stdout";

// Deliberately verbose and self-contained: this is the primary contract a
// caller invoking fpx-convert as a subprocess (Lumento, another program, or
// an AI coding agent wiring one up) has to go on if it only has the binary
// and not this repo. `fpx-convert --help` should answer "how do I call
// this and what can go wrong" without needing any other file.
const HELP: &str = concat!(
    "fpx-convert ",
    env!("CARGO_PKG_VERSION"),
    " — converts a FlashPix (.fpx) image to a lossless PNG.\n",
    "\n",
    "Reads one .fpx file, decodes its best available resolution, and writes\n",
    "one PNG. Camera model and capture date, if present in the source file,\n",
    "are preserved in the output as a PNG eXIf chunk.\n",
    "\n",
    "USAGE:\n",
    "  fpx-convert <input.fpx> <output.png>\n",
    "      Reads from and writes to the given file paths.\n",
    "\n",
    "  fpx-convert --stdin --stdout\n",
    "      Reads .fpx bytes from stdin, writes PNG bytes to stdout.\n",
    "      Both flags are required and can be given in either order.\n",
    "\n",
    "  fpx-convert --help | -h\n",
    "  fpx-convert --version | -V\n",
    "\n",
    "EXIT CODES:\n",
    "  0   success\n",
    "  1   parse or convert error (message on stderr names what failed)\n",
    "  2   usage error (bad or missing arguments)\n",
    "\n",
    "SCOPE:\n",
    "  One file in, one file out, per invocation — no directory/batch mode.\n",
    "  Output is always PNG; there is no other output format or fallback.\n",
    "  Only JPEG-compressed FlashPix tiles are supported; other tile\n",
    "  compression types are rejected with a clear error, not guessed at.\n",
    "\n",
    "See specs/0001-fpx-conversion-pipeline.md in the source repository for\n",
    "the full behavioral spec.",
);

fn parse_args(raw: &[String]) -> std::result::Result<Args, &'static str> {
    match raw {
        [a] if a == "--help" || a == "-h" => Ok(Args::Help),
        [a] if a == "--version" || a == "-V" => Ok(Args::Version),
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
        Args::Help => {
            println!("{HELP}");
            return ExitCode::SUCCESS;
        }
        Args::Version => {
            println!("fpx-convert {}", env!("CARGO_PKG_VERSION"));
            return ExitCode::SUCCESS;
        }
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
