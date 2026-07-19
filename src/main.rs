use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use fpx_convert::error::{FpxError, Result};
use fpx_convert::OutputFormat;

enum Args {
    FilePaths {
        input: PathBuf,
        output: PathBuf,
        format: OutputFormat,
    },
    Stdio {
        format: OutputFormat,
    },
    Help,
    Version,
}

const USAGE: &str = "Usage:\n  fpx-convert [--format png|jpeg] <input.fpx> <output>\n  fpx-convert --stdin --stdout [--format png|jpeg]";

// Deliberately verbose and self-contained: this is the primary contract a
// caller invoking fpx-convert as a subprocess (Lumento, another program, or
// an AI coding agent wiring one up) has to go on if it only has the binary
// and not this repo. `fpx-convert --help` should answer "how do I call
// this and what can go wrong" without needing any other file.
const HELP: &str = concat!(
    "fpx-convert ",
    env!("CARGO_PKG_VERSION"),
    " — converts a FlashPix (.fpx) image to PNG or JPEG.\n",
    "\n",
    "Reads one .fpx file, decodes its best available resolution, and writes\n",
    "one image, PNG by default or JPEG with --format jpeg. Camera model and\n",
    "capture date, if present in the source file, are preserved in the\n",
    "output as EXIF (a PNG eXIf chunk, or a JPEG APP1 Exif segment).\n",
    "\n",
    "USAGE:\n",
    "  fpx-convert [--format png|jpeg] <input.fpx> <output>\n",
    "      Reads from and writes to the given file paths. --format controls\n",
    "      the output file's encoding, not its name — the output path is\n",
    "      used exactly as given, extension included.\n",
    "\n",
    "  fpx-convert --stdin --stdout [--format png|jpeg]\n",
    "      Reads .fpx bytes from stdin, writes image bytes to stdout.\n",
    "      --stdin and --stdout are both required and can be given in\n",
    "      either order.\n",
    "\n",
    "  fpx-convert --help | -h\n",
    "  fpx-convert --version | -V\n",
    "\n",
    "OPTIONS:\n",
    "  --format png|jpeg\n",
    "      Output encoding. Defaults to png (lossless) if omitted. jpeg is\n",
    "      lossy; quality is fixed, not caller-configurable.\n",
    "\n",
    "EXIT CODES:\n",
    "  0   success\n",
    "  1   parse or convert error (message on stderr names what failed)\n",
    "  2   usage error (bad or missing arguments)\n",
    "\n",
    "SCOPE:\n",
    "  One file in, one file out, per invocation — no directory/batch mode.\n",
    "  Output is PNG or JPEG only; no other output format.\n",
    "  Only JPEG-compressed FlashPix tiles are supported; other tile\n",
    "  compression types are rejected with a clear error, not guessed at.\n",
    "\n",
    "See specs/0001-fpx-conversion-pipeline.md in the source repository for\n",
    "the full behavioral spec.",
);

fn parse_args(raw: &[String]) -> std::result::Result<Args, &'static str> {
    // Pull `--format <value>` out first, wherever it appears, so it can
    // combine with either invocation shape below without doubling every
    // match arm.
    let mut format = OutputFormat::default();
    let mut rest: Vec<&str> = Vec::new();
    let mut i = 0;
    while i < raw.len() {
        if raw[i] == "--format" {
            format = match raw.get(i + 1).map(String::as_str) {
                Some("png") => OutputFormat::Png,
                Some("jpeg") => OutputFormat::Jpeg,
                _ => return Err(USAGE),
            };
            i += 2;
        } else {
            rest.push(raw[i].as_str());
            i += 1;
        }
    }

    match rest.as_slice() {
        ["--help"] | ["-h"] => Ok(Args::Help),
        ["--version"] | ["-V"] => Ok(Args::Version),
        ["--stdin", "--stdout"] | ["--stdout", "--stdin"] => Ok(Args::Stdio { format }),
        [a, _] if a.starts_with("--") => Err(USAGE),
        [input, output] => Ok(Args::FilePaths {
            input: input.into(),
            output: output.into(),
            format,
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
        Args::FilePaths {
            input,
            output,
            format,
        } => run_file(&input, &output, format),
        Args::Stdio { format } => run_stdio(format),
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

fn run_file(input: &Path, output: &Path, format: OutputFormat) -> Result<()> {
    let bytes = std::fs::read(input).map_err(|source| FpxError::OpenInput {
        path: input.to_path_buf(),
        source,
    })?;
    let file = std::fs::File::create(output)?;
    fpx_convert::convert(&bytes, format, std::io::BufWriter::new(file))
}

fn run_stdio(format: OutputFormat) -> Result<()> {
    let mut bytes = Vec::new();
    std::io::stdin().lock().read_to_end(&mut bytes)?;
    let stdout = std::io::stdout();
    let mut lock = stdout.lock();
    fpx_convert::convert(&bytes, format, &mut lock)?;
    lock.flush()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(strs: &[&str]) -> Vec<String> {
        strs.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn file_paths_default_to_png() {
        let parsed = parse_args(&args(&["in.fpx", "out.png"])).unwrap();
        assert!(matches!(
            parsed,
            Args::FilePaths {
                format: OutputFormat::Png,
                ..
            }
        ));
    }

    #[test]
    fn file_paths_with_format_jpeg_before_positionals() {
        let parsed = parse_args(&args(&["--format", "jpeg", "in.fpx", "out.jpg"])).unwrap();
        match parsed {
            Args::FilePaths {
                input,
                output,
                format,
            } => {
                assert_eq!(input, PathBuf::from("in.fpx"));
                assert_eq!(output, PathBuf::from("out.jpg"));
                assert_eq!(format, OutputFormat::Jpeg);
            }
            _ => panic!("expected FilePaths"),
        }
    }

    #[test]
    fn file_paths_with_format_after_positionals() {
        let parsed = parse_args(&args(&["in.fpx", "out.jpg", "--format", "jpeg"])).unwrap();
        assert!(matches!(
            parsed,
            Args::FilePaths {
                format: OutputFormat::Jpeg,
                ..
            }
        ));
    }

    #[test]
    fn stdio_with_format() {
        let parsed = parse_args(&args(&["--stdin", "--stdout", "--format", "jpeg"])).unwrap();
        assert!(matches!(
            parsed,
            Args::Stdio {
                format: OutputFormat::Jpeg
            }
        ));
    }

    #[test]
    fn stdio_defaults_to_png() {
        let parsed = parse_args(&args(&["--stdout", "--stdin"])).unwrap();
        assert!(matches!(
            parsed,
            Args::Stdio {
                format: OutputFormat::Png
            }
        ));
    }

    #[test]
    fn unknown_format_value_is_usage_error() {
        assert!(parse_args(&args(&["--format", "gif", "in.fpx", "out.gif"])).is_err());
    }

    #[test]
    fn format_flag_missing_value_is_usage_error() {
        assert!(parse_args(&args(&["in.fpx", "out.png", "--format"])).is_err());
    }

    #[test]
    fn help_and_version_still_work() {
        assert!(matches!(parse_args(&args(&["--help"])), Ok(Args::Help)));
        assert!(matches!(parse_args(&args(&["-h"])), Ok(Args::Help)));
        assert!(matches!(
            parse_args(&args(&["--version"])),
            Ok(Args::Version)
        ));
        assert!(matches!(parse_args(&args(&["-V"])), Ok(Args::Version)));
    }
}
