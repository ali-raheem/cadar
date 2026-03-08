use std::collections::{HashMap, HashSet};

use crate::{
    ast::{
        ArrayType, BinaryOp, Block, BlockItem, CallArg, CaseArm, CaseStatement, DependItem,
        DependTarget, DependsContract, ElseIfBranch, EnumType, ExceptionHandler, ExceptionSelector,
        Expr, ForStatement, GlobalContract, GlobalItem, GlobalMode, IfStatement, Item, LocalDecl,
        LoopVariant, LoopVariantDirection, Name, Package, PackageItem, Param, ParamMode, Program,
        RangeType, RecordField, RecordFieldInit, RecordType, Statement, StatementBlock, Subprogram,
        TryStatement, TypeDecl, UnaryOp,
    },
    diagnostic::{Diagnostic, Position},
    lexer::{Token, TokenKind},
};

pub fn parse(tokens: &[Token]) -> Result<Program, Diagnostic> {
    Parser::new(tokens).parse_program()
}

struct Parser<'a> {
    tokens: &'a [Token],
    index: usize,
    package_aliases: HashMap<String, Name>,
}

impl<'a> Parser<'a> {
    fn new(tokens: &'a [Token]) -> Self {
        Self {
            tokens,
            index: 0,
            package_aliases: HashMap::new(),
        }
    }

    fn parse_program(&mut self) -> Result<Program, Diagnostic> {
        let mut items = Vec::new();
        while !self.is_eof() {
            items.push(self.parse_item()?);
        }
        Ok(Program { items })
    }

    fn parse_item(&mut self) -> Result<Item, Diagnostic> {
        match self.current_kind() {
            TokenKind::Import => {
                let position = self.current().position;
                self.bump();
                let name = self.parse_name()?;
                let alias = self.parse_context_alias()?;
                if let Some(alias_name) = &alias {
                    self.package_aliases
                        .insert(alias_name.clone(), name.clone());
                }
                self.expect_simple(TokenKind::Semicolon, "expected `;` after import")?;
                Ok(Item::Import {
                    name,
                    alias,
                    position,
                })
            }
            TokenKind::Use => {
                let position = self.current().position;
                self.bump();
                let name = self.parse_name()?;
                let alias = self.parse_context_alias()?;
                if let Some(alias_name) = &alias {
                    self.package_aliases
                        .insert(alias_name.clone(), name.clone());
                }
                self.expect_simple(TokenKind::Semicolon, "expected `;` after use")?;
                Ok(Item::Use {
                    name,
                    alias,
                    position,
                })
            }
            TokenKind::Fn => Ok(Item::Subprogram(self.parse_subprogram()?)),
            TokenKind::Type => Ok(Item::Type(self.parse_type_decl()?)),
            TokenKind::Enum => Ok(Item::Type(self.parse_enum_decl()?)),
            TokenKind::Package => Ok(Item::Package(self.parse_package()?)),
            _ => {
                Err(self.error_here("expected `import`, `use`, `fn`, `type`, `enum`, or `package`"))
            }
        }
    }

    fn parse_package(&mut self) -> Result<Package, Diagnostic> {
        let position = self.current().position;
        self.expect_simple(TokenKind::Package, "expected `package`")?;
        let is_body = self.matches_simple(&TokenKind::Body);
        let name = self.parse_name()?;
        self.expect_simple(TokenKind::LBrace, "expected `{` after package name")?;

        let mut items = Vec::new();
        while !self.check(&TokenKind::RBrace) && !self.check(&TokenKind::Private) {
            items.push(self.parse_package_item()?);
        }

        let private_items = if self.matches_simple(&TokenKind::Private) {
            if is_body {
                return Err(self.error_here("package bodies cannot contain `private` sections"));
            }
            self.expect_simple(TokenKind::LBrace, "expected `{` after `private`")?;
            let mut private_items = Vec::new();
            while !self.check(&TokenKind::RBrace) {
                private_items.push(self.parse_package_item()?);
            }
            self.expect_simple(
                TokenKind::RBrace,
                "expected `}` after private package items",
            )?;
            private_items
        } else {
            Vec::new()
        };

        self.expect_simple(TokenKind::RBrace, "expected `}` after package body")?;

        Ok(Package {
            is_body,
            name,
            items,
            private_items,
            position,
        })
    }

    fn parse_package_item(&mut self) -> Result<PackageItem, Diagnostic> {
        match self.current_kind() {
            TokenKind::Fn => Ok(PackageItem::Subprogram(self.parse_subprogram()?)),
            TokenKind::Type => Ok(PackageItem::Type(self.parse_type_decl()?)),
            TokenKind::Enum => Ok(PackageItem::Type(self.parse_enum_decl()?)),
            _ if self.looks_like_local_decl() => Ok(PackageItem::Object(self.parse_local_decl()?)),
            _ => Err(self
                .error_here("expected `fn`, `type`, `enum`, or object declaration inside package")),
        }
    }

