mod ast;
mod diagnostic;
mod lexer;
mod lowering;
mod parser;
mod sema;

pub use diagnostic::{Diagnostic, Position};
pub use lowering::{AdaOutputs, GeneratedFile};

pub fn transpile(source: &str) -> Result<AdaOutputs, Diagnostic> {
    let tokens = lexer::lex(source)?;
    let program = parser::parse(&tokens)?;
    sema::validate(&program)?;
    let ada_program = lowering::lower(program)?;
    Ok(lowering::render(&ada_program))
}

pub fn transpile_files(
    source: &str,
    fallback_stem: &str,
) -> Result<Vec<GeneratedFile>, Diagnostic> {
    let tokens = lexer::lex(source)?;
    let program = parser::parse(&tokens)?;
    sema::validate(&program)?;
    let ada_program = lowering::lower(program)?;
    Ok(lowering::render_files(&ada_program, fallback_stem))
}

#[cfg(test)]
mod tests {
    use super::transpile;

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
    fn rejects_call_arity_mismatch_for_known_subprogram() {
        let error = transpile(
            r#"
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
    fn rejects_package_body_without_matching_spec_declaration() {
        let error = transpile(
            r#"
            package Math {
                fn Add(Integer A, Integer B) -> Integer;
            }

            package body Math {
                fn Sub(Integer A, Integer B) -> Integer {
                    return A + B;
                }
            }
            "#,
        )
        .expect_err("transpile should fail");

        assert!(
            error
                .message
                .contains("package body `Math` defines `Sub` without a matching declaration"),
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
    fn rejects_function_call_used_as_statement() {
        let error = transpile(
            r#"
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
}
