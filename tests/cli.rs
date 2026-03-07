use std::{
    env, fs,
    io::Write,
    path::PathBuf,
    process::{self, Command, Stdio},
    time::{SystemTime, UNIX_EPOCH},
};

#[test]
fn writes_ads_and_adb_files_for_input_path() {
    let root = temp_test_dir("path-output");
    let input_path = root.join("math.cada");
    let out_dir = root.join("out");

    fs::write(
        &input_path,
        r#"
        fn Add(Integer A, Integer B) -> Integer {
            return A + B;
        }
        "#,
    )
    .expect("input should be written");

    let output = Command::new(env!("CARGO_BIN_EXE_cadar"))
        .arg("--write")
        .arg("--out-dir")
        .arg(&out_dir)
        .arg(&input_path)
        .output()
        .expect("cli should run");

    assert!(
        output.status.success(),
        "cli failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        fs::read_to_string(out_dir.join("math.ads")).expect("spec file should exist"),
        "function Add(A : Integer; B : Integer) return Integer;"
    );
    assert_eq!(
        fs::read_to_string(out_dir.join("math.adb")).expect("body file should exist"),
        "function Add(A : Integer; B : Integer) return Integer is\nbegin\n   return A + B;\nend Add;"
    );

    fs::remove_dir_all(root).expect("temp dir should be removed");
}

#[test]
fn writes_files_from_stdin_with_explicit_basename() {
    let root = temp_test_dir("stdin-output");
    let out_dir = root.join("emit");

    let mut child = Command::new(env!("CARGO_BIN_EXE_cadar"))
        .arg("--write")
        .arg("--out-dir")
        .arg(&out_dir)
        .arg("--basename")
        .arg("demo")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("cli should spawn");

    child
        .stdin
        .take()
        .expect("stdin should be piped")
        .write_all(
            br#"
            fn Count() {
                for (Integer I in 1..3) {
                    Put_Line(I);
                }
            }
            "#,
        )
        .expect("stdin should be written");

    let output = child.wait_with_output().expect("cli should complete");
    assert!(
        output.status.success(),
        "cli failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        fs::read_to_string(out_dir.join("demo.ads")).expect("spec file should exist"),
        "procedure Count;"
    );
    assert_eq!(
        fs::read_to_string(out_dir.join("demo.adb")).expect("body file should exist"),
        "procedure Count is\nbegin\n   for I in 1 .. 3 loop\n      Put_Line(I);\n   end loop;\nend Count;"
    );

    fs::remove_dir_all(root).expect("temp dir should be removed");
}