    fn parse_subprogram(&mut self) -> Result<Subprogram, Diagnostic> {
        let position = self.current().position;
        self.expect_simple(TokenKind::Fn, "expected `fn`")?;
        let name = self.expect_identifier("expected a subprogram name")?;
        self.expect_simple(TokenKind::LParen, "expected `(` after subprogram name")?;
        let params = self.parse_params()?;
        self.expect_simple(TokenKind::RParen, "expected `)` after parameter list")?;

        let return_type = if self.matches_simple(&TokenKind::Arrow) {
            Some(self.parse_name()?)
        } else {
            None
        };

        let mut requires = Vec::new();
        let mut ensures = Vec::new();
        let mut global = None;
        let mut depends = None;
        while matches!(
            self.current_kind(),
            TokenKind::Requires | TokenKind::Ensures | TokenKind::Global | TokenKind::Depends
        ) {
            match self.current_kind() {
                TokenKind::Requires => {
                    self.bump();
                    self.expect_simple(TokenKind::LParen, "expected `(` after `requires`")?;
                    requires.push(self.parse_expr()?);
                    self.expect_simple(TokenKind::RParen, "expected `)` after `requires`")?;
                }
                TokenKind::Ensures => {
                    self.bump();
                    self.expect_simple(TokenKind::LParen, "expected `(` after `ensures`")?;
                    ensures.push(self.parse_expr()?);
                    self.expect_simple(TokenKind::RParen, "expected `)` after `ensures`")?;
                }
                TokenKind::Global => {
                    if global.is_some() {
                        return Err(
                            self.error_here("subprogram cannot contain multiple `global` clauses")
                        );
                    }
                    global = Some(self.parse_global_contract()?);
                }
                TokenKind::Depends => {
                    if depends.is_some() {
                        return Err(
                            self.error_here("subprogram cannot contain multiple `depends` clauses")
                        );
                    }
                    depends = Some(self.parse_depends_contract()?);
                }
                _ => unreachable!("contract loop should only match supported clauses"),
            }
        }

        let body = if self.matches_simple(&TokenKind::Semicolon) {
            None
        } else if self.check(&TokenKind::LBrace) {
            Some(self.parse_block()?)
        } else {
            return Err(self.error_here("expected `;` or subprogram body"));
        };

        Ok(Subprogram {
            name,
            params,
            return_type,
            requires,
            ensures,
            global,
            depends,
            body,
            position,
        })
    }

    fn parse_params(&mut self) -> Result<Vec<Param>, Diagnostic> {
        if self.check(&TokenKind::RParen) {
            return Ok(Vec::new());
        }

        let mut params = Vec::new();
        let mut group_index = 0usize;

        loop {
            let mode = match group_index {
                0 => ParamMode::In,
                1 => ParamMode::Out,
                2 => ParamMode::InOut,
                _ => return Err(self.error_here("at most three parameter groups are supported")),
            };

            params.extend(self.parse_param_group(mode)?);

            if self.matches_simple(&TokenKind::Semicolon) {
                group_index += 1;
                continue;
            }

            break;
        }

        Ok(params)
    }

    fn parse_global_contract(&mut self) -> Result<GlobalContract, Diagnostic> {
        self.expect_simple(TokenKind::Global, "expected `global`")?;
        self.expect_simple(TokenKind::LParen, "expected `(` after `global`")?;

        if self.matches_simple(&TokenKind::Null) {
            self.expect_simple(TokenKind::RParen, "expected `)` after `global(null)`")?;
            return Ok(GlobalContract {
                items: vec![GlobalItem {
                    mode: GlobalMode::Null,
                    names: Vec::new(),
                }],
            });
        }

        let mut items = Vec::new();
        let mut seen_modes = HashSet::new();
        loop {
            let mode = self.parse_global_mode()?;
            if !seen_modes.insert(mode) {
                return Err(self.error_here("duplicate mode in `global` clause"));
            }
            self.expect_simple(TokenKind::FatArrow, "expected `=>` after global mode")?;
            let names = self.parse_contract_name_list()?;
            items.push(GlobalItem { mode, names });

            if !self.matches_simple(&TokenKind::Comma) {
                break;
            }
        }

        self.expect_simple(TokenKind::RParen, "expected `)` after `global` clause")?;
        Ok(GlobalContract { items })
    }

    fn parse_global_mode(&mut self) -> Result<GlobalMode, Diagnostic> {
        match self.current_kind() {
            TokenKind::Identifier(text) => {
                let mode = match text.as_str() {
                    "input" => GlobalMode::Input,
                    "output" => GlobalMode::Output,
                    "in_out" => GlobalMode::InOut,
                    "proof_in" => GlobalMode::ProofIn,
                    _ => {
                        return Err(self.error_here(
                            "expected `input`, `output`, `in_out`, or `proof_in` in `global` clause",
                        ));
                    }
                };
                self.bump();
                Ok(mode)
            }
            _ => Err(self.error_here(
                "expected `input`, `output`, `in_out`, or `proof_in` in `global` clause",
            )),
        }
    }

    fn parse_depends_contract(&mut self) -> Result<DependsContract, Diagnostic> {
        self.expect_simple(TokenKind::Depends, "expected `depends`")?;
        self.expect_simple(TokenKind::LParen, "expected `(` after `depends`")?;

        let mut items = Vec::new();
        let mut seen_targets = HashSet::new();
        loop {
            let target = self.parse_depend_target()?;
            if !seen_targets.insert(depend_target_key(&target)) {
                return Err(self.error_here("duplicate target in `depends` clause"));
            }
            self.expect_simple(TokenKind::FatArrow, "expected `=>` in `depends` clause")?;
            let sources = self.parse_contract_name_list()?;
            items.push(DependItem { target, sources });

            if !self.matches_simple(&TokenKind::Comma) {
                break;
            }
        }

        self.expect_simple(TokenKind::RParen, "expected `)` after `depends` clause")?;
        Ok(DependsContract { items })
    }

