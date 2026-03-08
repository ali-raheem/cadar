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
        "06_arrays",
        "07_record_aggregates",
        "08_named_args_and_defaults",
        "09_asserts",
        "10_loop_annotations",
        "11_dataflow_contracts",
        "12_package_state",
        "13_private_package_helpers",
        "14_nested_block_locals",
        "15_float_and_character_literals",
        "16_loop_control",
        "17_arrays_of_records",
        "18_matrix_trace",
        "19_inventory_report",
        "20_stateful_contracts",
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
            if ((true || false) && !false) {
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
fn gnat_compiles_use_without_import_program() {
    if !gnatmake_available() {
        return;
    }

    let root = temp_test_dir("gnat-use-only");
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

    run_cadar_split(&input_path, &out_dir);
    run_gnatmake(&out_dir, "main.adb");
    let output = run_binary(&out_dir, "main");

    assert_eq!(String::from_utf8_lossy(&output.stdout), "Hello\n");

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

#[test]
fn gnat_compiles_split_top_level_subprogram_dependencies() {
    if !gnatmake_available() {
        return;
    }

    let root = temp_test_dir("gnat-top-level-deps");
    let input_path = root.join("program.cada");
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

    run_cadar_split(&input_path, &out_dir);
    run_gnatmake(&out_dir, "main.adb");
    let output = run_binary(&out_dir, "main");

    assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "3");

    fs::remove_dir_all(root).expect("temp dir should be removed");
}

#[test]
fn gnat_compiles_single_source_top_level_subprogram_chain_with_imports() {
    if !gnatmake_available() {
        return;
    }

    let root = temp_test_dir("gnat-top-level-chain");
    let input_path = root.join("program.cada");
    let out_dir = root.join("out");

    fs::write(
        &input_path,
        r#"
        import Text_IO;
        use Text_IO;
        import Add;
        import Show;

        fn Add(Integer A, Integer B) -> Integer {
            return A + B;
        }

        fn Show(Integer Value) {
            Put_Line(Integer.image(Value));
        }

        fn Main() {
            Show(Add(2, 3));
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

#[test]
fn gnat_compiles_multi_file_program() {
    if !gnatmake_available() {
        return;
    }

    let root = temp_test_dir("gnat-multi-file");
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

    run_cadar_split_many(&[&adjust_path, &main_path], &out_dir);
    run_gnatmake(&out_dir, "main.adb");
    let output = run_binary(&out_dir, "main");

    assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "3");

    fs::remove_dir_all(root).expect("temp dir should be removed");
}

#[test]
fn gnat_compiles_multi_file_package_program_with_import() {
    if !gnatmake_available() {
        return;
    }

    let root = temp_test_dir("gnat-package-dependency");
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

    run_cadar_split_many(&[&math_path, &main_path], &out_dir);
    run_gnatmake(&out_dir, "main.adb");
    let output = run_binary(&out_dir, "main");

    assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "5");

    fs::remove_dir_all(root).expect("temp dir should be removed");
}

#[test]
fn gnat_compiles_multi_file_inventory_reporting_program() {
    if !gnatmake_available() {
        return;
    }

    let root = temp_test_dir("gnat-inventory-report");
    let inventory_path = root.join("inventory.cada");
    let reports_path = root.join("reports.cada");
    let main_path = root.join("main.cada");
    let out_dir = root.join("out");

    fs::write(
        &inventory_path,
        r#"
        package Inventory {
            type Item = record {
                Integer Quantity;
                Integer Price;
            };

            type Item_Array = [0..2] Item;

            fn Total_Value(Item_Array Items) -> Integer;
            fn Restock(Item Value, Integer Extra = 1) -> Item;
        }

        package body Inventory {
            fn Total_Value(Item_Array Items) -> Integer {
                Integer Total = 0;
                for (Integer I in 0..2)
                    invariant(Total >= 0)
                {
                    Total = Total + Items[I].Quantity * Items[I].Price;
                }
                return Total;
            }

            fn Restock(Item Value, Integer Extra = 1) -> Item {
                return Item {
                    Quantity = Value.Quantity + Extra,
                    Price = Value.Price
                };
            }
        }
        "#,
    )
    .expect("inventory input should be written");
    fs::write(
        &reports_path,
        r#"
        import Text_IO;
        use Text_IO;
        import Inventory;

        package Reports {
            fn Print_Total(Inventory.Item_Array Items);
            fn Count_Low_Stock(Inventory.Item_Array Items, Integer Limit = 3) -> Integer;
        }

        package body Reports {
            fn Print_Total(Inventory.Item_Array Items) {
                Put_Line(Integer.image(Inventory.Total_Value(Items)));
            }

            fn Count_Low_Stock(Inventory.Item_Array Items, Integer Limit = 3) -> Integer {
                Integer Count = 0;
                for (Integer I in 0..2)
                    invariant(Count >= 0)
                {
                    if (Items[I].Quantity < Limit) {
                        Count = Count + 1;
                    }
                }
                return Count;
            }
        }
        "#,
    )
    .expect("reports input should be written");
    fs::write(
        &main_path,
        r#"
        import Text_IO;
        use Text_IO;
        import Inventory;
        import Reports;

        fn Main() {
            Inventory.Item_Array Items = [
                Inventory.Item { Quantity = 2, Price = 5 },
                Inventory.Item { Quantity = 1, Price = 7 },
                Inventory.Item { Quantity = 4, Price = 3 }
            ];
            Inventory.Item Restocked = Inventory.Restock(Items[1], Extra = 4);

            Reports.Print_Total(Items);
            Put_Line(Integer.image(Reports.Count_Low_Stock(Items)));
            Put_Line(Integer.image(Restocked.Quantity));
        }
        "#,
    )
    .expect("main input should be written");

    run_cadar_split_many(&[&inventory_path, &reports_path, &main_path], &out_dir);
    run_gnatmake(&out_dir, "main.adb");
    let output = run_binary(&out_dir, "main");

    assert_eq!(String::from_utf8_lossy(&output.stdout), " 29\n 2\n 5\n");

    fs::remove_dir_all(root).expect("temp dir should be removed");
}

#[test]
fn gnat_compiles_multi_file_stateful_contract_program() {
    if !gnatmake_available() {
        return;
    }

    let root = temp_test_dir("gnat-stateful-contracts");
    let warehouse_path = root.join("warehouse.cada");
    let reports_path = root.join("reports.cada");
    let main_path = root.join("main.cada");
    let out_dir = root.join("out");

    fs::write(
        &warehouse_path,
        r#"
        package Warehouse {
            type Item = record {
                Integer Quantity;
                Integer Limit;
            };

            Integer Restocked_Count = 0;

            fn Needs_Restock(Item Value) -> Boolean
                ensures(result or else Value.Quantity >= Value.Limit);

            fn Restock(Item Value, Integer Extra = 1; Item Updated)
                requires(Extra > 0)
                global(in_out => Restocked_Count)
                depends(Updated => [Value, Extra], Restocked_Count => [Restocked_Count, Extra])
                ensures(Updated.Quantity >= Value.Quantity);
        }

        package body Warehouse {
            fn Needs_Restock(Item Value) -> Boolean {
                return Value.Quantity < Value.Limit;
            }

            fn Restock(Item Value, Integer Extra = 1; Item Updated) {
                Updated = Value;
                if (Needs_Restock(Value)) {
                    Updated.Quantity = Updated.Quantity + Extra;
                    Restocked_Count = Restocked_Count + 1;
                }
            }
        }
        "#,
    )
    .expect("warehouse input should be written");
    fs::write(
        &reports_path,
        r#"
        import Text_IO;
        use Text_IO;
        import Warehouse;

        package Reports {
            fn Show_Item(Warehouse.Item Value);
            fn Show_Count();
        }

        package body Reports {
            fn Show_Item(Warehouse.Item Value) {
                Put_Line(Integer.image(Value.Quantity));
            }

            fn Show_Count() {
                Put_Line(Integer.image(Warehouse.Restocked_Count));
            }
        }
        "#,
    )
    .expect("reports input should be written");
    fs::write(
        &main_path,
        r#"
        import Text_IO;
        use Text_IO;
        import Reports;
        import Warehouse;

        fn Main() {
            Warehouse.Item First = Warehouse.Item { Quantity = 1, Limit = 3 };
            Warehouse.Item Second = Warehouse.Item { Quantity = 0, Limit = 1 };
            Warehouse.Item Updated_First;
            Warehouse.Item Updated_Second;

            Warehouse.Restock(First, Extra = 4, Updated = Updated_First);
            Warehouse.Restock(Second, Updated = Updated_Second);

            Reports.Show_Item(Updated_First);
            Reports.Show_Item(Updated_Second);
            Reports.Show_Count();

            if (!Warehouse.Needs_Restock(Updated_Second)) {
                Put_Line("stable");
            }
        }
        "#,
    )
    .expect("main input should be written");

    run_cadar_split_many(&[&warehouse_path, &reports_path, &main_path], &out_dir);
    run_gnatmake(&out_dir, "main.adb");
    let output = run_binary(&out_dir, "main");

    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        " 5\n 1\n 2\nstable\n"
    );

    fs::remove_dir_all(root).expect("temp dir should be removed");
}

#[test]
fn gnat_compiles_package_body_with_later_private_helper() {
    if !gnatmake_available() {
        return;
    }

    let root = temp_test_dir("gnat-later-private-helper");
    let input_path = root.join("program.cada");
    let out_dir = root.join("out");

    fs::write(
        &input_path,
        r#"
        import Text_IO;
        use Text_IO;
        import Math;

        package Math {
            fn Add(Integer A, Integer B) -> Integer;
        }

        package body Math {
            fn Add(Integer A, Integer B) -> Integer {
                return Clamp(A) + Clamp(B);
            }

            fn Clamp(Integer Value) -> Integer {
                return Value;
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

#[test]
fn gnat_compiles_package_body_with_later_type_and_object_declarations() {
    if !gnatmake_available() {
        return;
    }

    let root = temp_test_dir("gnat-later-package-decls");
    let input_path = root.join("program.cada");
    let out_dir = root.join("out");

    fs::write(
        &input_path,
        r#"
        import Text_IO;
        use Text_IO;
        import Math;

        package Math {
            fn Get() -> Integer;
        }

        package body Math {
            fn Get() -> Integer {
                Hidden Value;
                Value.X = Local_Count;
                return Value.X;
            }

            Integer Local_Count = 7;

            type Hidden = record {
                Integer X;
            };
        }

        fn Main() {
            Put_Line(Integer.image(Math.Get()));
        }
        "#,
    )
    .expect("input should be written");

    run_cadar_split(&input_path, &out_dir);
    run_gnatmake(&out_dir, "main.adb");
    let output = run_binary(&out_dir, "main");

    assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "7");

    fs::remove_dir_all(root).expect("temp dir should be removed");
}

#[test]
fn gnat_compiles_contextually_disambiguated_overloads() {
    if !gnatmake_available() {
        return;
    }

    let root = temp_test_dir("gnat-contextual-overloads");
    let input_path = root.join("program.cada");
    let out_dir = root.join("out");

    fs::write(
        &input_path,
        r#"
        import Text_IO;
        use Text_IO;
        import Tools;
        use Tools;

        package Tools {
            fn Parse(String Text) -> Integer;
            fn Parse(String Text) -> Boolean;
            fn Show(Boolean Ready);
        }

        package body Tools {
            fn Parse(String Text) -> Integer {
                return 41;
            }

            fn Parse(String Text) -> Boolean {
                return true;
            }

            fn Show(Boolean Ready) {
                if (Ready) {
                    Put_Line("ready");
                }
            }
        }

        fn Main() {
            Integer Count = Parse("42");
            Integer Next = Parse("42") + 1;
            Boolean Ready = Parse("ok");

            Show(Parse("ok"));

            if (not Parse("no") or Parse("yes")) {
                Put_Line(Integer.image(Next));
            }

            if (Parse("42") == Count and Ready) {
                Put_Line("match");
            }
        }
        "#,
    )
    .expect("input should be written");

    run_cadar_split(&input_path, &out_dir);
    run_gnatmake(&out_dir, "main.adb");
    let output = run_binary(&out_dir, "main");

    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "ready\n 42\nmatch\n"
    );

    fs::remove_dir_all(root).expect("temp dir should be removed");
}

fn run_cadar_split(input_path: &Path, out_dir: &Path) {
    run_cadar_split_many(&[input_path], out_dir);
}

fn run_cadar_split_many(input_paths: &[&Path], out_dir: &Path) {
    let mut command = Command::new(env!("CARGO_BIN_EXE_cadar"));
    command
        .arg("--write")
        .arg("--split-units")
        .arg("--out-dir")
        .arg(out_dir);
    for input_path in input_paths {
        command.arg(input_path);
    }

    let output = command.output().expect("cadar should run");

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
