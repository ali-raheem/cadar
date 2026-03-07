use std::{
    env, fs,
    io::{self, Read},
    path::{Path, PathBuf},
    process,
};

use cadar::{AdaOutputs, GeneratedFile};

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let cli = Cli::parse(env::args().skip(1))?;
    if cli.show_help {
        print!("{}", usage());
        return Ok(());
    }

    let source = match &cli.input_path {
        Some(path) => fs::read_to_string(path).map_err(|error| error.to_string())?,
        None => {
            let mut source = String::new();
            io::stdin()
                .read_to_string(&mut source)
                .map_err(|error| error.to_string())?;
            source
        }
    };
    let diagnostic_label = cli
        .input_path
        .as_ref()
        .map(|path| path.display().to_string())
        .unwrap_or_else(|| "<stdin>".to_string());

    if cli.write_files {
        if cli.split_units {
            let fallback_stem = resolve_fallback_stem(&cli);
            let files = cadar::transpile_files(&source, &fallback_stem)
                .map_err(|error| error.render_with_source(&source, Some(&diagnostic_label)))?;
            write_split_files(&cli, &files).map_err(|error| error.to_string())?;
        } else {
            let outputs = cadar::transpile(&source)
                .map_err(|error| error.render_with_source(&source, Some(&diagnostic_label)))?;
            write_outputs(&cli, &outputs).map_err(|error| error.to_string())?;
        }
    } else {
        let outputs = cadar::transpile(&source)
            .map_err(|error| error.render_with_source(&source, Some(&diagnostic_label)))?;
        print_outputs(&outputs);
    }

    Ok(())
}

#[derive(Debug, Default)]
struct Cli {
    input_path: Option<PathBuf>,
    write_files: bool,
    split_units: bool,
    out_dir: Option<PathBuf>,
    basename: Option<String>,
    show_help: bool,
}

impl Cli {
    fn parse(args: impl Iterator<Item = String>) -> Result<Self, String> {
        let mut cli = Self::default();
        let mut args = args.peekable();

        while let Some(arg) = args.next() {
            match arg.as_str() {
                "-h" | "--help" => cli.show_help = true,
                "--write" => cli.write_files = true,
                "--split-units" => cli.split_units = true,
                "--out-dir" => {
                    let Some(path) = args.next() else {
                        return Err("`--out-dir` requires a directory path".to_string());
                    };
                    cli.out_dir = Some(PathBuf::from(path));
                }
                "--basename" => {
                    let Some(name) = args.next() else {
                        return Err("`--basename` requires a file stem".to_string());
                    };
                    if name.trim().is_empty() {
                        return Err("`--basename` cannot be empty".to_string());
                    }
                    cli.basename = Some(name);
                }
                _ if arg.starts_with('-') => {
                    return Err(format!("unknown option `{arg}`"));
                }
                _ => {
                    if cli.input_path.is_some() {
                        return Err("usage: cadar [--write] [--out-dir DIR] [--basename NAME] [path/to/file.cada]".to_string());
                    }
                    cli.input_path = Some(PathBuf::from(arg));
                }
            }
        }

        if cli.show_help {
            return Ok(cli);
        }

        if !cli.write_files && (cli.out_dir.is_some() || cli.basename.is_some()) {
            return Err("`--out-dir` and `--basename` require `--write`".to_string());
        }
        if cli.split_units && !cli.write_files {
            return Err("`--split-units` requires `--write`".to_string());
        }

        Ok(cli)
    }
}

fn usage() -> &'static str {
    "Usage: cadar [--write] [--out-dir DIR] [--basename NAME] [path/to/file.cada]\n\n\
Reads CADA source from a file or stdin.\n\
\n\
Options:\n\
  --write           Write .ads/.adb files instead of printing to stdout\n\
  --split-units     Write one Ada file per top-level package/subprogram unit\n\
  --out-dir DIR     Directory for emitted files when using --write\n\
  --basename NAME   File stem to use when writing files from stdin\n\
  -h, --help        Show this help text\n"
}

fn write_outputs(cli: &Cli, outputs: &AdaOutputs) -> Result<(), Box<dyn std::error::Error>> {
    let basename = resolve_basename(cli)?;
    let out_dir = resolve_out_dir(cli)?;
    fs::create_dir_all(&out_dir)?;

    let mut written = Vec::new();
    if !outputs.spec.is_empty() {
        let path = out_dir.join(format!("{basename}.ads"));
        fs::write(&path, &outputs.spec)?;
        written.push(path);
    }
    if !outputs.body.is_empty() {
        let path = out_dir.join(format!("{basename}.adb"));
        fs::write(&path, &outputs.body)?;
        written.push(path);
    }

    for path in written {
        println!("wrote {}", path.display());
    }

    Ok(())
}

fn write_split_files(cli: &Cli, files: &[GeneratedFile]) -> Result<(), Box<dyn std::error::Error>> {
    let out_dir = resolve_out_dir(cli)?;
    fs::create_dir_all(&out_dir)?;

    for file in files {
        let path = out_dir.join(&file.filename);
        fs::write(&path, &file.contents)?;
        println!("wrote {}", path.display());
    }

    Ok(())
}

fn resolve_basename(cli: &Cli) -> Result<String, Box<dyn std::error::Error>> {
    if let Some(name) = &cli.basename {
        return Ok(name.clone());
    }

    let Some(input_path) = &cli.input_path else {
        return Err("`--basename` is required when using `--write` with stdin".into());
    };

    let Some(stem) = input_path.file_stem().and_then(|stem| stem.to_str()) else {
        return Err(format!(
            "could not determine a basename from input path `{}`",
            input_path.display()
        )
        .into());
    };

    Ok(stem.to_string())
}

fn resolve_out_dir(cli: &Cli) -> Result<PathBuf, Box<dyn std::error::Error>> {
    if let Some(path) = &cli.out_dir {
        return Ok(path.clone());
    }

    if let Some(input_path) = &cli.input_path {
        return Ok(input_path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .to_path_buf());
    }

    Ok(env::current_dir()?)
}

fn resolve_fallback_stem(cli: &Cli) -> String {
    if let Some(name) = &cli.basename {
        return name.clone();
    }

    if let Some(input_path) = &cli.input_path
        && let Some(stem) = input_path.file_stem().and_then(|stem| stem.to_str())
    {
        return stem.to_string();
    }

    "output".to_string()
}

fn print_outputs(outputs: &AdaOutputs) {
    if !outputs.spec.is_empty() {
        println!("-- spec (.ads)");
        println!("{}", outputs.spec);
    }
    if !outputs.spec.is_empty() && !outputs.body.is_empty() {
        println!();
    }
    if !outputs.body.is_empty() {
        println!("-- body (.adb)");
        println!("{}", outputs.body);
    }
}