    fn parse_depend_target(&mut self) -> Result<DependTarget, Diagnostic> {
        if self.matches_simple(&TokenKind::Null) {
            return Ok(DependTarget::Null);
        }

        if let TokenKind::Identifier(text) = self.current_kind()
            && text == "result"
        {
            self.bump();
            return Ok(DependTarget::Result);
        }

        Ok(DependTarget::Name(self.parse_name()?))
    }

    fn parse_contract_name_list(&mut self) -> Result<Vec<Name>, Diagnostic> {
        if self.matches_simple(&TokenKind::LBracket) {
            let mut names = Vec::new();
            loop {
                names.push(self.parse_name()?);
                if !self.matches_simple(&TokenKind::Comma) {
                    break;
                }
            }
            self.expect_simple(TokenKind::RBracket, "expected `]` after name list")?;
            Ok(names)
        } else {
            Ok(vec![self.parse_name()?])
        }
    }

    fn parse_param_group(&mut self, mode: ParamMode) -> Result<Vec<Param>, Diagnostic> {
        let mut params = Vec::new();
        loop {
            let ty = self.parse_name()?;
            let name = self.expect_identifier("expected a parameter name")?;
            let default = if self.matches_simple(&TokenKind::Assign) {
                Some(self.parse_expr()?)
            } else {
                None
            };
            params.push(Param {
                mode,
                ty,
                name,
                default,
            });

            if !self.matches_simple(&TokenKind::Comma) {
                break;
            }
        }
        Ok(params)
    }

    fn parse_type_decl(&mut self) -> Result<TypeDecl, Diagnostic> {
        let position = self.current().position;
        self.expect_simple(TokenKind::Type, "expected `type`")?;
        let name = self.expect_identifier("expected a type name")?;
        self.expect_simple(TokenKind::Assign, "expected `=` after type name")?;

        if self.matches_simple(&TokenKind::Record) {
            let fields = self.parse_record_fields()?;
            self.expect_simple(TokenKind::Semicolon, "expected `;` after record type")?;
            return Ok(TypeDecl::Record(RecordType {
                name,
                fields,
                position,
            }));
        }

        if self.matches_simple(&TokenKind::LBracket) {
            let start = self.parse_expr()?;
            self.expect_simple(TokenKind::DotDot, "expected `..` in array type")?;
            let end = self.parse_expr()?;
            self.expect_simple(TokenKind::RBracket, "expected `]` after array bounds")?;
            let element_type = self.parse_name()?;
            self.expect_simple(TokenKind::Semicolon, "expected `;` after array type")?;
            return Ok(TypeDecl::Array(ArrayType {
                name,
                start,
                end,
                element_type,
                position,
            }));
        }

        let base = self.parse_name()?;
        self.expect_identifier_text("range", "expected `range` after base type")?;
        let start = self.parse_expr()?;
        self.expect_simple(TokenKind::DotDot, "expected `..` in range type")?;
        let end = self.parse_expr()?;
        self.expect_simple(TokenKind::Semicolon, "expected `;` after range type")?;
        Ok(TypeDecl::Range(RangeType {
            name,
            base,
            start,
            end,
            position,
        }))
    }

    fn parse_enum_decl(&mut self) -> Result<TypeDecl, Diagnostic> {
        let position = self.current().position;
        self.expect_simple(TokenKind::Enum, "expected `enum`")?;
        let name = self.expect_identifier("expected an enum name")?;
        self.expect_simple(TokenKind::LBrace, "expected `{` after enum name")?;

        let mut variants = Vec::new();
        while !self.check(&TokenKind::RBrace) {
            variants.push(self.expect_identifier("expected an enum variant")?);
            if !self.matches_simple(&TokenKind::Comma) {
                break;
            }
            if self.check(&TokenKind::RBrace) {
                break;
            }
        }

        self.expect_simple(TokenKind::RBrace, "expected `}` after enum variants")?;
        let _ = self.matches_simple(&TokenKind::Semicolon);

        Ok(TypeDecl::Enum(EnumType {
            name,
            variants,
            position,
        }))
    }

    fn parse_record_fields(&mut self) -> Result<Vec<RecordField>, Diagnostic> {
        self.expect_simple(TokenKind::LBrace, "expected `{` after `record`")?;
        let mut fields = Vec::new();
        while !self.check(&TokenKind::RBrace) {
            let ty = self.parse_name()?;
            let name = self.expect_identifier("expected a record field name")?;
            self.expect_simple(TokenKind::Semicolon, "expected `;` after record field")?;
            fields.push(RecordField { ty, name });
        }
        self.expect_simple(TokenKind::RBrace, "expected `}` after record fields")?;
        Ok(fields)
    }

    fn parse_block(&mut self) -> Result<Block, Diagnostic> {
        self.expect_simple(TokenKind::LBrace, "expected `{`")?;
        let mut items = Vec::new();

        while !self.check(&TokenKind::RBrace) {
            if self.looks_like_local_decl() {
                items.push(BlockItem::LocalDecl(self.parse_local_decl()?));
            } else {
                items.push(BlockItem::Statement(self.parse_statement()?));
            }
        }

        self.expect_simple(TokenKind::RBrace, "expected `}`")?;
        Ok(Block { items })
    }