#[test]
fn requires_basename_when_writing_from_stdin() {
    let root = temp_test_dir("stdin-error");
    let out_dir = root.join("emit");

    let mut child = Command::new(env!("CARGO_BIN_EXE_cadar"))
        .arg("--write")
        .arg("--out-dir")
        .arg(&out_dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("cli should spawn");

    child
        .stdin
        .take()
        .expect("stdin should be piped")
        .write_all(
            br#"
            fn Main() {
            }
            "#,
        )
        .expect("stdin should be written");

    let output = child.wait_with_output().expect("cli should complete");
    assert!(
        !output.status.success(),
        "cli unexpectedly succeeded: {}",
        String::from_utf8_lossy(&output.stdout)
    );
    assert!(
        String::from_utf8_lossy(&output.stderr)
            .contains("`--basename` is required when using `--write` with stdin"),
        "unexpected stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    fs::remove_dir_all(root).expect("temp dir should be removed");
}

#[test]
fn prints_source_snippet_for_transpile_errors() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_cadar"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("cli should spawn");

    child
        .stdin
        .take()
        .expect("stdin should be piped")
        .write_all(b"fn Main() {\n    Integer Count = 1;\n    Count + 1;\n}\n")
        .expect("stdin should be written");

    let output = child.wait_with_output().expect("cli should complete");
    assert!(
        !output.status.success(),
        "cli unexpectedly succeeded: {}",
        String::from_utf8_lossy(&output.stdout)
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("error: only call expressions are allowed as standalone statements"),
        "unexpected stderr: {stderr}"
    );
    assert!(
        stderr.contains("--> <stdin>:3:5"),
        "unexpected stderr: {stderr}"
    );
    assert!(
        stderr.contains("3 |     Count + 1;"),
        "unexpected stderr: {stderr}"
    );
    assert!(stderr.contains("|     ^"), "unexpected stderr: {stderr}");
}

#[test]
fn split_units_writes_package_and_subprogram_files() {
    let root = temp_test_dir("split-units");
    let input_path = root.join("bundle.cada");
    let out_dir = root.join("out");

    fs::write(
        &input_path,
        r#"
        import Text_IO;

        package Math {
            fn Add(Integer A, Integer B) -> Integer;
        }

        package body Math {
            fn Add(Integer A, Integer B) -> Integer {
                return A + B;
            }
        }

        fn Main() {
            Text_IO.Put_Line(1);
        }
        "#,
    )
    .expect("input should be written");

    let output = Command::new(env!("CARGO_BIN_EXE_cadar"))
        .arg("--write")
        .arg("--split-units")
        .arg("--out-dir")
        .arg(&out_dir)
        .arg(&input_path)
        .output()
        .expect("cli should run");

    assert!(
        output.status.success(),
        "cli failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        fs::read_to_string(out_dir.join("math.ads")).expect("package spec should exist"),
        "with Text_IO;\n\npackage Math is\n   function Add(A : Integer; B : Integer) return Integer;\nend Math;"
    );
    assert_eq!(
        fs::read_to_string(out_dir.join("math.adb")).expect("package body should exist"),
        "with Text_IO;\n\npackage body Math is\n   function Add(A : Integer; B : Integer) return Integer is\n   begin\n      return A + B;\n   end Add;\nend Math;"
    );
    assert_eq!(
        fs::read_to_string(out_dir.join("main.ads")).expect("main spec should exist"),
        "with Text_IO;\n\nprocedure Main;"
    );
    assert_eq!(
        fs::read_to_string(out_dir.join("main.adb")).expect("main body should exist"),
        "with Text_IO;\n\nprocedure Main is\nbegin\n   Text_IO.Put_Line(1);\nend Main;"
    );

    fs::remove_dir_all(root).expect("temp dir should be removed");
}

#[test]
fn split_units_uses_fallback_file_for_top_level_types() {
    let root = temp_test_dir("split-types");
    let input_path = root.join("shapes.cada");
    let out_dir = root.join("out");

    fs::write(
        &input_path,
        r#"
        type Point = record {
            Integer X;
            Integer Y;
        };

        fn Main() {
        }
        "#,
    )
    .expect("input should be written");

    let output = Command::new(env!("CARGO_BIN_EXE_cadar"))
        .arg("--write")
        .arg("--split-units")
        .arg("--out-dir")
        .arg(&out_dir)
        .arg(&input_path)
        .output()
        .expect("cli should run");

    assert!(
        output.status.success(),
        "cli failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        fs::read_to_string(out_dir.join("shapes.ads")).expect("aggregate spec should exist"),
        "type Point is record\n   X : Integer;\n   Y : Integer;\nend record;"
    );
    assert_eq!(
        fs::read_to_string(out_dir.join("main.ads")).expect("main spec should exist"),
        "procedure Main;"
    );
    assert_eq!(
        fs::read_to_string(out_dir.join("main.adb")).expect("main body should exist"),
        "procedure Main is\nbegin\n   null;\nend Main;"
    );

    fs::remove_dir_all(root).expect("temp dir should be removed");
}

#[test]
fn split_units_derives_package_spec_from_body_only() {
    let root = temp_test_dir("derived-package-spec");
    let input_path = root.join("math_body_only.cada");
    let out_dir = root.join("out");

    fs::write(
        &input_path,
        r#"
        package body Math {
            type Hidden = record {
                Integer Value;
            };
            fn Add(Integer A, Integer B) -> Integer {
                return A + B;
            }
        }
        "#,
    )
    .expect("input should be written");

    let output = Command::new(env!("CARGO_BIN_EXE_cadar"))
        .arg("--write")
        .arg("--split-units")
        .arg("--out-dir")
        .arg(&out_dir)
        .arg(&input_path)
        .output()
        .expect("cli should run");

    assert!(
        output.status.success(),
        "cli failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        fs::read_to_string(out_dir.join("math.ads")).expect("derived spec should exist"),
        "package Math is\n   function Add(A : Integer; B : Integer) return Integer;\nend Math;"
    );
    assert_eq!(
        fs::read_to_string(out_dir.join("math.adb")).expect("package body should exist"),
        "package body Math is\n   type Hidden is record\n      Value : Integer;\n   end record;\n\n   function Add(A : Integer; B : Integer) return Integer is\n   begin\n      return A + B;\n   end Add;\nend Math;"
    );

    fs::remove_dir_all(root).expect("temp dir should be removed");
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
