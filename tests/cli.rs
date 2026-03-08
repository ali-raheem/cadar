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
fn build_requires_write() {
    let output = Command::new(env!("CARGO_BIN_EXE_cadar"))
        .arg("--build")
        .output()
        .expect("cli should run");

    assert!(
        !output.status.success(),
        "cli unexpectedly succeeded: {}",
        String::from_utf8_lossy(&output.stdout)
    );
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("`--build` requires `--write`"),
        "unexpected stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn version_flag_prints_package_version() {
    let output = Command::new(env!("CARGO_BIN_EXE_cadar"))
        .arg("--version")
        .output()
        .expect("cli should run");

    assert!(
        output.status.success(),
        "cli failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        format!("cadar {}\n", env!("CARGO_PKG_VERSION"))
    );
}

#[test]
fn build_unit_requires_build() {
    let output = Command::new(env!("CARGO_BIN_EXE_cadar"))
        .arg("--build-unit")
        .arg("main.adb")
        .output()
        .expect("cli should run");

    assert!(
        !output.status.success(),
        "cli unexpectedly succeeded: {}",
        String::from_utf8_lossy(&output.stdout)
    );
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("`--build-unit` requires `--build`"),
        "unexpected stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn build_reports_missing_gnatmake_in_path() {
    let root = temp_test_dir("missing-gnatmake");
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

    let output = Command::new(env!("CARGO_BIN_EXE_cadar"))
        .arg("--write")
        .arg("--split-units")
        .arg("--build")
        .arg("--out-dir")
        .arg(&out_dir)
        .arg(&input_path)
        .env("PATH", "/nonexistent")
        .output()
        .expect("cli should run");

    assert!(
        !output.status.success(),
        "cli unexpectedly succeeded: {}",
        String::from_utf8_lossy(&output.stdout)
    );
    assert!(
        String::from_utf8_lossy(&output.stderr).contains(
            "failed to run `gnatmake -q main.adb`: `gnatmake` was not found in PATH; install GNAT so `gnatmake` is available"
        ),
        "unexpected stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    fs::remove_dir_all(root).expect("temp dir should be removed");
}

#[test]
fn build_reports_missing_gprbuild_in_path() {
    let root = temp_test_dir("missing-gprbuild");
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

    let output = Command::new(env!("CARGO_BIN_EXE_cadar"))
        .arg("--write")
        .arg("--split-units")
        .arg("--emit-project")
        .arg("--build")
        .arg("--out-dir")
        .arg(&out_dir)
        .arg(&input_path)
        .env("PATH", "/nonexistent")
        .output()
        .expect("cli should run");

    assert!(
        !output.status.success(),
        "cli unexpectedly succeeded: {}",
        String::from_utf8_lossy(&output.stdout)
    );
    assert!(
        String::from_utf8_lossy(&output.stderr).contains(
            "failed to run `gprbuild -q -p -P cadar.gpr main.adb`: `gprbuild` was not found in PATH; install `gprbuild` or omit `--emit-project`"
        ),
        "unexpected stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    fs::remove_dir_all(root).expect("temp dir should be removed");
}

#[test]
fn emit_project_requires_write() {
    let output = Command::new(env!("CARGO_BIN_EXE_cadar"))
        .arg("--emit-project")
        .output()
        .expect("cli should run");

    assert!(
        !output.status.success(),
        "cli unexpectedly succeeded: {}",
        String::from_utf8_lossy(&output.stdout)
    );
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("`--emit-project` requires `--write`"),
        "unexpected stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn emit_project_requires_split_units() {
    let output = Command::new(env!("CARGO_BIN_EXE_cadar"))
        .arg("--write")
        .arg("--emit-project")
        .output()
        .expect("cli should run");

    assert!(
        !output.status.success(),
        "cli unexpectedly succeeded: {}",
        String::from_utf8_lossy(&output.stdout)
    );
    assert!(
        String::from_utf8_lossy(&output.stderr)
            .contains("`--emit-project` requires `--split-units`"),
        "unexpected stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn build_reports_missing_default_main_unit() {
    let root = temp_test_dir("build-missing-main");
    let input_path = root.join("helper.cada");
    let out_dir = root.join("out");

    fs::write(
        &input_path,
        r#"
        fn Helper() {
        }
        "#,
    )
    .expect("input should be written");

    let output = Command::new(env!("CARGO_BIN_EXE_cadar"))
        .arg("--write")
        .arg("--split-units")
        .arg("--build")
        .arg("--out-dir")
        .arg(&out_dir)
        .arg(&input_path)
        .output()
        .expect("cli should run");

    assert!(
        !output.status.success(),
        "cli unexpectedly succeeded: {}",
        String::from_utf8_lossy(&output.stdout)
    );
    assert!(
        String::from_utf8_lossy(&output.stderr)
            .contains("cannot build `main.adb` because it was not emitted"),
        "unexpected stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    fs::remove_dir_all(root).expect("temp dir should be removed");
}

#[test]
fn split_units_emit_project_file() {
    let root = temp_test_dir("emit-project");
    let input_path = root.join("main.cada");
    let out_dir = root.join("out");

    fs::write(
        &input_path,
        r#"
        fn Main() {
        }
        "#,
    )
    .expect("input should be written");

    let output = Command::new(env!("CARGO_BIN_EXE_cadar"))
        .arg("--write")
        .arg("--split-units")
        .arg("--emit-project")
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

    let project = fs::read_to_string(out_dir.join("cadar.gpr")).expect("project file should exist");
    assert!(
        project.contains("project Cadar is"),
        "unexpected project file: {project}"
    );
    assert!(
        project.contains("for Source_Dirs use (\".\");"),
        "unexpected project file: {project}"
    );
    assert!(
        project.contains("for Object_Dir use \"obj\";"),
        "unexpected project file: {project}"
    );
    assert!(
        project.contains("for Exec_Dir use \".\";"),
        "unexpected project file: {project}"
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
fn split_units_reject_top_level_overloaded_subprograms() {
    let root = temp_test_dir("split-overload-error");
    let input_path = root.join("bundle.cada");
    let out_dir = root.join("out");

    fs::write(
        &input_path,
        r#"
        fn Parse(String Text) -> Integer {
            return 1;
        }

        fn Parse(String Text) -> Boolean {
            return true;
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
        !output.status.success(),
        "cli unexpectedly succeeded: {}",
        String::from_utf8_lossy(&output.stdout)
    );
    assert!(
        String::from_utf8_lossy(&output.stderr).contains(
            "split-unit output does not support overloaded top-level subprograms like `Parse`"
        ),
        "unexpected stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    fs::remove_dir_all(root).expect("temp dir should be removed");
}

#[test]
fn split_units_rejects_called_top_level_subprogram_without_import() {
    let root = temp_test_dir("split-top-level-call");
    let input_path = root.join("bundle.cada");
    let out_dir = root.join("out");

    fs::write(
        &input_path,
        r#"
        import Text_IO;
        use Text_IO;

        fn Adjust(Integer Value) -> Integer {
            return Value + 1;
        }

        fn Main() {
            Put_Line(Integer.image(Adjust(2)));
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
        !output.status.success(),
        "cli unexpectedly succeeded: {}",
        String::from_utf8_lossy(&output.stdout)
    );
    assert!(
        String::from_utf8_lossy(&output.stderr)
            .contains("top-level subprogram `Adjust` is not visible; add `import Adjust;`"),
        "unexpected stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    fs::remove_dir_all(root).expect("temp dir should be removed");
}

#[test]
fn split_units_rejects_use_of_top_level_subprogram() {
    let root = temp_test_dir("split-use-top-level-subprogram");
    let input_path = root.join("bundle.cada");

    fs::write(
        &input_path,
        r#"
        use Adjust;

        fn Adjust(Integer Value) -> Integer {
            return Value + 1;
        }
        "#,
    )
    .expect("input should be written");

    let output = Command::new(env!("CARGO_BIN_EXE_cadar"))
        .arg("--write")
        .arg("--split-units")
        .arg(&input_path)
        .output()
        .expect("cli should run");

    assert!(
        !output.status.success(),
        "cli unexpectedly succeeded: {}",
        String::from_utf8_lossy(&output.stdout)
    );
    assert!(
        String::from_utf8_lossy(&output.stderr).contains(
            "`use Adjust` is not valid because `Adjust` is a top-level subprogram; use `import Adjust;` and call it explicitly"
        ),
        "unexpected stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    fs::remove_dir_all(root).expect("temp dir should be removed");
}

#[test]
fn split_units_rejects_import_of_known_package_member() {
    let root = temp_test_dir("split-import-package-member");
    let input_path = root.join("bundle.cada");

    fs::write(
        &input_path,
        r#"
        import Math.Add;

        package Math {
            fn Add(Integer A, Integer B) -> Integer;
        }
        "#,
    )
    .expect("input should be written");

    let output = Command::new(env!("CARGO_BIN_EXE_cadar"))
        .arg("--write")
        .arg("--split-units")
        .arg(&input_path)
        .output()
        .expect("cli should run");

    assert!(
        !output.status.success(),
        "cli unexpectedly succeeded: {}",
        String::from_utf8_lossy(&output.stdout)
    );
    assert!(
        String::from_utf8_lossy(&output.stderr).contains(
            "`import Math.Add` is not valid because `Math.Add` names a member of package `Math`; import the package instead"
        ),
        "unexpected stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    fs::remove_dir_all(root).expect("temp dir should be removed");
}

#[test]
fn split_units_rejects_late_import_clause() {
    let root = temp_test_dir("split-late-import");
    let input_path = root.join("bundle.cada");

    fs::write(
        &input_path,
        r#"
        fn Main() {
            null;
        }

        import Text_IO;
        "#,
    )
    .expect("input should be written");

    let output = Command::new(env!("CARGO_BIN_EXE_cadar"))
        .arg("--write")
        .arg("--split-units")
        .arg(&input_path)
        .output()
        .expect("cli should run");

    assert!(
        !output.status.success(),
        "cli unexpectedly succeeded: {}",
        String::from_utf8_lossy(&output.stdout)
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("`import` clauses must appear before top-level declarations"),
        "unexpected stderr: {stderr}"
    );
    assert!(
        stderr.contains(&format!("--> {}:6:", input_path.display())),
        "unexpected stderr: {stderr}"
    );

    fs::remove_dir_all(root).expect("temp dir should be removed");
}

#[test]
fn split_units_rejects_ada_reserved_word_identifier() {
    let root = temp_test_dir("split-reserved-word");
    let input_path = root.join("bundle.cada");

    fs::write(
        &input_path,
        r#"
        fn Record() {
            null;
        }
        "#,
    )
    .expect("input should be written");

    let output = Command::new(env!("CARGO_BIN_EXE_cadar"))
        .arg("--write")
        .arg("--split-units")
        .arg(&input_path)
        .output()
        .expect("cli should run");

    assert!(
        !output.status.success(),
        "cli unexpectedly succeeded: {}",
        String::from_utf8_lossy(&output.stdout)
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains(
            "top-level subprogram `Record` uses Ada reserved word `record` and cannot be used as an identifier"
        ) || stderr.contains(
            "subprogram `Record` uses Ada reserved word `record` and cannot be used as an identifier"
        ),
        "unexpected stderr: {stderr}"
    );
    assert!(
        stderr.contains(&format!("--> {}:2:", input_path.display())),
        "unexpected stderr: {stderr}"
    );

    fs::remove_dir_all(root).expect("temp dir should be removed");
}

#[test]
fn split_units_adds_with_clause_for_called_top_level_subprogram() {
    let root = temp_test_dir("split-top-level-call-imported");
    let input_path = root.join("bundle.cada");
    let out_dir = root.join("out");

    fs::write(
        &input_path,
        r#"
        import Text_IO;
        use Text_IO;
        import Adjust;

        fn Adjust(Integer Value) -> Integer {
            return Value + 1;
        }

        fn Main() {
            Put_Line(Integer.image(Adjust(2)));
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
        fs::read_to_string(out_dir.join("main.adb")).expect("main body should exist"),
        "with Text_IO;\nuse Text_IO;\nwith Adjust;\n\nprocedure Main is\nbegin\n   Put_Line(Integer'Image(Adjust(2)));\nend Main;"
    );

    fs::remove_dir_all(root).expect("temp dir should be removed");
}

#[test]
fn split_units_adds_with_clause_for_use_without_import() {
    let root = temp_test_dir("split-use-without-import");
    let input_path = root.join("main.cada");
    let out_dir = root.join("out");

    fs::write(
        &input_path,
        r#"
        use Text_IO;

        fn Main() {
            Put_Line("Hello");
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
        fs::read_to_string(out_dir.join("main.adb")).expect("main body should exist"),
        "with Text_IO;\nuse Text_IO;\n\nprocedure Main is\nbegin\n   Put_Line(\"Hello\");\nend Main;"
    );

    fs::remove_dir_all(root).expect("temp dir should be removed");
}

#[test]
fn split_units_accept_multiple_input_files() {
    let root = temp_test_dir("split-multi-file");
    let adjust_path = root.join("adjust.cada");
    let main_path = root.join("main.cada");
    let out_dir = root.join("out");

    fs::write(
        &adjust_path,
        r#"
        fn Adjust(Integer Value) -> Integer {
            return Value + 1;
        }
        "#,
    )
    .expect("adjust input should be written");
    fs::write(
        &main_path,
        r#"
        import Text_IO;
        use Text_IO;
        import Adjust;

        fn Main() {
            Put_Line(Integer.image(Adjust(2)));
        }
        "#,
    )
    .expect("main input should be written");

    let output = Command::new(env!("CARGO_BIN_EXE_cadar"))
        .arg("--write")
        .arg("--split-units")
        .arg("--out-dir")
        .arg(&out_dir)
        .arg(&adjust_path)
        .arg(&main_path)
        .output()
        .expect("cli should run");

    assert!(
        output.status.success(),
        "cli failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        fs::read_to_string(out_dir.join("adjust.ads")).expect("adjust spec should exist"),
        "function Adjust(Value : Integer) return Integer;"
    );
    assert_eq!(
        fs::read_to_string(out_dir.join("main.adb")).expect("main body should exist"),
        "with Text_IO;\nuse Text_IO;\nwith Adjust;\n\nprocedure Main is\nbegin\n   Put_Line(Integer'Image(Adjust(2)));\nend Main;"
    );

    fs::remove_dir_all(root).expect("temp dir should be removed");
}

#[test]
fn split_units_rejects_referenced_package_without_import() {
    let root = temp_test_dir("split-package-dependency");
    let math_path = root.join("math.cada");
    let main_path = root.join("main.cada");
    let out_dir = root.join("out");

    fs::write(
        &math_path,
        r#"
        package body Math {
            fn Add(Integer A, Integer B) -> Integer {
                return A + B;
            }
        }
        "#,
    )
    .expect("math input should be written");
    fs::write(
        &main_path,
        r#"
        import Text_IO;
        use Text_IO;

        fn Main() {
            Put_Line(Integer.image(Math.Add(2, 3)));
        }
        "#,
    )
    .expect("main input should be written");

    let output = Command::new(env!("CARGO_BIN_EXE_cadar"))
        .arg("--write")
        .arg("--split-units")
        .arg("--out-dir")
        .arg(&out_dir)
        .arg(&math_path)
        .arg(&main_path)
        .output()
        .expect("cli should run");

    assert!(
        !output.status.success(),
        "cli unexpectedly succeeded: {}",
        String::from_utf8_lossy(&output.stdout)
    );
    assert!(
        String::from_utf8_lossy(&output.stderr)
            .contains("package `Math` is not visible; add `import Math;` or `use Math;`"),
        "unexpected stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    fs::remove_dir_all(root).expect("temp dir should be removed");
}

#[test]
fn split_units_adds_with_clause_for_referenced_package_with_import() {
    let root = temp_test_dir("split-package-dependency-imported");
    let math_path = root.join("math.cada");
    let main_path = root.join("main.cada");
    let out_dir = root.join("out");

    fs::write(
        &math_path,
        r#"
        package body Math {
            fn Add(Integer A, Integer B) -> Integer {
                return A + B;
            }
        }
        "#,
    )
    .expect("math input should be written");
    fs::write(
        &main_path,
        r#"
        import Text_IO;
        use Text_IO;
        import Math;

        fn Main() {
            Put_Line(Integer.image(Math.Add(2, 3)));
        }
        "#,
    )
    .expect("main input should be written");

    let output = Command::new(env!("CARGO_BIN_EXE_cadar"))
        .arg("--write")
        .arg("--split-units")
        .arg("--out-dir")
        .arg(&out_dir)
        .arg(&math_path)
        .arg(&main_path)
        .output()
        .expect("cli should run");

    assert!(
        output.status.success(),
        "cli failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        fs::read_to_string(out_dir.join("main.adb")).expect("main body should exist"),
        "with Text_IO;\nuse Text_IO;\nwith Math;\n\nprocedure Main is\nbegin\n   Put_Line(Integer'Image(Math.Add(2, 3)));\nend Main;"
    );

    fs::remove_dir_all(root).expect("temp dir should be removed");
}

#[test]
fn split_units_add_with_clause_for_cross_package_type_signatures() {
    let root = temp_test_dir("split-cross-package-types");
    let inventory_path = root.join("inventory.cada");
    let reports_path = root.join("reports.cada");
    let out_dir = root.join("out");

    fs::write(
        &inventory_path,
        r#"
        package Inventory {
            type Item = record {
                Integer Quantity;
                Integer Price;
            };

            type Item_Array = [0..1] Item;
        }
        "#,
    )
    .expect("inventory input should be written");
    fs::write(
        &reports_path,
        r#"
        import Inventory;

        package Reports {
            fn First_Quantity(Inventory.Item_Array Items) -> Integer;
        }

        package body Reports {
            fn First_Quantity(Inventory.Item_Array Items) -> Integer {
                return Items[0].Quantity;
            }
        }
        "#,
    )
    .expect("reports input should be written");

    let output = Command::new(env!("CARGO_BIN_EXE_cadar"))
        .arg("--write")
        .arg("--split-units")
        .arg("--out-dir")
        .arg(&out_dir)
        .arg(&inventory_path)
        .arg(&reports_path)
        .output()
        .expect("cli should run");

    assert!(
        output.status.success(),
        "cli failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        fs::read_to_string(out_dir.join("reports.ads")).expect("reports spec should exist"),
        "with Inventory;\n\npackage Reports is\n   function First_Quantity(Items : Inventory.Item_Array) return Integer;\nend Reports;"
    );
    assert_eq!(
        fs::read_to_string(out_dir.join("reports.adb")).expect("reports body should exist"),
        "with Inventory;\n\npackage body Reports is\n   function First_Quantity(Items : Inventory.Item_Array) return Integer is\n   begin\n      return Items(0).Quantity;\n   end First_Quantity;\nend Reports;"
    );

    fs::remove_dir_all(root).expect("temp dir should be removed");
}

#[test]
fn maps_transpile_errors_to_the_correct_input_file() {
    let root = temp_test_dir("multi-file-error");
    let helper_path = root.join("helper.cada");
    let broken_path = root.join("broken.cada");

    fs::write(
        &helper_path,
        r#"
        fn Helper() {
        }
        "#,
    )
    .expect("helper input should be written");
    fs::write(
        &broken_path,
        "fn Main() {\n    Integer Count = 1;\n    Count + 1;\n}\n",
    )
    .expect("broken input should be written");

    let output = Command::new(env!("CARGO_BIN_EXE_cadar"))
        .arg(&helper_path)
        .arg(&broken_path)
        .output()
        .expect("cli should run");

    assert!(
        !output.status.success(),
        "cli unexpectedly succeeded: {}",
        String::from_utf8_lossy(&output.stdout)
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains(&format!("--> {}:3:5", broken_path.display())),
        "unexpected stderr: {stderr}"
    );
    assert!(
        stderr.contains("3 |     Count + 1;"),
        "unexpected stderr: {stderr}"
    );

    fs::remove_dir_all(root).expect("temp dir should be removed");
}

#[test]
fn multi_file_use_visibility_is_file_local() {
    let root = temp_test_dir("multi-file-use-scope");
    let globals_path = root.join("globals.cada");
    let main_path = root.join("main.cada");
    let broken_path = root.join("broken.cada");

    fs::write(
        &globals_path,
        r#"
        package Globals {
            Integer Counter = 1;
        }
        "#,
    )
    .expect("globals input should be written");
    fs::write(
        &main_path,
        r#"
        use Globals;

        fn Main() {
            Integer Value = Counter;
        }
        "#,
    )
    .expect("main input should be written");
    fs::write(
        &broken_path,
        r#"
        fn Broken() {
            Integer Value = Counter;
        }
        "#,
    )
    .expect("broken input should be written");

    let output = Command::new(env!("CARGO_BIN_EXE_cadar"))
        .arg(&globals_path)
        .arg(&main_path)
        .arg(&broken_path)
        .output()
        .expect("cli should run");

    assert!(
        !output.status.success(),
        "cli unexpectedly succeeded: {}",
        String::from_utf8_lossy(&output.stdout)
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("undefined identifier `Counter`"),
        "unexpected stderr: {stderr}"
    );
    assert!(
        stderr.contains(&format!("--> {}:3:", broken_path.display())),
        "unexpected stderr: {stderr}"
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
        fs::read_to_string(out_dir.join("cadar_shapes_support.ads"))
            .expect("support package spec should exist"),
        "package Cadar_shapes_Support is\n   type Point is record\n      X : Integer;\n      Y : Integer;\n   end record;\nend Cadar_shapes_Support;"
    );
    assert_eq!(
        fs::read_to_string(out_dir.join("main.ads")).expect("main spec should exist"),
        "with Cadar_shapes_Support;\nuse Cadar_shapes_Support;\n\nprocedure Main;"
    );
    assert_eq!(
        fs::read_to_string(out_dir.join("main.adb")).expect("main body should exist"),
        "with Cadar_shapes_Support;\nuse Cadar_shapes_Support;\n\nprocedure Main is\nbegin\n   null;\nend Main;"
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