    fn parse_local_decl(&mut self) -> Result<LocalDecl, Diagnostic> {
        let position = self.current().position;
        let is_const = self.matches_simple(&TokenKind::Const);
        let ty = self.parse_name()?;
        let name = self.expect_identifier("expected a local variable name")?;
        let initializer = if self.matches_simple(&TokenKind::Assign) {
            Some(self.parse_expr()?)
        } else {
            None
        };
        self.expect_simple(TokenKind::Semicolon, "expected `;` after local declaration")?;

        Ok(LocalDecl {
            is_const,
            ty,
            name,
            initializer,
            position,
        })
    }

    fn parse_statement(&mut self) -> Result<Statement, Diagnostic> {
        let position = self.current().position;
        match self.current_kind() {
            TokenKind::Null => {
                self.bump();
                self.expect_simple(TokenKind::Semicolon, "expected `;` after `null`")?;
                Ok(Statement::Null { position })
            }
            TokenKind::Raise => {
                self.bump();
                let exception = self.parse_name()?;
                self.expect_simple(TokenKind::Semicolon, "expected `;` after `raise`")?;
                Ok(Statement::Raise {
                    exception,
                    position,
                })
            }
            TokenKind::Break => {
                self.bump();
                self.expect_simple(TokenKind::Semicolon, "expected `;` after `break`")?;
                Ok(Statement::Break { position })
            }
            TokenKind::Continue => {
                self.bump();
                self.expect_simple(TokenKind::Semicolon, "expected `;` after `continue`")?;
                Ok(Statement::Continue { position })
            }
            TokenKind::Assert => {
                self.bump();
                self.expect_simple(TokenKind::LParen, "expected `(` after `assert`")?;
                let expr = self.parse_expr()?;
                self.expect_simple(TokenKind::RParen, "expected `)` after assert expression")?;
                self.expect_simple(TokenKind::Semicolon, "expected `;` after `assert`")?;
                Ok(Statement::Assert { expr, position })
            }
            TokenKind::Return => {
                self.bump();
                let expr = if self.check(&TokenKind::Semicolon) {
                    None
                } else {
                    Some(self.parse_expr()?)
                };
                self.expect_simple(TokenKind::Semicolon, "expected `;` after return")?;
                Ok(Statement::Return { expr, position })
            }
            TokenKind::Try => self.parse_try_statement(),
            TokenKind::For => self.parse_for_statement(),
            TokenKind::If => self.parse_if_statement(),
            TokenKind::Case => self.parse_case_statement(),
            TokenKind::While => self.parse_while_statement(),
            TokenKind::Identifier(_) => {
                let expr = self.parse_expr()?;
                if self.matches_simple(&TokenKind::Assign) {
                    let value = self.parse_expr()?;
                    self.expect_simple(TokenKind::Semicolon, "expected `;` after assignment")?;
                    Ok(Statement::Assign {
                        target: expr,
                        value,
                        position,
                    })
                } else {
                    self.expect_simple(TokenKind::Semicolon, "expected `;` after expression")?;
                    Ok(Statement::Expr { expr, position })
                }
            }
            _ => Err(self.error_here("expected a statement")),
        }
    }

    fn parse_try_statement(&mut self) -> Result<Statement, Diagnostic> {
        let position = self.current().position;
        self.expect_simple(TokenKind::Try, "expected `try`")?;
        let body = self.parse_statement_block()?;

        let mut handlers = Vec::new();
        let mut saw_others = false;
        while self.matches_simple(&TokenKind::Catch) {
            let handler_position = self.previous_position();
            self.expect_simple(TokenKind::LParen, "expected `(` after `catch`")?;
            let selector = if self.matches_simple(&TokenKind::Others) {
                if saw_others {
                    return Err(self.error_here("`catch (others)` can only appear once"));
                }
                saw_others = true;
                ExceptionSelector::Others
            } else {
                if saw_others {
                    return Err(self.error_here("`catch (others)` must be the last handler"));
                }
                ExceptionSelector::Name(self.parse_name()?)
            };
            self.expect_simple(TokenKind::RParen, "expected `)` after catch selector")?;
            let body = self.parse_statement_block()?;
            handlers.push(ExceptionHandler {
                selector,
                body,
                position: handler_position,
            });
        }

        if handlers.is_empty() {
            return Err(self.error_here("`try` requires at least one `catch` handler"));
        }

        Ok(Statement::Try(TryStatement {
            body,
            handlers,
            position,
        }))
    }

    fn parse_if_statement(&mut self) -> Result<Statement, Diagnostic> {
        let position = self.current().position;
        self.expect_simple(TokenKind::If, "expected `if`")?;
        self.expect_simple(TokenKind::LParen, "expected `(` after `if`")?;
        let condition = self.parse_expr()?;
        self.expect_simple(TokenKind::RParen, "expected `)` after if condition")?;
        let then_branch = self.parse_statement_block()?;

        let mut else_if_branches = Vec::new();
        let mut else_branch = None;

        while self.matches_simple(&TokenKind::Else) {
            if self.matches_simple(&TokenKind::If) {
                let position = self.previous_position();
                self.expect_simple(TokenKind::LParen, "expected `(` after `else if`")?;
                let condition = self.parse_expr()?;
                self.expect_simple(TokenKind::RParen, "expected `)` after else-if condition")?;
                let body = self.parse_statement_block()?;
                else_if_branches.push(ElseIfBranch {
                    condition,
                    body,
                    position,
                });
            } else {
                else_branch = Some(self.parse_statement_block()?);
                break;
            }
        }

        Ok(Statement::If(IfStatement {
            condition,
            then_branch,
            else_if_branches,
            else_branch,
            position,
        }))
    }

