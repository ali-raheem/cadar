use std::{
    env, fs,
    io::{self, Read},
    path::{Path, PathBuf},
    process::{self, Command},
};

use cadar::{AdaOutputs, GeneratedFile, IndexedDiagnostic, SourceInput};

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

    let bundle = read_source_bundle(&cli)?;
    let sources = bundle
        .documents
        .iter()
        .map(|document| SourceInput {
            source: &document.source,
        })
        .collect::<Vec<_>>();

    if cli.write_files {
        let out_dir = resolve_out_dir(&cli).map_err(|error| error.to_string())?;
        if cli.split_units {
            let fallback_stem = resolve_fallback_stem(&cli);
            let files = cadar::transpile_project_files(&sources, &fallback_stem)
                .map_err(|error| render_source_diagnostic(error, &bundle))?;
            write_split_files(&out_dir, &files).map_err(|error| error.to_string())?;
            if cli.emit_project {
                write_project_file(&out_dir).map_err(|error| error.to_string())?;
            }
        } else {
            let outputs = cadar::transpile_project(&sources)
                .map_err(|error| render_source_diagnostic(error, &bundle))?;
            write_outputs(&cli, &out_dir, &outputs).map_err(|error| error.to_string())?;
        }

        if cli.build {
            let build_unit = resolve_build_unit(&cli).map_err(|error| error.to_string())?;
            build_outputs(&out_dir, &build_unit).map_err(|error| error.to_string())?;
        }
    } else {
        let outputs = cadar::transpile_project(&sources)
            .map_err(|error| render_source_diagnostic(error, &bundle))?;
        print_outputs(&outputs);
    }

    Ok(())
}

#[derive(Debug, Default)]
struct Cli {
    input_paths: Vec<PathBuf>,
    write_files: bool,
    split_units: bool,
    build: bool,
    emit_project: bool,
    build_unit: Option<String>,
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
                "--build" => cli.build = true,
                "--emit-project" => cli.emit_project = true,
                "--build-unit" => {
                    let Some(unit) = args.next() else {
                        return Err("`--build-unit` requires an Ada body filename".to_string());
                    };
                    if unit.trim().is_empty() {
                        return Err("`--build-unit` cannot be empty".to_string());
                    }
                    cli.build_unit = Some(unit);
                }
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
                    cli.input_paths.push(PathBuf::from(arg));
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
        if cli.build && !cli.write_files {
            return Err("`--build` requires `--write`".to_string());
        }
        if cli.emit_project && !cli.write_files {
            return Err("`--emit-project` requires `--write`".to_string());
        }
        if cli.emit_project && !cli.split_units {
            return Err("`--emit-project` requires `--split-units`".to_string());
        }
        if cli.build_unit.is_some() && !cli.build {
            return Err("`--build-unit` requires `--build`".to_string());
        }

        Ok(cli)
    }
}

fn usage() -> &'static str {
    "Usage: cadar [--write] [--split-units] [--build] [--emit-project] [--build-unit FILE] [--out-dir DIR] [--basename NAME] [path/to/file1.cada ...]\n\n\
Reads CADA source from one or more files, or from stdin when no input paths are given.\n\
\n\
Options:\n\
  --write           Write .ads/.adb files instead of printing to stdout\n\
  --split-units     Write one Ada file per top-level package/subprogram unit\n\
  --build           Run `gnatmake -q` on the emitted Ada after writing files\n\
  --emit-project    Write a `cadar.gpr` GNAT project file for split-unit output\n\
  --build-unit FILE Ada body file to pass to `gnatmake` (default: `main.adb` for split units)\n\
  --out-dir DIR     Directory for emitted files when using --write\n\
  --basename NAME   File stem to use when writing aggregate files from stdin\n\
  -h, --help        Show this help text\n"
}

fn write_outputs(
    cli: &Cli,
    out_dir: &Path,
    outputs: &AdaOutputs,
) -> Result<(), Box<dyn std::error::Error>> {
    let basename = resolve_basename(cli)?;
    fs::create_dir_all(out_dir)?;

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

fn write_split_files(
    out_dir: &Path,
    files: &[GeneratedFile],
) -> Result<(), Box<dyn std::error::Error>> {
    fs::create_dir_all(out_dir)?;

    for file in files {
        let path = out_dir.join(&file.filename);
        fs::write(&path, &file.contents)?;
        println!("wrote {}", path.display());
    }

    Ok(())
}

fn write_project_file(out_dir: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let path = out_dir.join("cadar.gpr");
    fs::write(
        &path,
        "project Cadar is\n   for Source_Dirs use (\".\");\n   for Object_Dir use \"obj\";\n   for Exec_Dir use \".\";\nend Cadar;\n",
    )?;
    println!("wrote {}", path.display());
    Ok(())
}

fn resolve_basename(cli: &Cli) -> Result<String, Box<dyn std::error::Error>> {
    if let Some(name) = &cli.basename {
        return Ok(name.clone());
    }

    let Some(input_path) = cli.input_paths.first() else {
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

    if let Some(input_path) = cli.input_paths.first() {
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

    if let Some(input_path) = cli.input_paths.first()
        && let Some(stem) = input_path.file_stem().and_then(|stem| stem.to_str())
    {
        return stem.to_string();
    }

    "output".to_string()
}

fn resolve_build_unit(cli: &Cli) -> Result<String, Box<dyn std::error::Error>> {
    if let Some(unit) = &cli.build_unit {
        return Ok(unit.clone());
    }

    if cli.split_units {
        return Ok("main.adb".to_string());
    }

    Ok(format!("{}.adb", resolve_basename(cli)?))
}

fn build_outputs(out_dir: &Path, build_unit: &str) -> Result<(), Box<dyn std::error::Error>> {
    let build_path = out_dir.join(build_unit);
    if !build_path.exists() {
        return Err(format!(
            "cannot build `{build_unit}` because it was not emitted in `{}`",
            out_dir.display()
        )
        .into());
    }

    let mut command = Command::new("gnatmake");
    command.arg("-q");
    let output = command.arg(build_unit).current_dir(out_dir).output()?;

    if !output.status.success() {
        return Err(format!(
            "gnatmake failed for `{build_unit}`:\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        )
        .into());
    }

    println!("built {}", build_path.display());
    Ok(())
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

#[derive(Debug)]
struct SourceBundle {
    documents: Vec<SourceDocument>,
}

#[derive(Debug)]
struct SourceDocument {
    label: String,
    source: String,
}

fn read_source_bundle(cli: &Cli) -> Result<SourceBundle, String> {
    let mut documents = Vec::new();

    if cli.input_paths.is_empty() {
        let mut source = String::new();
        io::stdin()
            .read_to_string(&mut source)
            .map_err(|error| error.to_string())?;
        documents.push(SourceDocument {
            label: "<stdin>".to_string(),
            source,
        });
    } else {
        for path in &cli.input_paths {
            let source = fs::read_to_string(path).map_err(|error| error.to_string())?;
            documents.push(SourceDocument {
                label: path.display().to_string(),
                source,
            });
        }
    }

    Ok(SourceBundle { documents })
}

fn render_source_diagnostic(error: IndexedDiagnostic, bundle: &SourceBundle) -> String {
    let Some(document) = bundle.documents.get(error.source_index) else {
        return error.diagnostic.to_string();
    };

    error
        .diagnostic
        .render_with_source(&document.source, Some(&document.label))
}
