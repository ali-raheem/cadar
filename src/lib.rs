mod ast;
mod diagnostic;
mod lexer;
mod lowering;
mod parser;
mod sema;

use ast::Program;

pub use diagnostic::{Diagnostic, IndexedDiagnostic, Position};
pub use lowering::{AdaOutputs, GeneratedFile};

pub fn transpile(source: &str) -> Result<AdaOutputs, Diagnostic> {
    transpile_project(&[SourceInput { source }]).map_err(|error| error.diagnostic)
}

pub fn transpile_files(
    source: &str,
    fallback_stem: &str,
) -> Result<Vec<GeneratedFile>, Diagnostic> {
    transpile_project_files(&[SourceInput { source }], fallback_stem)
        .map_err(|error| error.diagnostic)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SourceInput<'a> {
    pub source: &'a str,
}

pub fn transpile_project(sources: &[SourceInput<'_>]) -> Result<AdaOutputs, IndexedDiagnostic> {
    let programs = parse_sources(sources)?;
    let semantic_info = sema::validate_all(&programs)?;
    let ada_program = lowering::lower_all(programs, &semantic_info)?;
    Ok(lowering::render(&ada_program))
}

pub fn transpile_project_files(
    sources: &[SourceInput<'_>],
    fallback_stem: &str,
) -> Result<Vec<GeneratedFile>, IndexedDiagnostic> {
    let programs = parse_sources(sources)?;
    let semantic_info = sema::validate_all(&programs)?;
    lowering::validate_split_unit_layout(&programs, fallback_stem)?;
    let ada_program = lowering::lower_all(programs, &semantic_info)?;
    Ok(lowering::render_files(&ada_program, fallback_stem))
}

fn parse_sources(sources: &[SourceInput<'_>]) -> Result<Vec<Program>, IndexedDiagnostic> {
    sources
        .iter()
        .enumerate()
        .map(|(source_index, source)| {
            let tokens = lexer::lex(source.source)
                .map_err(|diagnostic| IndexedDiagnostic::new(source_index, diagnostic))?;
            parser::parse(&tokens)
                .map_err(|diagnostic| IndexedDiagnostic::new(source_index, diagnostic))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::{SourceInput, transpile, transpile_project_files};

    #[test]
    fn transpiles_function_definition_to_spec_and_body() {
        let output = transpile(
            r#"
            import Text_IO;
            use Text_IO;

            fn Add(Integer A, Integer B) -> Integer {
                return A + B;
            }
            "#,
        )
        .expect("transpile should succeed");

        assert_eq!(
            output.spec,
            "with Text_IO;\nuse Text_IO;\n\nfunction Add(A : Integer; B : Integer) return Integer;"
        );
        assert_eq!(
            output.body,
            "with Text_IO;\nuse Text_IO;\n\nfunction Add(A : Integer; B : Integer) return Integer is\nbegin\n   return A + B;\nend Add;"
        );
    }

    #[test]
    fn transpiles_use_without_import_as_with_and_use() {
        let output = transpile(
            r#"
            use Text_IO;

            fn Main() {
                Put_Line("Hello");
            }
            "#,
        )
        .expect("transpile should succeed");

        assert_eq!(
            output.spec,
            "with Text_IO;\nuse Text_IO;\n\nprocedure Main;"
        );
        assert_eq!(
            output.body,
            "with Text_IO;\nuse Text_IO;\n\nprocedure Main is\nbegin\n   Put_Line(\"Hello\");\nend Main;"
        );
    }

    #[test]
    fn transpiles_grouped_parameter_modes_and_local_declarations() {
        let output = transpile(
            r#"
            fn UpdateStats(
                Integer Sample;
                Float Mean;
                Integer Count, Integer Sum
            ) {
                const Integer Scale = 1;
                Count = Count + 1;
                Sum = Sum + Sample;
                Mean = Sum / Count;
            }
            "#,
        )
        .expect("transpile should succeed");

        assert_eq!(
            output.spec,
            "procedure UpdateStats(Sample : Integer; Mean : out Float; Count : in out Integer; Sum : in out Integer);"
        );
        assert_eq!(
            output.body,
            "procedure UpdateStats(Sample : Integer; Mean : out Float; Count : in out Integer; Sum : in out Integer) is\n   Scale : constant Integer := 1;\nbegin\n   Count := Count + 1;\n   Sum := Sum + Sample;\n   Mean := Sum / Count;\nend UpdateStats;"
        );
    }

    #[test]
    fn rejects_local_declaration_after_statement() {
        let error = transpile(
            r#"
            fn Main() {
                Do_Work();
                Integer Late = 1;
            }
            "#,
        )
        .expect_err("transpile should fail");

        assert!(
            error
                .message
                .contains("local declarations must appear before statements"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn transpiles_if_else_if_else_chains() {
        let output = transpile(
            r#"
            fn Adjust(Integer X, Integer Y) -> Integer {
                if (X > 0) {
                    return Y + 1;
                } else if (X < 0) {
                    return Y - 1;
                } else {
                    return 0;
                }
            }
            "#,
        )
        .expect("transpile should succeed");

        assert_eq!(
            output.body,
            "function Adjust(X : Integer; Y : Integer) return Integer is\nbegin\n   if X > 0 then\n      return Y + 1;\n   elsif X < 0 then\n      return Y - 1;\n   else\n      return 0;\n   end if;\nend Adjust;"
        );
    }

    #[test]
    fn transpiles_while_loops() {
        let output = transpile(
            r#"
            fn CountUp(Integer X) -> Integer {
                Integer Value = X;
                while (Value < 10) {
                    Value = Value + 1;
                }
                return Value;
            }
            "#,
        )
        .expect("transpile should succeed");

        assert_eq!(
            output.body,
            "function CountUp(X : Integer) return Integer is\n   Value : Integer := X;\nbegin\n   while Value < 10 loop\n      Value := Value + 1;\n   end loop;\n   return Value;\nend CountUp;"
        );
    }

    #[test]
    fn transpiles_procedure_return_break_and_continue() {
        let output = transpile(
            r#"
            fn Run() {
                Integer Count = 0;
                while (Count < 5) {
                    Count = Count + 1;
                    if (Count == 2) {
                        continue;
                    }
                    if (Count == 4) {
                        break;
                    }
                }
                return;
            }
            "#,
        )
        .expect("transpile should succeed");

        assert_eq!(output.spec, "procedure Run;");
        assert_eq!(
            output.body,
            "procedure Run is\n   Count : Integer := 0;\nbegin\n   while Count < 5 loop\n      Count := Count + 1;\n      if Count = 2 then\n         goto Continue_1;\n      end if;\n      if Count = 4 then\n         exit;\n      end if;\n      <<Continue_1>>\n      null;\n   end loop;\n   return;\nend Run;"
        );
    }

    #[test]
    fn transpiles_package_spec_and_body() {
        let output = transpile(
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
            "#,
        )
        .expect("transpile should succeed");

        assert_eq!(
            output.spec,
            "with Text_IO;\n\npackage Math is\n   function Add(A : Integer; B : Integer) return Integer;\nend Math;"
        );
        assert_eq!(
            output.body,
            "with Text_IO;\n\npackage body Math is\n   function Add(A : Integer; B : Integer) return Integer is\n   begin\n      return A + B;\n   end Add;\nend Math;"
        );
    }

    #[test]
    fn transpiles_private_package_sections() {
        let output = transpile(
            r#"
            package Metrics {
                fn Read() -> Integer;

                private {
                    Integer Hidden_Total = 2;

                    type Hidden = record {
                        Integer Value;
                    };

                    fn Boost(Hidden Data) -> Integer;
                }
            }

            package body Metrics {
                fn Read() -> Integer {
                    Hidden Data;
                    Data.Value = Hidden_Total;
                    return Boost(Data);
                }

                fn Boost(Hidden Data) -> Integer {
                    Hidden_Total = Hidden_Total + 1;
                    return Data.Value + Hidden_Total;
                }
            }
            "#,
        )
        .expect("transpile should succeed");

        assert_eq!(
            output.spec,
            "package Metrics is\n   function Read return Integer;\n   private\n   Hidden_Total : Integer := 2;\n   type Hidden is record\n      Value : Integer;\n   end record;\n   function Boost(Data : Hidden) return Integer;\nend Metrics;"
        );
        assert_eq!(
            output.body,
            "package body Metrics is\n   function Read return Integer is\n      Data : Hidden;\n   begin\n      Data.Value := Hidden_Total;\n      return Boost(Data);\n   end Read;\n\n   function Boost(Data : Hidden) return Integer is\n   begin\n      Hidden_Total := Hidden_Total + 1;\n      return Data.Value + Hidden_Total;\n   end Boost;\nend Metrics;"
        );
    }

    #[test]
    fn rejects_package_spec_with_subprogram_body() {
        let error = transpile(
            r#"
            package Math {
                fn Add(Integer A, Integer B) -> Integer {
                    return A + B;
                }
            }
            "#,
        )
        .expect_err("transpile should fail");

        assert!(
            error
                .message
                .contains("package specifications cannot contain subprogram bodies"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn rejects_public_package_signature_using_private_spec_type() {
        let error = transpile(
            r#"
            package Metrics {
                fn Read(Hidden Data) -> Integer;

                private {
                    type Hidden = record {
                        Integer Value;
                    };
                }
            }
            "#,
        )
        .expect_err("transpile should fail");

        assert!(
            error
                .message
                .contains("type `Hidden` is not visible in parameter type"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn rejects_external_use_of_private_spec_type() {
        let error = transpile(
            r#"
            import Metrics;

            package Metrics {
                private {
                    type Hidden = record {
                        Integer Value;
                    };
                }
            }

            fn Main() {
                Metrics.Hidden Value;
            }
            "#,
        )
        .expect_err("transpile should fail");

        assert!(
            error
                .message
                .contains("type `Metrics.Hidden` is not visible in local initializer")
                || error
                    .message
                    .contains("type `Metrics.Hidden` is not visible in object type"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn transpiles_for_loops() {
        let output = transpile(
            r#"
            fn Count() {
                for (Integer I in 1..10) {
                    Put_Line(I);
                }
            }
            "#,
        )
        .expect("transpile should succeed");

        assert_eq!(output.spec, "procedure Count;");
        assert_eq!(
            output.body,
            "procedure Count is\nbegin\n   for I in 1 .. 10 loop\n      Put_Line(I);\n   end loop;\nend Count;"
        );
    }

    #[test]
    fn transpiles_contracts_to_ada_aspects() {
        let output = transpile(
            r#"
            fn Divide(Integer A, Integer B) -> Integer
                requires(B != 0)
                requires(A >= 0)
                ensures(result * B <= A) {
                return A / B;
            }
            "#,
        )
        .expect("transpile should succeed");

        assert_eq!(
            output.spec,
            "function Divide(A : Integer; B : Integer) return Integer\n   with Pre => B /= 0 and then A >= 0,\n        Post => Divide'Result * B <= A;"
        );
        assert_eq!(
            output.body,
            "function Divide(A : Integer; B : Integer) return Integer is\nbegin\n   return A / B;\nend Divide;"
        );
    }

    #[test]
    fn transpiles_dataflow_contracts_to_ada_aspects() {
        let output = transpile(
            r#"
            fn Add(Integer A, Integer B) -> Integer
                global(null)
                depends(result => [A, B]) {
                return A + B;
            }

            fn Log(Integer Value)
                global(null)
                depends(null => Value) {
                Put_Line(Value);
            }
            "#,
        )
        .expect("transpile should succeed");

        assert_eq!(
            output.spec,
            "function Add(A : Integer; B : Integer) return Integer\n   with Global => null,\n        Depends => (Add'Result => (A, B));\n\nprocedure Log(Value : Integer)\n   with Global => null,\n        Depends => (null => Value);"
        );
        assert_eq!(
            output.body,
            "function Add(A : Integer; B : Integer) return Integer is\nbegin\n   return A + B;\nend Add;\n\nprocedure Log(Value : Integer) is\nbegin\n   Put_Line(Value);\nend Log;"
        );
    }

    #[test]
    fn keeps_declared_dataflow_contracts_when_definition_omits_them() {
        let output = transpile(
            r#"
            fn Add(Integer A, Integer B) -> Integer
                global(null)
                depends(result => [A, B]);

            fn Add(Integer A, Integer B) -> Integer {
                return A + B;
            }
            "#,
        )
        .expect("transpile should succeed");

        assert_eq!(
            output.spec,
            "function Add(A : Integer; B : Integer) return Integer\n   with Global => null,\n        Depends => (Add'Result => (A, B));"
        );
        assert_eq!(
            output.body,
            "function Add(A : Integer; B : Integer) return Integer is\nbegin\n   return A + B;\nend Add;"
        );
    }

    #[test]
    fn transpiles_boolean_operators_and_literals() {
        let output = transpile(
            r#"
            fn Should_Run(Boolean Ready, Boolean Failed) -> Boolean {
                if ((Ready or false) and then not Failed) {
                    return true;
                } else {
                    return false;
                }
            }
            "#,
        )
        .expect("transpile should succeed");

        assert_eq!(
            output.spec,
            "function Should_Run(Ready : Boolean; Failed : Boolean) return Boolean;"
        );
        assert_eq!(
            output.body,
            "function Should_Run(Ready : Boolean; Failed : Boolean) return Boolean is\nbegin\n   if (Ready or False) and then not Failed then\n      return True;\n   else\n      return False;\n   end if;\nend Should_Run;"
        );
    }

    #[test]
    fn transpiles_c_style_boolean_operator_aliases() {
        let output = transpile(
            r#"
            fn Matches(Integer Value, Boolean Failed) -> Boolean {
                return !(Value != 40) && !Failed || false;
            }
            "#,
        )
        .expect("transpile should succeed");

        assert_eq!(
            output.spec,
            "function Matches(Value : Integer; Failed : Boolean) return Boolean;"
        );
        assert_eq!(
            output.body,
            "function Matches(Value : Integer; Failed : Boolean) return Boolean is\nbegin\n   return not (Value /= 40) and then not Failed or else False;\nend Matches;"
        );
    }

    #[test]
    fn transpiles_short_circuit_contract_expressions() {
        let output = transpile(
            r#"
            fn Continue(Boolean Ready, Boolean Failed) -> Boolean
                requires(Ready and then not Failed)
                ensures(result or else Failed) {
                return Ready or Failed;
            }
            "#,
        )
        .expect("transpile should succeed");

        assert_eq!(
            output.spec,
            "function Continue(Ready : Boolean; Failed : Boolean) return Boolean\n   with Pre => Ready and then not Failed,\n        Post => Continue'Result or else Failed;"
        );
        assert_eq!(
            output.body,
            "function Continue(Ready : Boolean; Failed : Boolean) return Boolean is\nbegin\n   return Ready or Failed;\nend Continue;"
        );
    }

    #[test]
    fn transpiles_case_and_null_statements() {
        let output = transpile(
            r#"
            fn Describe(Integer Value) {
                case (Value) {
                    when 0 => {
                        Put_Line("zero");
                    }
                    when 1, 2 => {
                        null;
                    }
                    else => {
                        Put_Line("many");
                    }
                }
            }
            "#,
        )
        .expect("transpile should succeed");

        assert_eq!(output.spec, "procedure Describe(Value : Integer);");
        assert_eq!(
            output.body,
            "procedure Describe(Value : Integer) is\nbegin\n   case Value is\n      when 0 =>\n         Put_Line(\"zero\");\n      when 1 | 2 =>\n         null;\n      when others =>\n         Put_Line(\"many\");\n   end case;\nend Describe;"
        );
    }

    #[test]
    fn accepts_exhaustive_enum_case_returns_without_else() {
        let output = transpile(
            r#"
            enum Axis { X, Y }

            fn Axis_Name(Axis Selected) -> String {
                case (Selected) {
                    when X => {
                        return "X axis";
                    }
                    when Y => {
                        return "Y axis";
                    }
                }
            }
            "#,
        )
        .expect("transpile should succeed");

        assert_eq!(
            output.body,
            "function Axis_Name(Selected : Axis) return String is\nbegin\n   case Selected is\n      when X =>\n         return \"X axis\";\n      when Y =>\n         return \"Y axis\";\n   end case;\nend Axis_Name;"
        );
    }

    #[test]
    fn transpiles_assert_statements() {
        let output = transpile(
            r#"
            fn Check(Integer Value) {
                assert(Value > 0);
            }
            "#,
        )
        .expect("transpile should succeed");

        assert_eq!(output.spec, "procedure Check(Value : Integer);");
        assert_eq!(
            output.body,
            "procedure Check(Value : Integer) is\nbegin\n   pragma Assert (Value > 0);\nend Check;"
        );
    }

    #[test]
    fn transpiles_loop_invariants_and_variants() {
        let output = transpile(
            r#"
            fn Count() {
                Integer Value = 0;

                while (Value < 3)
                    invariant(Value >= 0)
                    invariant(Value <= 3)
                    increases(Value)
                {
                    Value = Value + 1;
                }

                for (Integer I in 1..2)
                    invariant(I >= 1)
                    decreases(2 - I)
                {
                    Put_Line(I);
                }
            }
            "#,
        )
        .expect("transpile should succeed");

        assert_eq!(output.spec, "procedure Count;");
        assert_eq!(
            output.body,
            "procedure Count is\n   Value : Integer := 0;\nbegin\n   while Value < 3 loop\n      pragma Loop_Invariant (Value >= 0);\n      pragma Loop_Invariant (Value <= 3);\n      pragma Loop_Variant (Increases => Value);\n      Value := Value + 1;\n   end loop;\n   for I in 1 .. 2 loop\n      pragma Loop_Invariant (I >= 1);\n      pragma Loop_Variant (Decreases => 2 - I);\n      Put_Line(I);\n   end loop;\nend Count;"
        );
    }

    #[test]
    fn rejects_duplicate_case_else_arms() {
        let error = transpile(
            r#"
            fn Main() {
                case (1) {
                    else => {
                        null;
                    }
                    else => {
                        null;
                    }
                }
            }
            "#,
        )
        .expect_err("transpile should fail");

        assert!(
            error
                .message
                .contains("case statement cannot contain multiple `else` arms"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn rejects_non_boolean_loop_invariant() {
        let error = transpile(
            r#"
            fn Main(Integer Count) {
                while (Count < 3)
                    invariant(Count + 1)
                {
                    Count = Count + 1;
                }
            }
            "#,
        )
        .expect_err("transpile should fail");

        assert!(
            error
                .message
                .contains("loop invariant must be Boolean, found `Integer`"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn rejects_non_numeric_loop_variant() {
        let error = transpile(
            r#"
            fn Main(Boolean Ready) {
                while (true)
                    decreases(Ready)
                {
                    null;
                }
            }
            "#,
        )
        .expect_err("transpile should fail");

        assert!(
            error
                .message
                .contains("loop variant must be numeric, found `Boolean`"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn rejects_non_boolean_assertion() {
        let error = transpile(
            r#"
            fn Main(Integer Count) {
                assert(Count + 1);
            }
            "#,
        )
        .expect_err("transpile should fail");

        assert!(
            error
                .message
                .contains("assertion must be Boolean, found `Integer`"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn rejects_duplicate_parameter_names() {
        let error = transpile(
            r#"
            fn Add(Integer Value, Integer Value) -> Integer {
                return Value;
            }
            "#,
        )
        .expect_err("transpile should fail");

        assert!(
            error.message.contains("duplicate parameter `Value`"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn rejects_duplicate_local_declarations() {
        let error = transpile(
            r#"
            fn Main() {
                Integer Count = 1;
                Integer Count = 2;
            }
            "#,
        )
        .expect_err("transpile should fail");

        assert!(
            error
                .message
                .contains("duplicate local declaration `Count`"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn rejects_case_distinct_parameter_names() {
        let error = transpile(
            r#"
            fn Add(Integer Value, Integer value) -> Integer {
                return Value;
            }
            "#,
        )
        .expect_err("transpile should fail");

        assert!(
            error.message.contains(
                "parameter `value` differs only by case from `Value`; Ada identifiers are case-insensitive"
            ),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn rejects_case_distinct_local_declarations() {
        let error = transpile(
            r#"
            fn Main() {
                Integer Count = 1;
                Integer count = 2;
            }
            "#,
        )
        .expect_err("transpile should fail");

        assert!(
            error.message.contains(
                "local declaration `count` differs only by case from `Count`; Ada identifiers are case-insensitive"
            ),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn rejects_case_distinct_top_level_package_names() {
        let error = transpile(
            r#"
            package Math {
            }

            package math {
            }
            "#,
        )
        .expect_err("transpile should fail");

        assert!(
            error.message.contains(
                "top-level package `math` differs only by case from `Math`; Ada identifiers are case-insensitive"
            ),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn rejects_case_distinct_top_level_subprogram_names() {
        let error = transpile(
            r#"
            fn Parse(Integer Value) -> Integer {
                return Value;
            }

            fn parse(Boolean Value) -> Boolean {
                return Value;
            }
            "#,
        )
        .expect_err("transpile should fail");

        assert!(
            error.message.contains(
                "top-level subprogram `parse` differs only by case from `Parse`; Ada identifiers are case-insensitive"
            ) || error.message.contains(
                "subprogram `parse` differs only by case from `Parse`; Ada identifiers are case-insensitive"
            ),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn rejects_ada_reserved_word_subprogram_name() {
        let error = transpile(
            r#"
            fn Record() {
                null;
            }
            "#,
        )
        .expect_err("transpile should fail");

        assert!(
            error.message.contains(
                "top-level subprogram `Record` uses Ada reserved word `record` and cannot be used as an identifier"
            ) || error.message.contains(
                "subprogram `Record` uses Ada reserved word `record` and cannot be used as an identifier"
            ),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn rejects_ada_reserved_word_parameter_name() {
        let error = transpile(
            r#"
            fn Add(Integer end) -> Integer {
                return end;
            }
            "#,
        )
        .expect_err("transpile should fail");

        assert!(
            error.message.contains(
                "parameter `end` uses Ada reserved word `end` and cannot be used as an identifier"
            ),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn rejects_ada_reserved_word_record_field_name() {
        let error = transpile(
            r#"
            type Point = record {
                Integer Record;
            };
            "#,
        )
        .expect_err("transpile should fail");

        assert!(
            error.message.contains(
                "field of record `Point` `Record` uses Ada reserved word `record` and cannot be used as an identifier"
            ),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn rejects_ada_reserved_word_package_object_name() {
        let error = transpile(
            r#"
            package State {
                Integer task = 0;
            }
            "#,
        )
        .expect_err("transpile should fail");

        assert!(
            error.message.contains(
                "object in package `State` `task` uses Ada reserved word `task` and cannot be used as an identifier"
            ),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn rejects_call_arity_mismatch_for_known_subprogram() {
        let error = transpile(
            r#"
            import Add;

            fn Add(Integer A, Integer B) -> Integer {
                return A + B;
            }

            fn Main() {
                Add(1);
            }
            "#,
        )
        .expect_err("transpile should fail");

        assert!(
            error.message.contains("`Add` does not accept 1 argument"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn rejects_calling_local_value_as_subprogram() {
        let error = transpile(
            r#"
            fn Main() {
                Integer Count = 1;
                Count();
            }
            "#,
        )
        .expect_err("transpile should fail");

        assert!(
            error.message.contains("`Count` is not callable"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn rejects_undefined_identifier_in_expression() {
        let error = transpile(
            r#"
            fn Main() {
                Integer Count = Missing + 1;
            }
            "#,
        )
        .expect_err("transpile should fail");

        assert!(
            error.message.contains("undefined identifier `Missing`"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn rejects_undefined_assignment_target() {
        let error = transpile(
            r#"
            fn Main() {
                Missing = 1;
            }
            "#,
        )
        .expect_err("transpile should fail");

        assert!(
            error
                .message
                .contains("undefined assignment target `Missing`"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn rejects_unimported_package_qualified_call() {
        let error = transpile(
            r#"
            package Math {
                fn Add(Integer A, Integer B) -> Integer;
            }

            package body Math {
                fn Add(Integer A, Integer B) -> Integer {
                    return A + B;
                }
            }

            fn Main() -> Integer {
                return Math.Add(2, 3);
            }
            "#,
        )
        .expect_err("transpile should fail");

        assert!(
            error
                .message
                .contains("package `Math` is not visible; add `import Math;` or `use Math;`"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn rejects_unimported_top_level_subprogram_call() {
        let error = transpile(
            r#"
            fn Adjust(Integer Value) -> Integer {
                return Value + 1;
            }

            fn Main() -> Integer {
                return Adjust(2);
            }
            "#,
        )
        .expect_err("transpile should fail");

        assert!(
            error
                .message
                .contains("top-level subprogram `Adjust` is not visible; add `import Adjust;`"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn rejects_use_of_top_level_subprogram() {
        let error = transpile(
            r#"
            use Adjust;

            fn Adjust(Integer Value) -> Integer {
                return Value + 1;
            }
            "#,
        )
        .expect_err("transpile should fail");

        assert!(
            error.message.contains(
                "`use Adjust` is not valid because `Adjust` is a top-level subprogram; use `import Adjust;` and call it explicitly"
            ),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn rejects_import_of_top_level_type() {
        let error = transpile(
            r#"
            import Buffer;

            type Buffer = [0..3] Integer;
            "#,
        )
        .expect_err("transpile should fail");

        assert!(
            error.message.contains(
                "`import Buffer` is not valid because `Buffer` is a top-level type, not a library unit"
            ),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn rejects_import_of_known_package_member() {
        let error = transpile(
            r#"
            import Math.Add;

            package Math {
                fn Add(Integer A, Integer B) -> Integer;
            }
            "#,
        )
        .expect_err("transpile should fail");

        assert!(
            error.message.contains(
                "`import Math.Add` is not valid because `Math.Add` names a member of package `Math`; import the package instead"
            ),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn rejects_use_of_known_package_member() {
        let error = transpile(
            r#"
            use Math.Count;

            package Math {
                Integer Count = 0;
            }
            "#,
        )
        .expect_err("transpile should fail");

        assert!(
            error.message.contains(
                "`use Math.Count` is not valid because `Math.Count` names a member of package `Math`; use the package instead"
            ),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn rejects_import_after_top_level_declaration() {
        let error = transpile(
            r#"
            fn Main() {
                null;
            }

            import Text_IO;
            "#,
        )
        .expect_err("transpile should fail");

        assert!(
            error
                .message
                .contains("`import` clauses must appear before top-level declarations"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn rejects_use_after_top_level_declaration() {
        let error = transpile(
            r#"
            fn Main() {
                null;
            }

            use Text_IO;
            "#,
        )
        .expect_err("transpile should fail");

        assert!(
            error
                .message
                .contains("`use` clauses must appear before top-level declarations"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn allows_recursive_top_level_function_without_import() {
        let output = transpile(
            r#"
            fn Factorial(Integer Value) -> Integer {
                if (Value == 0) {
                    return 1;
                }

                return Value * Factorial(Value - 1);
            }
            "#,
        )
        .expect("transpile should succeed");

        assert!(
            output.body.contains("return Value * Factorial(Value - 1);"),
            "unexpected body: {}",
            output.body
        );
    }

    #[test]
    fn rejects_ambiguous_use_visible_package_value_name() {
        let error = transpile(
            r#"
            use Left;
            use Right;

            package Left {
                const Integer Value = 1;
            }

            package Right {
                const Integer Value = 2;
            }

            fn Main() {
                Integer Count = Value;
            }
            "#,
        )
        .expect_err("transpile should fail");

        assert!(
            error
                .message
                .contains("identifier `Value` is ambiguous via `use` of packages `Left`, `Right`"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn rejects_ambiguous_use_visible_type_name() {
        let error = transpile(
            r#"
            use Left;
            use Right;

            package Left {
                type Count = Integer range 0..10;
            }

            package Right {
                type Count = Integer range 0..20;
            }

            fn Main() {
                Count Value = 1;
            }
            "#,
        )
        .expect_err("transpile should fail");

        assert!(
            error
                .message
                .contains("type `Count` is ambiguous via `use` of packages `Left`, `Right`"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn rejects_ambiguous_use_visible_enum_literal() {
        let error = transpile(
            r#"
            use Left;
            use Right;

            package Left {
                enum Color { Red }
            }

            package Right {
                enum Shade { Red }
            }

            fn Main() {
                Left.Color Value = Red;
            }
            "#,
        )
        .expect_err("transpile should fail");

        assert!(
            error.message.contains(
                "enum literal `Red` is ambiguous between types `Left.Color`, `Right.Shade`"
            ),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn rejects_ambiguous_use_visible_subprogram_call() {
        let error = transpile(
            r#"
            use Left;
            use Right;

            package Left {
                fn Tick(Integer Value) -> Integer;
            }

            package Right {
                fn Tick(Integer Value) -> Integer;
            }

            fn Main() {
                Integer Value = Tick(1);
            }
            "#,
        )
        .expect_err("transpile should fail");

        assert!(
            error
                .message
                .contains("call to `Tick` is ambiguous between"),
            "unexpected error: {error}"
        );
        assert!(
            error.message.contains("`Left.Tick(Integer) -> Integer`"),
            "unexpected error: {error}"
        );
        assert!(
            error.message.contains("`Right.Tick(Integer) -> Integer`"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn transpiles_package_body_private_helper_subprograms() {
        let output = transpile(
            r#"
            package Math {
                fn Add(Integer A, Integer B) -> Integer;
            }

            package body Math {
                fn Clamp(Integer Value) -> Integer {
                    if (Value < 0) {
                        return 0;
                    }
                    return Value;
                }

                fn Add(Integer A, Integer B) -> Integer {
                    return Clamp(A) + Clamp(B);
                }
            }
            "#,
        )
        .expect("transpile should succeed");

        assert!(
            output
                .spec
                .contains("function Add(A : Integer; B : Integer) return Integer;"),
            "unexpected spec: {}",
            output.spec
        );
        assert!(
            !output.spec.contains("Clamp"),
            "unexpected spec: {}",
            output.spec
        );
        assert!(
            output
                .body
                .contains("function Clamp(Value : Integer) return Integer is"),
            "unexpected body: {}",
            output.body
        );
        assert!(
            output.body.contains("return Clamp(A) + Clamp(B);"),
            "unexpected body: {}",
            output.body
        );
    }

    #[test]
    fn renders_forward_declaration_for_later_private_package_helper() {
        let output = transpile(
            r#"
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
            "#,
        )
        .expect("transpile should succeed");

        assert!(
            output
                .body
                .contains("function Clamp(Value : Integer) return Integer;"),
            "unexpected body: {}",
            output.body
        );
        assert!(
            output
                .body
                .contains("function Add(A : Integer; B : Integer) return Integer is"),
            "unexpected body: {}",
            output.body
        );
    }

    #[test]
    fn hoists_later_package_body_declarations_before_subprogram_bodies() {
        let output = transpile(
            r#"
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
            "#,
        )
        .expect("transpile should succeed");

        assert_eq!(
            output.spec,
            "package Math is\n   function Get return Integer;\nend Math;"
        );
        assert_eq!(
            output.body,
            "package body Math is\n   Local_Count : Integer := 7;\n\n   type Hidden is record\n      X : Integer;\n   end record;\n\n   function Get return Integer is\n      Value : Hidden;\n   begin\n      Value.X := Local_Count;\n      return Value.X;\n   end Get;\nend Math;"
        );
    }

    #[test]
    fn rejects_private_package_helper_call_argument_type_mismatch() {
        let error = transpile(
            r#"
            package Math {
                fn Add(Integer A, Integer B) -> Integer;
            }

            package body Math {
                fn Clamp(Integer Value) -> Integer {
                    return Value;
                }

                fn Add(Integer A, Integer B) -> Integer {
                    return Clamp(true) + B;
                }
            }
            "#,
        )
        .expect_err("transpile should fail");

        assert!(
            error
                .message
                .contains("no matching overload for `Clamp` with argument types (Boolean)"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn rejects_package_body_missing_public_definition() {
        let error = transpile(
            r#"
            package Math {
                fn Add(Integer A, Integer B) -> Integer;
            }

            package body Math {
                fn Clamp(Integer Value) -> Integer {
                    return Value;
                }
            }
            "#,
        )
        .expect_err("transpile should fail");

        assert!(
            error
                .message
                .contains("package body `Math` is missing a definition for `Add`"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn rejects_package_spec_using_body_only_type_in_public_signature() {
        let error = transpile(
            r#"
            package Math {
                fn Make() -> Hidden;
            }

            package body Math {
                type Hidden = record {
                    Integer Value;
                };

                fn Make() -> Hidden {
                    Hidden Result;
                    return Result;
                }
            }
            "#,
        )
        .expect_err("transpile should fail");

        assert!(
            error
                .message
                .contains("type `Hidden` is not visible in subprogram return type"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn rejects_body_only_package_public_signature_using_body_local_type() {
        let error = transpile(
            r#"
            package body Math {
                type Hidden = record {
                    Integer Value;
                };

                fn Make() -> Hidden {
                    Hidden Result;
                    return Result;
                }
            }
            "#,
        )
        .expect_err("transpile should fail");

        assert!(
            error
                .message
                .contains("type `Hidden` is not visible in subprogram return type"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn rejects_result_in_procedure_postcondition() {
        let error = transpile(
            "fn Note(Integer Value)\n    ensures(result == Value) {\n    Put_Line(Value);\n}\n",
        )
        .expect_err("transpile should fail");

        assert!(
            error
                .message
                .contains("`result` is only valid in postconditions of functions"),
            "unexpected error: {error}"
        );
        assert_eq!(error.position.line, 2);
        assert_eq!(error.position.column, 13);
    }

    #[test]
    fn rejects_result_in_procedure_depends_clause() {
        let error = transpile(
            r#"
            fn Note(Integer Value)
                depends(result => Value) {
                Put_Line(Value);
            }
            "#,
        )
        .expect_err("transpile should fail");

        assert!(
            error
                .message
                .contains("`result` is only valid in `depends` clauses of functions"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn rejects_function_depends_clause_without_result_target() {
        let error = transpile(
            r#"
            fn Add(Integer A, Integer B) -> Integer
                depends(null => [A, B]) {
                return A + B;
            }
            "#,
        )
        .expect_err("transpile should fail");

        assert!(
            error
                .message
                .contains("function `depends` clauses must mention `result` exactly once"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn rejects_result_in_global_clause() {
        let error = transpile(
            r#"
            fn Add(Integer A) -> Integer
                global(input => result) {
                return A;
            }
            "#,
        )
        .expect_err("transpile should fail");

        assert!(
            error
                .message
                .contains("`result` is not valid in `global` clauses"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn rejects_definition_that_changes_dataflow_contracts() {
        let error = transpile(
            r#"
            fn Add(Integer A, Integer B) -> Integer
                global(null)
                depends(result => [A, B]);

            fn Add(Integer A, Integer B) -> Integer
                global(null)
                depends(result => A) {
                return A + B;
            }
            "#,
        )
        .expect_err("transpile should fail");

        assert!(
            error.message.contains(
                "definition of `Add` must use the same dataflow contracts as its declaration"
            ),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn rejects_standalone_non_call_statement_at_statement_position() {
        let error = transpile("fn Main() {\n    Integer Count = 1;\n    Count + 1;\n}\n")
            .expect_err("transpile should fail");

        assert!(
            error
                .message
                .contains("only call expressions are allowed as standalone statements"),
            "unexpected error: {error}"
        );
        assert_eq!(error.position.line, 3);
        assert_eq!(error.position.column, 5);
    }

    #[test]
    fn transpiles_attributes_and_qualified_calls() {
        let output = transpile(
            r#"
            fn Describe(Integer X, Buffer Data) {
                Text_IO.Put_Line(Integer.image(X));
                Print_Length(Data.length);
                Print_Range(Data.range);
            }
            "#,
        )
        .expect("transpile should succeed");

        assert_eq!(
            output.spec,
            "procedure Describe(X : Integer; Data : Buffer);"
        );
        assert_eq!(
            output.body,
            "procedure Describe(X : Integer; Data : Buffer) is\nbegin\n   Text_IO.Put_Line(Integer'Image(X));\n   Print_Length(Data'Length);\n   Print_Range(Data'Range);\nend Describe;"
        );
    }

    #[test]
    fn transpiles_type_declarations() {
        let output = transpile(
            r#"
            type Point = record {
                Integer X;
                Integer Y;
            };

            enum Color {
                Red,
                Green,
                Blue
            }

            type Speed = Integer range 0..300;
            "#,
        )
        .expect("transpile should succeed");

        assert_eq!(
            output.spec,
            "type Point is record\n   X : Integer;\n   Y : Integer;\nend record;\n\ntype Color is (Red, Green, Blue);\n\nsubtype Speed is Integer range 0 .. 300;"
        );
        assert!(output.body.is_empty(), "expected no body output");
    }

    #[test]
    fn transpiles_package_types_and_subprograms() {
        let output = transpile(
            r#"
            package Geometry {
                type Point = record {
                    Integer X;
                    Integer Y;
                };
                enum Axis { X, Y }
                type Speed = Integer range 0..300;
                fn Length(Point P) -> Integer;
            }

            package body Geometry {
                type InternalPoint = record {
                    Integer X;
                    Integer Y;
                };
                fn Length(Point P) -> Integer {
                    return P.X + P.Y;
                }
            }
            "#,
        )
        .expect("transpile should succeed");

        assert_eq!(
            output.spec,
            "package Geometry is\n   type Point is record\n      X : Integer;\n      Y : Integer;\n   end record;\n   type Axis is (X, Y);\n   subtype Speed is Integer range 0 .. 300;\n   function Length(P : Point) return Integer;\nend Geometry;"
        );
        assert_eq!(
            output.body,
            "package body Geometry is\n   type InternalPoint is record\n      X : Integer;\n      Y : Integer;\n   end record;\n\n   function Length(P : Point) return Integer is\n   begin\n      return P.X + P.Y;\n   end Length;\nend Geometry;"
        );
    }

    #[test]
    fn derives_package_spec_from_package_body() {
        let output = transpile(
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
        .expect("transpile should succeed");

        assert_eq!(
            output.spec,
            "package Math is\n   function Add(A : Integer; B : Integer) return Integer;\nend Math;"
        );
        assert_eq!(
            output.body,
            "package body Math is\n   type Hidden is record\n      Value : Integer;\n   end record;\n\n   function Add(A : Integer; B : Integer) return Integer is\n   begin\n      return A + B;\n   end Add;\nend Math;"
        );
    }

    #[test]
    fn rejects_non_boolean_if_condition() {
        let error = transpile(
            r#"
            fn Main(Integer Count) {
                if (Count) {
                    null;
                }
            }
            "#,
        )
        .expect_err("transpile should fail");

        assert!(
            error
                .message
                .contains("if condition must be Boolean, found `Integer`"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn rejects_non_boolean_precondition() {
        let error = transpile(
            r#"
            fn Main(Integer Count)
                requires(Count + 1) {
                null;
            }
            "#,
        )
        .expect_err("transpile should fail");

        assert!(
            error
                .message
                .contains("precondition must be Boolean, found `Integer`"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn rejects_boolean_in_arithmetic_expression() {
        let error = transpile(
            r#"
            fn Main(Boolean Ready) -> Integer {
                return Ready + 1;
            }
            "#,
        )
        .expect_err("transpile should fail");

        assert!(
            error
                .message
                .contains("`+` requires numeric operands, found `Boolean`"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn rejects_assignment_type_mismatch() {
        let error = transpile(
            r#"
            fn Main() {
                Boolean Ready = false;
                Ready = 1;
            }
            "#,
        )
        .expect_err("transpile should fail");

        assert!(
            error
                .message
                .contains("assignment expects `Boolean`, found `Integer`"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn rejects_return_type_mismatch() {
        let error = transpile(
            r#"
            fn Main() -> Boolean {
                return 1;
            }
            "#,
        )
        .expect_err("transpile should fail");

        assert!(
            error
                .message
                .contains("return expression expects `Boolean`, found `Integer`"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn rejects_bare_return_in_function() {
        let error = transpile(
            r#"
            fn Main() -> Integer {
                return;
            }
            "#,
        )
        .expect_err("transpile should fail");

        assert!(
            error.message.contains("functions must return a value"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn rejects_function_that_may_fall_through_without_return() {
        let error = transpile(
            r#"
            fn Main(Boolean Ready) -> Integer {
                if (Ready) {
                    return 1;
                }
            }
            "#,
        )
        .expect_err("transpile should fail");

        assert!(
            error
                .message
                .contains("function `Main` may complete without returning a value"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn rejects_return_value_in_procedure() {
        let error = transpile(
            r#"
            fn Main() {
                return 1;
            }
            "#,
        )
        .expect_err("transpile should fail");

        assert!(
            error.message.contains("procedures cannot return a value"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn rejects_break_outside_loop() {
        let error = transpile(
            r#"
            fn Main() {
                break;
            }
            "#,
        )
        .expect_err("transpile should fail");

        assert!(
            error.message.contains("`break` is only valid inside loops"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn rejects_continue_outside_loop() {
        let error = transpile(
            r#"
            fn Main() {
                continue;
            }
            "#,
        )
        .expect_err("transpile should fail");

        assert!(
            error
                .message
                .contains("`continue` is only valid inside loops"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn rejects_function_call_used_as_statement() {
        let error = transpile(
            r#"
            import Add;

            fn Add(Integer A, Integer B) -> Integer {
                return A + B;
            }

            fn Main() {
                Add(1, 2);
            }
            "#,
        )
        .expect_err("transpile should fail");

        assert!(
            error
                .message
                .contains("function calls cannot be used as standalone statements"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn rejects_procedure_used_as_value() {
        let error = transpile(
            r#"
            import Log;

            fn Log() {
                null;
            }

            fn Main() {
                Integer Value = Log();
            }
            "#,
        )
        .expect_err("transpile should fail");

        assert!(
            error.message.contains("procedures do not produce a value"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn transpiles_array_types_literals_and_indexing() {
        let output = transpile(
            r#"
            type Buffer = [0..3] Integer;

            fn Sum() -> Integer {
                Buffer Data = [1, 2, 3, 4];
                Data[1] = 9;
                return Data[0] + Data[1];
            }
            "#,
        )
        .expect("transpile should succeed");

        assert_eq!(
            output.spec,
            "type Buffer is array (0 .. 3) of Integer;\n\nfunction Sum return Integer;"
        );
        assert_eq!(
            output.body,
            "function Sum return Integer is\n   Data : Buffer := (1, 2, 3, 4);\nbegin\n   Data(1) := 9;\n   return Data(0) + Data(1);\nend Sum;"
        );
    }

    #[test]
    fn rejects_array_literal_length_mismatch() {
        let error = transpile(
            r#"
            type Buffer = [0..3] Integer;

            fn Main() {
                Buffer Data = [1, 2, 3];
            }
            "#,
        )
        .expect_err("transpile should fail");

        assert!(
            error
                .message
                .contains("local initializer expects `Buffer` with 4 elements, found array literal with 3 elements"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn rejects_indexing_non_array_value() {
        let error = transpile(
            r#"
            fn Main() {
                Integer Value = 1;
                Value[0] = 2;
            }
            "#,
        )
        .expect_err("transpile should fail");

        assert!(
            error
                .message
                .contains("indexed expression must be an array, found `Integer`"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn transpiles_record_field_assignment_and_access() {
        let output = transpile(
            r#"
            type Point = record {
                Integer X;
                Integer Y;
            };

            fn Sum() -> Integer {
                Point P;
                P.X = 1;
                P.Y = 2;
                return P.X + P.Y;
            }
            "#,
        )
        .expect("transpile should succeed");

        assert_eq!(
            output.spec,
            "type Point is record\n   X : Integer;\n   Y : Integer;\nend record;\n\nfunction Sum return Integer;"
        );
        assert_eq!(
            output.body,
            "function Sum return Integer is\n   P : Point;\nbegin\n   P.X := 1;\n   P.Y := 2;\n   return P.X + P.Y;\nend Sum;"
        );
    }

    #[test]
    fn rejects_unknown_record_field() {
        let error = transpile(
            r#"
            type Point = record {
                Integer X;
                Integer Y;
            };

            fn Sum(Point P) -> Integer {
                return P.Z;
            }
            "#,
        )
        .expect_err("transpile should fail");

        assert!(
            error.message.contains("type `Point` has no field `Z`"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn rejects_record_field_assignment_type_mismatch() {
        let error = transpile(
            r#"
            type Point = record {
                Integer X;
                Boolean Ready;
            };

            fn Main() {
                Point P;
                P.X = true;
            }
            "#,
        )
        .expect_err("transpile should fail");

        assert!(
            error
                .message
                .contains("assignment expects `Integer`, found `Boolean`"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn rejects_call_argument_type_mismatch_for_known_subprogram() {
        let error = transpile(
            r#"
            import Add;

            fn Add(Integer Value) -> Integer {
                return Value + 1;
            }

            fn Main() {
                Boolean Ready = true;
                Integer Result = Add(Ready);
            }
            "#,
        )
        .expect_err("transpile should fail");

        assert!(
            error
                .message
                .contains("no matching overload for `Add` with argument types (Boolean)"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn rejects_function_overload_used_as_statement_when_args_select_function() {
        let error = transpile(
            r#"
            import Log;

            fn Log(Integer Value) {
                null;
            }

            fn Log(Boolean Enabled) -> Boolean {
                return Enabled;
            }

            fn Main() {
                Log(true);
            }
            "#,
        )
        .expect_err("transpile should fail");

        assert!(
            error
                .message
                .contains("function calls cannot be used as standalone statements"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn resolves_overloaded_call_by_expected_assignment_type() {
        let output = transpile(
            r#"
            import Parse;

            fn Parse(String Text) -> Integer {
                return 1;
            }

            fn Parse(String Text) -> Boolean {
                return true;
            }

            fn Main() {
                Integer Count = Parse("42");
                Boolean Ready = Parse("ok");
            }
            "#,
        )
        .expect("transpile should succeed");

        assert!(
            output
                .body
                .contains("Count : Integer := Integer'(Parse(\"42\"));"),
            "unexpected body: {}",
            output.body
        );
        assert!(
            output
                .body
                .contains("Ready : Boolean := Boolean'(Parse(\"ok\"));"),
            "unexpected body: {}",
            output.body
        );
    }

    #[test]
    fn resolves_overloaded_call_by_expected_parameter_type() {
        let output = transpile(
            r#"
            import Parse;
            import Show;

            fn Parse(String Text) -> Integer {
                return 1;
            }

            fn Parse(String Text) -> Boolean {
                return true;
            }

            fn Show(Boolean Ready) {
                null;
            }

            fn Main() {
                Show(Parse("ok"));
            }
            "#,
        )
        .expect("transpile should succeed");

        assert!(
            output.body.contains("Show(Boolean'(Parse(\"ok\")));"),
            "unexpected body: {}",
            output.body
        );
    }

    #[test]
    fn resolves_overloaded_call_by_boolean_operator_context() {
        let output = transpile(
            r#"
            import Parse;

            fn Parse(String Text) -> Integer {
                return 1;
            }

            fn Parse(String Text) -> Boolean {
                return true;
            }

            fn Main() -> Boolean {
                return not Parse("ok") or Parse("again");
            }
            "#,
        )
        .expect("transpile should succeed");

        assert!(
            output
                .body
                .contains("return not Boolean'(Parse(\"ok\")) or Boolean'(Parse(\"again\"));"),
            "unexpected body: {}",
            output.body
        );
    }

    #[test]
    fn resolves_overloaded_call_by_numeric_operator_context() {
        let output = transpile(
            r#"
            import Parse;

            fn Parse(String Text) -> Integer {
                return 1;
            }

            fn Parse(String Text) -> Boolean {
                return true;
            }

            fn Main() -> Integer {
                return Parse("42") + 1;
            }
            "#,
        )
        .expect("transpile should succeed");

        assert!(
            output.body.contains("return Integer'(Parse(\"42\")) + 1;"),
            "unexpected body: {}",
            output.body
        );
    }

    #[test]
    fn resolves_overloaded_call_by_comparison_context() {
        let output = transpile(
            r#"
            import Parse;

            fn Parse(String Text) -> Integer {
                return 1;
            }

            fn Parse(String Text) -> Boolean {
                return true;
            }

            fn Main() -> Boolean {
                return Parse("42") == 1;
            }
            "#,
        )
        .expect("transpile should succeed");

        assert!(
            output.body.contains("return Integer'(Parse(\"42\")) = 1;"),
            "unexpected body: {}",
            output.body
        );
    }

    #[test]
    fn rejects_non_boolean_overloaded_call_in_boolean_context() {
        let error = transpile(
            r#"
            import Parse;

            fn Parse(String Text) -> Integer {
                return 1;
            }

            fn Parse(String Text) -> String {
                return "bad";
            }

            fn Main() -> Boolean {
                return Parse("x") and true;
            }
            "#,
        )
        .expect_err("transpile should fail");

        assert!(
            error.message.contains(
                "no matching overload for `Parse` with argument types (String) and expected return type `Boolean`"
            ),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn rejects_non_numeric_overloaded_call_in_numeric_context() {
        let error = transpile(
            r#"
            import Parse;

            fn Parse(String Text) -> Boolean {
                return true;
            }

            fn Parse(String Text) -> String {
                return "bad";
            }

            fn Main() -> Integer {
                return Parse("x") + 1;
            }
            "#,
        )
        .expect_err("transpile should fail");

        assert!(
            error.message.contains(
                "no matching overload for `Parse` with argument types (String) in a numeric context"
            ),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn split_unit_files_reject_top_level_overloaded_subprograms() {
        let error = transpile_project_files(
            &[SourceInput {
                source: r#"
                fn Parse(String Text) -> Integer {
                    return 1;
                }

                fn Parse(String Text) -> Boolean {
                    return true;
                }
                "#,
            }],
            "demo",
        )
        .expect_err("split-unit transpile should fail");

        assert_eq!(error.source_index, 0);
        assert!(
            error.diagnostic.message.contains(
                "split-unit output does not support overloaded top-level subprograms like `Parse`"
            ),
            "unexpected error: {}",
            error.diagnostic
        );
    }

    #[test]
    fn split_unit_files_reject_top_level_package_and_subprogram_name_collision() {
        let error = transpile_project_files(
            &[SourceInput {
                source: r#"
                package Math {
                    fn Add(Integer A, Integer B) -> Integer;
                }

                fn Math() {
                    null;
                }
                "#,
            }],
            "demo",
        )
        .expect_err("split-unit transpile should fail");

        assert_eq!(error.source_index, 0);
        assert!(
            error.diagnostic.message.contains(
                "split-unit output would write both top-level package `Math` and top-level subprogram `Math` to `math.ads`"
            ) || error
                .diagnostic
                .message
                .contains(
                    "split-unit output would write both top-level subprogram `Math` and top-level package `Math` to `math.ads`"
                ),
            "unexpected error: {}",
            error.diagnostic
        );
    }

    #[test]
    fn split_unit_files_scope_top_level_subprogram_imports_per_unit() {
        let files = transpile_project_files(
            &[SourceInput {
                source: r#"
                import Add;
                import Show;

                fn Add(Integer A, Integer B) -> Integer {
                    return A + B;
                }

                fn Show(Integer Value) {
                    null;
                }

                fn Main() {
                    Show(Add(2, 3));
                }
                "#,
            }],
            "demo",
        )
        .expect("split-unit transpile should succeed");

        let files_by_name: HashMap<_, _> = files
            .into_iter()
            .map(|file| (file.filename, file.contents))
            .collect();

        assert_eq!(
            files_by_name
                .get("add.ads")
                .expect("add spec should be emitted"),
            "function Add(A : Integer; B : Integer) return Integer;"
        );
        assert_eq!(
            files_by_name
                .get("show.ads")
                .expect("show spec should be emitted"),
            "procedure Show(Value : Integer);"
        );
        assert_eq!(
            files_by_name
                .get("main.adb")
                .expect("main body should be emitted"),
            "with Add;\nwith Show;\n\nprocedure Main is\nbegin\n   Show(Add(2, 3));\nend Main;"
        );
    }

    #[test]
    fn split_unit_files_add_with_clause_for_cross_package_type_signatures() {
        let files = transpile_project_files(
            &[
                SourceInput {
                    source: r#"
                    package Inventory {
                        type Item = record {
                            Integer Quantity;
                            Integer Price;
                        };
                        type Item_Array = [0..1] Item;
                    }
                    "#,
                },
                SourceInput {
                    source: r#"
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
                },
            ],
            "demo",
        )
        .expect("split-unit transpile should succeed");

        let files_by_name: HashMap<_, _> = files
            .into_iter()
            .map(|file| (file.filename, file.contents))
            .collect();

        let reports_spec = files_by_name
            .get("reports.ads")
            .expect("reports spec should be emitted");
        assert!(
            reports_spec.contains("with Inventory;"),
            "unexpected reports spec: {reports_spec}"
        );
        assert!(
            reports_spec
                .contains("function First_Quantity(Items : Inventory.Item_Array) return Integer;"),
            "unexpected reports spec: {reports_spec}"
        );

        let reports_body = files_by_name
            .get("reports.adb")
            .expect("reports body should be emitted");
        assert!(
            reports_body.contains("with Inventory;"),
            "unexpected reports body: {reports_body}"
        );
        assert!(
            reports_body.contains("return Items(0).Quantity;"),
            "unexpected reports body: {reports_body}"
        );
    }

    #[test]
    fn split_unit_files_scope_same_source_package_imports_per_unit() {
        let files = transpile_project_files(
            &[SourceInput {
                source: r#"
                import Inventory;
                import Reports;

                package Inventory {
                    type Item = record {
                        Integer Quantity;
                    };
                    type Item_Array = [0..0] Item;
                }

                package Reports {
                    fn First(Inventory.Item_Array Items) -> Integer;
                }

                package body Reports {
                    fn First(Inventory.Item_Array Items) -> Integer {
                        return Items[0].Quantity;
                    }
                }

                fn Main() -> Integer {
                    Inventory.Item_Array Items = [
                        Inventory.Item { Quantity = 2 }
                    ];
                    return Reports.First(Items);
                }
                "#,
            }],
            "demo",
        )
        .expect("split-unit transpile should succeed");

        let files_by_name: HashMap<_, _> = files
            .into_iter()
            .map(|file| (file.filename, file.contents))
            .collect();

        let inventory_spec = files_by_name
            .get("inventory.ads")
            .expect("inventory spec should be emitted");
        assert!(
            !inventory_spec.contains("with Reports;"),
            "unexpected inventory spec: {inventory_spec}"
        );

        let reports_spec = files_by_name
            .get("reports.ads")
            .expect("reports spec should be emitted");
        assert!(
            reports_spec.contains("with Inventory;"),
            "unexpected reports spec: {reports_spec}"
        );

        let main_body = files_by_name
            .get("main.adb")
            .expect("main body should be emitted");
        assert!(
            main_body.contains("with Inventory;"),
            "unexpected main body: {main_body}"
        );
        assert!(
            main_body.contains("with Reports;"),
            "unexpected main body: {main_body}"
        );
    }

    #[test]
    fn aggregate_output_allows_top_level_overloaded_subprograms() {
        let output = transpile(
            r#"
            fn Parse(String Text) -> Integer {
                return 1;
            }

            fn Parse(String Text) -> Boolean {
                return true;
            }
            "#,
        )
        .expect("aggregate transpile should succeed");

        assert!(
            output
                .spec
                .contains("function Parse(Text : String) return Integer;"),
            "unexpected spec: {}",
            output.spec
        );
        assert!(
            output
                .spec
                .contains("function Parse(Text : String) return Boolean;"),
            "unexpected spec: {}",
            output.spec
        );
        assert!(
            output
                .body
                .contains("function Parse(Text : String) return Integer is"),
            "unexpected body: {}",
            output.body
        );
        assert!(
            output
                .body
                .contains("function Parse(Text : String) return Boolean is"),
            "unexpected body: {}",
            output.body
        );
    }

    #[test]
    fn transpiles_record_aggregates() {
        let output = transpile(
            r#"
            type Point = record {
                Integer X;
                Integer Y;
            };

            fn Origin() -> Point {
                return Point { X = 1, Y = 2 };
            }

            fn Sum() -> Integer {
                Point P = Point { Y = 4, X = 3 };
                return P.X + P.Y;
            }
            "#,
        )
        .expect("transpile should succeed");

        assert_eq!(
            output.spec,
            "type Point is record\n   X : Integer;\n   Y : Integer;\nend record;\n\nfunction Origin return Point;\n\nfunction Sum return Integer;"
        );
        assert_eq!(
            output.body,
            "function Origin return Point is\nbegin\n   return (X => 1, Y => 2);\nend Origin;\n\nfunction Sum return Integer is\n   P : Point := (Y => 4, X => 3);\nbegin\n   return P.X + P.Y;\nend Sum;"
        );
    }

    #[test]
    fn transpiles_arrays_of_records_and_nested_aggregates() {
        let output = transpile(
            r#"
            import Geometry;

            package Geometry {
                type Point = record {
                    Integer X;
                    Integer Y;
                };
                type Point_Array = [0..1] Point;
                type Segment = record {
                    Point_Array Points;
                    Integer Count;
                };
                fn Total(Segment Value) -> Integer;
            }

            package body Geometry {
                fn Total(Segment Value) -> Integer {
                    return Value.Points[0].X
                        + Value.Points[0].Y
                        + Value.Points[1].X
                        + Value.Points[1].Y
                        + Value.Count;
                }
            }

            fn Build() -> Geometry.Segment {
                return Geometry.Segment {
                    Points = [
                        Geometry.Point { X = 1, Y = 2 },
                        Geometry.Point { X = 3, Y = 4 }
                    ],
                    Count = 2
                };
            }
            "#,
        )
        .expect("transpile should succeed");

        assert!(
            output
                .spec
                .contains("type Point_Array is array (0 .. 1) of Point;"),
            "unexpected spec: {}",
            output.spec
        );
        assert!(
            output
                .spec
                .contains("type Segment is record\n      Points : Point_Array;\n      Count : Integer;\n   end record;"),
            "unexpected spec: {}",
            output.spec
        );
        assert!(
            output
                .body
                .contains("return (Points => ((X => 1, Y => 2), (X => 3, Y => 4)), Count => 2);"),
            "unexpected body: {}",
            output.body
        );
    }

    #[test]
    fn transpiles_arrays_of_arrays_and_nested_array_aggregates() {
        let output = transpile(
            r#"
            import Math;

            package Math {
                type Row = [0..1] Integer;
                type Matrix = [0..1] Row;
                fn Trace(Matrix Value) -> Integer;
            }

            package body Math {
                fn Trace(Matrix Value) -> Integer {
                    Integer Total = 0;
                    for (Integer I in 0..1) {
                        Total = Total + Value[I][I];
                    }
                    return Total;
                }
            }

            fn Build() -> Math.Matrix {
                return [
                    [1, 2],
                    [3, 4]
                ];
            }
            "#,
        )
        .expect("transpile should succeed");

        assert!(
            output
                .spec
                .contains("type Row is array (0 .. 1) of Integer;"),
            "unexpected spec: {}",
            output.spec
        );
        assert!(
            output
                .spec
                .contains("type Matrix is array (0 .. 1) of Row;"),
            "unexpected spec: {}",
            output.spec
        );
        assert!(
            output.body.contains("return ((1, 2), (3, 4));"),
            "unexpected body: {}",
            output.body
        );
        assert!(
            output.body.contains("Total := Total + Value(I)(I);"),
            "unexpected body: {}",
            output.body
        );
    }

    #[test]
    fn rejects_record_aggregate_missing_field() {
        let error = transpile(
            r#"
            type Point = record {
                Integer X;
                Integer Y;
            };

            fn Origin() -> Point {
                return Point { X = 1 };
            }
            "#,
        )
        .expect_err("transpile should fail");

        assert!(
            error
                .message
                .contains("record aggregate for `Point` is missing field `Y`"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn rejects_record_aggregate_duplicate_field() {
        let error = transpile(
            r#"
            type Point = record {
                Integer X;
                Integer Y;
            };

            fn Origin() -> Point {
                return Point { X = 1, X = 2, Y = 3 };
            }
            "#,
        )
        .expect_err("transpile should fail");

        assert!(
            error
                .message
                .contains("record aggregate for `Point` contains duplicate field `X`"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn rejects_record_aggregate_for_non_record_type() {
        let error = transpile(
            r#"
            type Buffer = [0..1] Integer;

            fn Main() {
                Buffer Data = Buffer { X = 1 };
            }
            "#,
        )
        .expect_err("transpile should fail");

        assert!(
            error.message.contains("type `Buffer` is not a record type"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn transpiles_named_arguments_and_default_parameters() {
        let output = transpile(
            r#"
            import Scale;

            fn Scale(Integer Value, Integer Factor = 10) -> Integer {
                return Value * Factor;
            }

            fn Main() -> Integer {
                return Scale(Factor = 3, Value = 4);
            }
            "#,
        )
        .expect("transpile should succeed");

        assert_eq!(
            output.spec,
            "with Scale;\n\nfunction Scale(Value : Integer; Factor : Integer := 10) return Integer;\n\nfunction Main return Integer;"
        );
        assert_eq!(
            output.body,
            "with Scale;\n\nfunction Scale(Value : Integer; Factor : Integer := 10) return Integer is\nbegin\n   return Value * Factor;\nend Scale;\n\nfunction Main return Integer is\nbegin\n   return Scale(Factor => 3, Value => 4);\nend Main;"
        );
    }

    #[test]
    fn allows_omitting_defaulted_parameters_in_calls() {
        let output = transpile(
            r#"
            import Scale;

            fn Scale(Integer Value, Integer Factor = 10) -> Integer {
                return Value * Factor;
            }

            fn Main() -> Integer {
                return Scale(4);
            }
            "#,
        )
        .expect("transpile should succeed");

        assert!(
            output.body.contains("return Scale(4);"),
            "unexpected body: {}",
            output.body
        );
    }

    #[test]
    fn rejects_positional_argument_after_named_argument() {
        let error = transpile(
            r#"
            fn Scale(Integer Value, Integer Factor) -> Integer {
                return Value * Factor;
            }

            fn Main() -> Integer {
                return Scale(Value = 2, 3);
            }
            "#,
        )
        .expect_err("transpile should fail");

        assert!(
            error
                .message
                .contains("positional arguments cannot follow named arguments"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn rejects_duplicate_named_arguments() {
        let error = transpile(
            r#"
            fn Scale(Integer Value, Integer Factor) -> Integer {
                return Value * Factor;
            }

            fn Main() -> Integer {
                return Scale(Value = 2, Value = 3);
            }
            "#,
        )
        .expect_err("transpile should fail");

        assert!(
            error.message.contains("duplicate named argument `Value`"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn rejects_default_on_out_parameter() {
        let error = transpile(
            r#"
            fn Write(Integer Input; Integer Result = 0) {
                Result = Input;
            }
            "#,
        )
        .expect_err("transpile should fail");

        assert!(
            error
                .message
                .contains("default values are only allowed for `in` parameters"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn rejects_non_assignable_out_argument() {
        let error = transpile(
            r#"
            import Write;

            fn Write(Integer Input; Integer Result) {
                Result = Input;
            }

            fn Main() {
                Write(1, 2);
            }
            "#,
        )
        .expect_err("transpile should fail");

        assert!(
            error.message.contains(
                "no matching overload for `Write` with argument types (Integer, Integer)"
            ),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn rejects_definition_that_omits_declared_default_parameter() {
        let error = transpile(
            r#"
            fn Scale(Integer Value, Integer Factor = 10) -> Integer;

            fn Scale(Integer Value, Integer Factor) -> Integer {
                return Value * Factor;
            }
            "#,
        )
        .expect_err("transpile should fail");

        assert!(
            error
                .message
                .contains("definition of `Scale` must repeat the default for parameter `Factor`"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn transpiles_package_state_objects() {
        let output = transpile(
            r#"
            package State {
                Integer Count = 0;
                const Integer Limit = 10;
                fn Next() -> Integer;
            }

            package body State {
                Integer Step = 1;

                fn Next() -> Integer {
                    Count = Count + Step;
                    return Count;
                }
            }
            "#,
        )
        .expect("transpile should succeed");

        assert_eq!(
            output.spec,
            "package State is\n   Count : Integer := 0;\n   Limit : constant Integer := 10;\n   function Next return Integer;\nend State;"
        );
        assert_eq!(
            output.body,
            "package body State is\n   Step : Integer := 1;\n\n   function Next return Integer is\n   begin\n      Count := Count + Step;\n      return Count;\n   end Next;\nend State;"
        );
    }

    #[test]
    fn transpiles_qualified_package_object_access() {
        let output = transpile(
            r#"
            import State;

            package State {
                Integer Count = 2;
            }

            fn Read() -> Integer {
                return State.Count;
            }
            "#,
        )
        .expect("transpile should succeed");

        assert!(
            output.body.contains("return State.Count;"),
            "unexpected body: {}",
            output.body
        );
    }

    #[test]
    fn transpiles_zero_argument_calls_without_empty_parentheses() {
        let output = transpile(
            r#"
            import Next;
            import Touch;

            fn Next() -> Integer {
                return 1;
            }

            fn Touch() {
            }

            fn Main() -> Integer {
                Touch();
                return Next();
            }
            "#,
        )
        .expect("transpile should succeed");

        assert!(
            output.body.contains("   Touch;"),
            "unexpected body: {}",
            output.body
        );
        assert!(
            output.body.contains("return Next;"),
            "unexpected body: {}",
            output.body
        );
    }

    #[test]
    fn transpiles_nested_block_locals_in_control_flow() {
        let output = transpile(
            r#"
            fn Main() {
                Integer Total = 1;
                if (Total > 0) {
                    Integer Step = 2;
                    Total = Total + Step;
                }
            }
            "#,
        )
        .expect("transpile should succeed");

        assert_eq!(
            output.body,
            "procedure Main is\n   Total : Integer := 1;\nbegin\n   if Total > 0 then\n      declare\n         Step : Integer := 2;\n      begin\n         Total := Total + Step;\n      end;\n   end if;\nend Main;"
        );
    }

    #[test]
    fn transpiles_float_and_character_literals() {
        let output = transpile(
            r#"
            fn Main() {
                Float Value = 1.5;
                Character Letter = 'A';

                if (Value > 1.0) {
                    Put_Line("float");
                }

                if (Letter == 'A') {
                    Put_Line("char");
                }
            }
            "#,
        )
        .expect("transpile should succeed");

        assert_eq!(
            output.body,
            "procedure Main is\n   Value : Float := 1.5;\n   Letter : Character := 'A';\nbegin\n   if Value > 1.0 then\n      Put_Line(\"float\");\n   end if;\n   if Letter = 'A' then\n      Put_Line(\"char\");\n   end if;\nend Main;"
        );
    }

    #[test]
    fn rejects_nested_block_declaration_after_statement() {
        let error = transpile(
            r#"
            fn Main() {
                if (true) {
                    Touch();
                    Integer Late = 1;
                }
            }
            "#,
        )
        .expect_err("transpile should fail");

        assert!(
            error
                .message
                .contains("local declarations must appear before statements in a nested block"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn rejects_character_literal_type_mismatch() {
        let error = transpile(
            r#"
            fn Main() {
                Integer Value = 'A';
            }
            "#,
        )
        .expect_err("transpile should fail");

        assert!(
            error
                .message
                .contains("local initializer expects `Integer`, found `Character`"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn rejects_assignment_to_const_local() {
        let error = transpile(
            r#"
            fn Main() {
                const Integer Limit = 10;
                Limit = 11;
            }
            "#,
        )
        .expect_err("transpile should fail");

        assert!(
            error
                .message
                .contains("cannot assign to immutable value `Limit`"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn rejects_assignment_to_in_parameter() {
        let error = transpile(
            r#"
            fn Adjust(Integer Value) {
                Value = Value + 1;
            }
            "#,
        )
        .expect_err("transpile should fail");

        assert!(
            error
                .message
                .contains("cannot assign to immutable value `Value`"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn rejects_assignment_to_for_loop_iterator() {
        let error = transpile(
            r#"
            fn Main() {
                for (Integer I in 1..3) {
                    I = I + 1;
                }
            }
            "#,
        )
        .expect_err("transpile should fail");

        assert!(
            error
                .message
                .contains("cannot assign to immutable value `I`"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn rejects_assignment_to_const_package_object() {
        let error = transpile(
            r#"
            package State {
                const Integer Limit = 10;
                fn Touch();
            }

            package body State {
                fn Touch() {
                    Limit = 11;
                }
            }
            "#,
        )
        .expect_err("transpile should fail");

        assert!(
            error
                .message
                .contains("cannot assign to immutable value `Limit`"),
            "unexpected error: {error}"
        );
    }
}