    fn parse_while_statement(&mut self) -> Result<Statement, Diagnostic> {
        let position = self.current().position;
        self.expect_simple(TokenKind::While, "expected `while`")?;
        self.expect_simple(TokenKind::LParen, "expected `(` after `while`")?;
        let condition = self.parse_expr()?;
        self.expect_simple(TokenKind::RParen, "expected `)` after while condition")?;
        let (invariants, variants) = self.parse_loop_annotations()?;
        let body = self.parse_statement_block()?;
        Ok(Statement::While {
            condition,
            invariants,
            variants,
            body,
            position,
        })
    }

    fn parse_case_statement(&mut self) -> Result<Statement, Diagnostic> {
        let position = self.current().position;
        self.expect_simple(TokenKind::Case, "expected `case`")?;
        self.expect_simple(TokenKind::LParen, "expected `(` after `case`")?;
        let expr = self.parse_expr()?;
        self.expect_simple(TokenKind::RParen, "expected `)` after case expression")?;
        self.expect_simple(TokenKind::LBrace, "expected `{` after case expression")?;

        let mut arms = Vec::new();
        let mut else_arm = None;

        while !self.check(&TokenKind::RBrace) {
            if self.matches_simple(&TokenKind::When) {
                let arm_position = self.previous_position();
                let mut choices = vec![self.parse_expr()?];
                while self.matches_simple(&TokenKind::Comma) {
                    choices.push(self.parse_expr()?);
                }
                self.expect_simple(TokenKind::FatArrow, "expected `=>` after case choices")?;
                let body = self.parse_statement_block()?;
                arms.push(CaseArm {
                    choices,
                    body,
                    position: arm_position,
                });
                continue;
            }

            if self.matches_simple(&TokenKind::Else) {
                if else_arm.is_some() {
                    return Err(
                        self.error_here("case statement cannot contain multiple `else` arms")
                    );
                }
                self.expect_simple(TokenKind::FatArrow, "expected `=>` after `else`")?;
                else_arm = Some(self.parse_statement_block()?);
                continue;
            }

            return Err(self.error_here("expected `when` or `else` in case statement"));
        }

        self.expect_simple(TokenKind::RBrace, "expected `}` after case statement")?;
        Ok(Statement::Case(CaseStatement {
            expr,
            arms,
            else_arm,
            position,
        }))
    }

    fn parse_for_statement(&mut self) -> Result<Statement, Diagnostic> {
        let position = self.current().position;
        self.expect_simple(TokenKind::For, "expected `for`")?;
        self.expect_simple(TokenKind::LParen, "expected `(` after `for`")?;
        let iterator_type = self.parse_name()?;
        let iterator = self.expect_identifier("expected a loop variable name")?;
        self.expect_simple(TokenKind::In, "expected `in` in for loop")?;
        let start = self.parse_expr()?;
        self.expect_simple(TokenKind::DotDot, "expected `..` in for loop range")?;
        let end = self.parse_expr()?;
        self.expect_simple(TokenKind::RParen, "expected `)` after for loop header")?;
        let (invariants, variants) = self.parse_loop_annotations()?;
        let body = self.parse_statement_block()?;

        Ok(Statement::For(ForStatement {
            iterator_type,
            iterator,
            start,
            end,
            invariants,
            variants,
            body,
            position,
        }))
    }

    fn parse_loop_annotations(&mut self) -> Result<(Vec<Expr>, Vec<LoopVariant>), Diagnostic> {
        let mut invariants = Vec::new();
        let mut variants = Vec::new();

        loop {
            match self.current_kind() {
                TokenKind::Invariant => {
                    self.bump();
                    self.expect_simple(TokenKind::LParen, "expected `(` after `invariant`")?;
                    invariants.push(self.parse_expr()?);
                    self.expect_simple(
                        TokenKind::RParen,
                        "expected `)` after invariant expression",
                    )?;
                }
                TokenKind::Increases => {
                    self.bump();
                    self.expect_simple(TokenKind::LParen, "expected `(` after `increases`")?;
                    let expr = self.parse_expr()?;
                    self.expect_simple(TokenKind::RParen, "expected `)` after variant expression")?;
                    variants.push(LoopVariant {
                        direction: LoopVariantDirection::Increases,
                        expr,
                    });
                }
                TokenKind::Decreases => {
                    self.bump();
                    self.expect_simple(TokenKind::LParen, "expected `(` after `decreases`")?;
                    let expr = self.parse_expr()?;
                    self.expect_simple(TokenKind::RParen, "expected `)` after variant expression")?;
                    variants.push(LoopVariant {
                        direction: LoopVariantDirection::Decreases,
                        expr,
                    });
                }
                _ => return Ok((invariants, variants)),
            }
        }
    }

