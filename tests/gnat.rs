use std::{
    env, fs,
    path::{Path, PathBuf},
    process::{self, Command, Stdio},
    time::{SystemTime, UNIX_EPOCH},
};

#[test]
fn gnat_compiles_and_runs_repository_examples() {
    if !gnatmake_available() {
        return;
    }

    for stem in [
        "01_hello_world",
        "02_control_flow",
        "03_packages_and_contracts",
        "04_types_and_ranges",
        "05_body_only_package",
    ] {
        run_repository_example(stem);
    }
}

#[test]
fn gnat_compiles_and_runs_split_hello_world() {
    if !gnatmake_available() {
        return;
    }

    let root = temp_test_dir("gnat-hello");
    let input_path = root.join("main.cada");
    let out_dir = root.join("out");

    fs::write(
        &input_path,
        r#"
        import Text_IO;
        use Text_IO;

        fn Main() {
            Put_Line("Hello");
        }
        "#,
    )
    .expect("input should be written");

    run_cadar_split(&input_path, &out_dir);
    run_gnatmake(&out_dir, "main.adb");
    let output = run_binary(&out_dir, "main");

    assert_eq!(String::from_utf8_lossy(&output.stdout), "Hello\n");

    fs::remove_dir_all(root).expect("temp dir should be removed");
}

#[test]
fn gnat_compiles_boolean_logic_program() {
    if !gnatmake_available() {
        return;
    }

    let root = temp_test_dir("gnat-bool");
    let input_path = root.join("main.cada");
    let out_dir = root.join("out");

    fs::write(
        &input_path,
        r#"
        import Text_IO;
        use Text_IO;

        fn Main() {
            if ((true or false) and then not false) {
                Put_Line("ok");
            } else {
                Put_Line("bad");
            }
        }
        "#,
    )
    .expect("input should be written");

    run_cadar_split(&input_path, &out_dir);
    run_gnatmake(&out_dir, "main.adb");
    let output = run_binary(&out_dir, "main");

    assert_eq!(String::from_utf8_lossy(&output.stdout), "ok\n");

    fs::remove_dir_all(root).expect("temp dir should be removed");
}

#[test]
fn gnat_compiles_case_statement_program() {
    if !gnatmake_available() {
        return;
    }

    let root = temp_test_dir("gnat-case");
    let input_path = root.join("main.cada");
    let out_dir = root.join("out");

    fs::write(
        &input_path,
        r#"
        import Text_IO;
        use Text_IO;

        fn Main() {
            Integer Value = 2;
            case (Value) {
                when 0 => {
                    Put_Line("zero");
                }
                when 1, 2 => {
                    Put_Line("small");
                }
                else => {
                    null;
                }
            }
        }
        "#,
    )
    .expect("input should be written");

    run_cadar_split(&input_path, &out_dir);
    run_gnatmake(&out_dir, "main.adb");
    let output = run_binary(&out_dir, "main");

    assert_eq!(String::from_utf8_lossy(&output.stdout), "small\n");

    fs::remove_dir_all(root).expect("temp dir should be removed");
}

#[test]
fn gnat_compiles_split_package_program_without_self_import_cycle() {
    if !gnatmake_available() {
        return;
    }

    let root = temp_test_dir("gnat-package");
    let input_path = root.join("program.cada");
    let out_dir = root.join("out");

    fs::write(
        &input_path,
        r#"
        import Text_IO;
        use Text_IO;
        import Math;

        package body Math {
            fn Add(Integer A, Integer B) -> Integer {
                return A + B;
            }
        }

        fn Main() {
            Put_Line(Integer.image(Math.Add(2, 3)));
        }
        "#,
    )
    .expect("input should be written");

    run_cadar_split(&input_path, &out_dir);
    run_gnatmake(&out_dir, "main.adb");
    let output = run_binary(&out_dir, "main");

    assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "5");

    fs::remove_dir_all(root).expect("temp dir should be removed");
}

fn run_cadar_split(input_path: &Path, out_dir: &Path) {
    let output = Command::new(env!("CARGO_BIN_EXE_cadar"))
        .arg("--write")
        .arg("--split-units")
        .arg("--out-dir")
        .arg(out_dir)
        .arg(input_path)
        .output()
        .expect("cadar should run");

    assert!(
        output.status.success(),
        "cadar failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn run_repository_example(stem: &str) {
    let root = temp_test_dir(&format!("example-{stem}"));
    let out_dir = root.join("out");
    let examples_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples");
    let input_path = examples_dir.join(format!("{stem}.cada"));
    let expected_stdout = fs::read_to_string(examples_dir.join(format!("{stem}.stdout")))
        .expect("expected stdout fixture should be readable");

    run_cadar_split(&input_path, &out_dir);
    run_gnatmake(&out_dir, "main.adb");
    let output = run_binary(&out_dir, "main");

    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        expected_stdout,
        "unexpected stdout for example `{stem}`"
    );

    fs::remove_dir_all(root).expect("temp dir should be removed");
}

fn run_gnatmake(out_dir: &Path, unit: &str) {
    let output = Command::new("gnatmake")
        .arg("-q")
        .arg(unit)
        .current_dir(out_dir)
        .output()
        .expect("gnatmake should run");

    assert!(
        output.status.success(),
        "gnatmake failed:\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn run_binary(out_dir: &Path, name: &str) -> process::Output {
    Command::new(out_dir.join(name))
        .current_dir(out_dir)
        .output()
        .expect("compiled binary should run")
}

fn gnatmake_available() -> bool {
    Command::new("gnatmake")
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

fn temp_test_dir(label: &str) -> PathBuf {
    let mut path = env::temp_dir();
    let unique = format!(
        "cadar-{label}-{}-{}",
        process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time should be monotonic enough for tests")
            .as_nanos()
    );
    path.push(unique);
    fs::create_dir_all(&path).expect("temp dir should be created");
    path
}