    fn parse_statement_block(&mut self) -> Result<StatementBlock, Diagnostic> {
        self.expect_simple(TokenKind::LBrace, "expected `{`")?;
        let mut items = Vec::new();
        while !self.check(&TokenKind::RBrace) {
            if self.looks_like_local_decl() {
                items.push(BlockItem::LocalDecl(self.parse_local_decl()?));
            } else {
                items.push(BlockItem::Statement(self.parse_statement()?));
            }
        }
        self.expect_simple(TokenKind::RBrace, "expected `}`")?;
        Ok(StatementBlock { items })
    }

    fn parse_expr(&mut self) -> Result<Expr, Diagnostic> {
        self.parse_or()
    }

    fn parse_or(&mut self) -> Result<Expr, Diagnostic> {
        let mut expr = self.parse_and()?;
        loop {
            let op = if self.matches_simple(&TokenKind::OrOr) {
                BinaryOp::ShortCircuitOr
            } else if self.matches_simple(&TokenKind::Or) {
                if self.matches_simple(&TokenKind::Else) {
                    BinaryOp::ShortCircuitOr
                } else {
                    BinaryOp::Or
                }
            } else {
                return Ok(expr);
            };

            let rhs = self.parse_and()?;
            let position = expr.position();
            expr = Expr::Binary {
                lhs: Box::new(expr),
                op,
                rhs: Box::new(rhs),
                position,
            };
        }
    }

    fn parse_and(&mut self) -> Result<Expr, Diagnostic> {
        let mut expr = self.parse_equality()?;
        loop {
            let op = if self.matches_simple(&TokenKind::AndAnd) {
                BinaryOp::ShortCircuitAnd
            } else if self.matches_simple(&TokenKind::And) {
                if self.matches_simple(&TokenKind::Then) {
                    BinaryOp::ShortCircuitAnd
                } else {
                    BinaryOp::And
                }
            } else {
                return Ok(expr);
            };

            let rhs = self.parse_equality()?;
            let position = expr.position();
            expr = Expr::Binary {
                lhs: Box::new(expr),
                op,
                rhs: Box::new(rhs),
                position,
            };
        }
    }

    fn parse_equality(&mut self) -> Result<Expr, Diagnostic> {
        let mut expr = self.parse_comparison()?;
        loop {
            let op = if self.matches_simple(&TokenKind::EqualEqual) {
                Some(BinaryOp::Equal)
            } else if self.matches_simple(&TokenKind::BangEqual) {
                Some(BinaryOp::NotEqual)
            } else {
                None
            };

            let Some(op) = op else {
                return Ok(expr);
            };

            let rhs = self.parse_comparison()?;
            let position = expr.position();
            expr = Expr::Binary {
                lhs: Box::new(expr),
                op,
                rhs: Box::new(rhs),
                position,
            };
        }
    }

    fn parse_comparison(&mut self) -> Result<Expr, Diagnostic> {
        let mut expr = self.parse_term()?;
        loop {
            let op = if self.matches_simple(&TokenKind::Less) {
                Some(BinaryOp::Less)
            } else if self.matches_simple(&TokenKind::LessEqual) {
                Some(BinaryOp::LessEqual)
            } else if self.matches_simple(&TokenKind::Greater) {
                Some(BinaryOp::Greater)
            } else if self.matches_simple(&TokenKind::GreaterEqual) {
                Some(BinaryOp::GreaterEqual)
            } else {
                None
            };

            let Some(op) = op else {
                return Ok(expr);
            };

            let rhs = self.parse_term()?;
            let position = expr.position();
            expr = Expr::Binary {
                lhs: Box::new(expr),
                op,
                rhs: Box::new(rhs),
                position,
            };
        }
    }

    fn parse_term(&mut self) -> Result<Expr, Diagnostic> {
        let mut expr = self.parse_factor()?;
        loop {
            let op = if self.matches_simple(&TokenKind::Plus) {
                Some(BinaryOp::Add)
            } else if self.matches_simple(&TokenKind::Minus) {
                Some(BinaryOp::Subtract)
            } else {
                None
            };

            let Some(op) = op else {
                return Ok(expr);
            };

            let rhs = self.parse_factor()?;
            let position = expr.position();
            expr = Expr::Binary {
                lhs: Box::new(expr),
                op,
                rhs: Box::new(rhs),
                position,
            };
        }
    }

    fn parse_factor(&mut self) -> Result<Expr, Diagnostic> {
        let mut expr = self.parse_unary()?;
        loop {
            let op = if self.matches_simple(&TokenKind::Star) {
                Some(BinaryOp::Multiply)
            } else if self.matches_simple(&TokenKind::Slash) {
                Some(BinaryOp::Divide)
            } else {
                None
            };

            let Some(op) = op else {
                return Ok(expr);
            };

            let rhs = self.parse_unary()?;
            let position = expr.position();
            expr = Expr::Binary {
                lhs: Box::new(expr),
                op,
                rhs: Box::new(rhs),
                position,
            };
        }
    }

    fn parse_unary(&mut self) -> Result<Expr, Diagnostic> {
        if self.matches_simple(&TokenKind::Minus) {
            let position = self.previous_position();
            Ok(Expr::Unary {
                op: UnaryOp::Negate,
                expr: Box::new(self.parse_unary()?),
                position,
            })
        } else if self.matches_simple(&TokenKind::Not) {
            let position = self.previous_position();
            Ok(Expr::Unary {
                op: UnaryOp::Not,
                expr: Box::new(self.parse_unary()?),
                position,
            })
        } else {
            self.parse_postfix()
        }
    }

    fn parse_postfix(&mut self) -> Result<Expr, Diagnostic> {
        let mut expr = self.parse_primary()?;
        loop {
            if self.matches_simple(&TokenKind::Dot) {
                expr = self.canonicalize_package_alias_expr(expr);
                let position = expr.position();
                let member = self.expect_identifier("expected an identifier after `.`")?;
                expr = Expr::Member {
                    base: Box::new(expr),
                    member,
                    position,
                };
                continue;
            }

            if self.matches_simple(&TokenKind::LBracket) {
                let position = expr.position();
                let start = self.parse_expr()?;
                expr = if self.matches_simple(&TokenKind::DotDot) {
                    let end = self.parse_expr()?;
                    self.expect_simple(TokenKind::RBracket, "expected `]` after slice expression")?;
                    Expr::Slice {
                        base: Box::new(expr),
                        start: Box::new(start),
                        end: Box::new(end),
                        position,
                    }
                } else {
                    self.expect_simple(TokenKind::RBracket, "expected `]` after index expression")?;
                    Expr::Index {
                        base: Box::new(expr),
                        index: Box::new(start),
                        position,
                    }
                };
                continue;
            }

            if self.matches_simple(&TokenKind::LParen) {
                let position = expr.position();
                let args = self.parse_call_args()?;
                self.expect_simple(TokenKind::RParen, "expected `)` after arguments")?;
                expr = Expr::Call {
                    callee: Box::new(expr),
                    args,
                    position,
                };
                continue;
            }

            if self.matches_simple(&TokenKind::LBrace) {
                let position = expr.position();
                let Some(ty) = expr_to_name(&expr) else {
                    return Err(self.error_here("record aggregates require a type name"));
                };
                let fields = self.parse_record_literal_fields()?;
                expr = Expr::RecordLiteral {
                    ty,
                    fields,
                    position,
                };
                continue;
            }

            return Ok(expr);
        }
    }

    fn parse_primary(&mut self) -> Result<Expr, Diagnostic> {
        let position = self.current().position;
        match self.current_kind() {
            TokenKind::True => {
                self.bump();
                Ok(Expr::Bool {
                    value: true,
                    position,
                })
            }
            TokenKind::False => {
                self.bump();
                Ok(Expr::Bool {
                    value: false,
                    position,
                })
            }
            TokenKind::Integer(value) => {
                let value = value.clone();
                self.bump();
                Ok(Expr::Integer { value, position })
            }
            TokenKind::Float(value) => {
                let value = value.clone();
                self.bump();
                Ok(Expr::Float { value, position })
            }
            TokenKind::Character(value) => {
                let value = *value;
                self.bump();
                Ok(Expr::Character { value, position })
            }
            TokenKind::String(value) => {
                let value = value.clone();
                self.bump();
                Ok(Expr::String { value, position })
            }
            TokenKind::Identifier(_) => Ok(Expr::Name {
                name: Name {
                    segments: vec![self.expect_identifier("expected an identifier")?],
                },
                position,
            }),
            TokenKind::LBracket => {
                self.bump();
                let mut elements = Vec::new();
                if !self.check(&TokenKind::RBracket) {
                    loop {
                        elements.push(self.parse_expr()?);
                        if !self.matches_simple(&TokenKind::Comma) {
                            break;
                        }
                    }
                }
                self.expect_simple(TokenKind::RBracket, "expected `]` after array literal")?;
                Ok(Expr::ArrayLiteral { elements, position })
            }
            TokenKind::LParen => {
                self.bump();
                let expr = self.parse_expr()?;
                self.expect_simple(TokenKind::RParen, "expected `)` after expression")?;
                Ok(expr)
            }
            _ => Err(self.error_here("expected an expression")),
        }
    }

    fn parse_record_literal_fields(&mut self) -> Result<Vec<RecordFieldInit>, Diagnostic> {
        let mut fields = Vec::new();
        if !self.check(&TokenKind::RBrace) {
            loop {
                let name = self.expect_identifier("expected a record field name")?;
                self.expect_simple(TokenKind::Assign, "expected `=` after record field name")?;
                let value = self.parse_expr()?;
                fields.push(RecordFieldInit { name, value });
                if !self.matches_simple(&TokenKind::Comma) {
                    break;
                }
                if self.check(&TokenKind::RBrace) {
                    break;
                }
            }
        }
        self.expect_simple(TokenKind::RBrace, "expected `}` after record aggregate")?;
        Ok(fields)
    }

    fn parse_call_args(&mut self) -> Result<Vec<CallArg>, Diagnostic> {
        let mut args = Vec::new();
        if !self.check(&TokenKind::RParen) {
            loop {
                let position = self.current().position;
                let arg = if self.looks_like_named_call_arg() {
                    let name = self.expect_identifier("expected a parameter name")?;
                    self.expect_simple(TokenKind::Assign, "expected `=` after parameter name")?;
                    let value = self.parse_expr()?;
                    CallArg {
                        name: Some(name),
                        value,
                        position,
                    }
                } else {
                    let value = self.parse_expr()?;
                    CallArg {
                        name: None,
                        position: value.position(),
                        value,
                    }
                };
                args.push(arg);
                if !self.matches_simple(&TokenKind::Comma) {
                    break;
                }
            }
        }
        Ok(args)
    }

    fn parse_name(&mut self) -> Result<Name, Diagnostic> {
        let mut segments = vec![self.expect_identifier("expected an identifier")?];
        while self.matches_simple(&TokenKind::Dot) {
            segments.push(self.expect_identifier("expected an identifier after `.`")?);
        }
        let mut name = Name { segments };
        if let Some(target) = self.package_aliases.get(&name.segments[0]) {
            let mut canonical_segments = target.segments.clone();
            canonical_segments.extend(name.segments.into_iter().skip(1));
            name = Name {
                segments: canonical_segments,
            };
        }
        Ok(name)
    }

    fn parse_context_alias(&mut self) -> Result<Option<String>, Diagnostic> {
        if self.matches_simple(&TokenKind::As) {
            return Ok(Some(
                self.expect_identifier("expected an alias after `as`")?,
            ));
        }

        Ok(None)
    }

    fn canonicalize_package_alias_expr(&self, expr: Expr) -> Expr {
        match expr {
            Expr::Name { mut name, position } => {
                if let Some(target) = self.package_aliases.get(&name.segments[0]) {
                    let mut canonical_segments = target.segments.clone();
                    canonical_segments.extend(name.segments.into_iter().skip(1));
                    name = Name {
                        segments: canonical_segments,
                    };
                }
                Expr::Name { name, position }
            }
            _ => expr,
        }
    }

    fn looks_like_local_decl(&self) -> bool {
        let mut cursor = self.index;
        if self.kind_at(cursor) == Some(&TokenKind::Const) {
            cursor += 1;
        }

        cursor = match self.advance_name(cursor) {
            Some(cursor) => cursor,
            None => return false,
        };

        if !matches!(self.kind_at(cursor), Some(TokenKind::Identifier(_))) {
            return false;
        }
        cursor += 1;

        matches!(
            self.kind_at(cursor),
            Some(TokenKind::Assign | TokenKind::Semicolon)
        )
    }

    fn looks_like_named_call_arg(&self) -> bool {
        matches!(self.current_kind(), TokenKind::Identifier(_))
            && self.kind_at(self.index + 1) == Some(&TokenKind::Assign)
    }

    fn advance_name(&self, mut cursor: usize) -> Option<usize> {
        if !matches!(self.kind_at(cursor), Some(TokenKind::Identifier(_))) {
            return None;
        }
        cursor += 1;
        while self.kind_at(cursor) == Some(&TokenKind::Dot) {
            cursor += 1;
            if !matches!(self.kind_at(cursor), Some(TokenKind::Identifier(_))) {
                return None;
            }
            cursor += 1;
        }
        Some(cursor)
    }

    fn expect_identifier(&mut self, message: &str) -> Result<String, Diagnostic> {
        match self.current_kind() {
            TokenKind::Identifier(value) => {
                let value = value.clone();
                self.bump();
                Ok(value)
            }
            _ => Err(self.error_here(message)),
        }
    }

    fn expect_identifier_text(&mut self, expected: &str, message: &str) -> Result<(), Diagnostic> {
        match self.current_kind() {
            TokenKind::Identifier(value) if value == expected => {
                self.bump();
                Ok(())
            }
            _ => Err(self.error_here(message)),
        }
    }

    fn matches_simple(&mut self, expected: &TokenKind) -> bool {
        if self.check(expected) {
            self.bump();
            true
        } else {
            false
        }
    }

    fn expect_simple(&mut self, expected: TokenKind, message: &str) -> Result<(), Diagnostic> {
        if self.check(&expected) {
            self.bump();
            Ok(())
        } else {
            Err(self.error_here(message))
        }
    }

    fn check(&self, expected: &TokenKind) -> bool {
        self.current_kind() == expected
    }

    fn current(&self) -> &Token {
        &self.tokens[self.index]
    }

    fn current_kind(&self) -> &TokenKind {
        &self.current().kind
    }

    fn kind_at(&self, index: usize) -> Option<&TokenKind> {
        self.tokens.get(index).map(|token| &token.kind)
    }

    fn bump(&mut self) {
        if !self.is_eof() {
            self.index += 1;
        }
    }

    fn previous_position(&self) -> Position {
        self.tokens[self.index.saturating_sub(1)].position
    }

    fn is_eof(&self) -> bool {
        matches!(self.current_kind(), TokenKind::Eof)
    }

    fn error_here(&self, message: &str) -> Diagnostic {
        Diagnostic::new(message, self.current().position)
    }
}

fn expr_to_name(expr: &Expr) -> Option<Name> {
    match expr {
        Expr::Name { name, .. } => Some(name.clone()),
        Expr::Member { base, member, .. } => {
            let mut name = expr_to_name(base)?;
            name.segments.push(member.clone());
            Some(name)
        }
        _ => None,
    }
}

fn depend_target_key(target: &DependTarget) -> String {
    match target {
        DependTarget::Null => "null".to_string(),
        DependTarget::Result => "result".to_string(),
        DependTarget::Name(name) => name.as_string(),
    }
}
