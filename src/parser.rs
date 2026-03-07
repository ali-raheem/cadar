use crate::{
    ast::{
        BinaryOp, Block, BlockItem, CaseArm, CaseStatement, ElseIfBranch, EnumType, Expr,
        ForStatement, IfStatement, Item, LocalDecl, Name, Package, PackageItem, Param, ParamMode,
        Program, RangeType, RecordField, RecordType, Statement, StatementBlock, Subprogram,
        TypeDecl, UnaryOp,
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
}

impl<'a> Parser<'a> {
    fn new(tokens: &'a [Token]) -> Self {
        Self { tokens, index: 0 }
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
                self.bump();
                let name = self.parse_name()?;
                self.expect_simple(TokenKind::Semicolon, "expected `;` after import")?;
                Ok(Item::Import(name))
            }
            TokenKind::Use => {
                self.bump();
                let name = self.parse_name()?;
                self.expect_simple(TokenKind::Semicolon, "expected `;` after use")?;
                Ok(Item::Use(name))
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
        while !self.check(&TokenKind::RBrace) {
            items.push(self.parse_package_item()?);
        }

        self.expect_simple(TokenKind::RBrace, "expected `}` after package body")?;

        Ok(Package {
            is_body,
            name,
            items,
            position,
        })
    }

    fn parse_package_item(&mut self) -> Result<PackageItem, Diagnostic> {
        match self.current_kind() {
            TokenKind::Fn => Ok(PackageItem::Subprogram(self.parse_subprogram()?)),
            TokenKind::Type => Ok(PackageItem::Type(self.parse_type_decl()?)),
            TokenKind::Enum => Ok(PackageItem::Type(self.parse_enum_decl()?)),
            _ => Err(self.error_here("expected `fn`, `type`, or `enum` inside package")),
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
        while matches!(
            self.current_kind(),
            TokenKind::Requires | TokenKind::Ensures
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
                _ => unreachable!("contract loop should only match requires/ensures"),
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

    fn parse_param_group(&mut self, mode: ParamMode) -> Result<Vec<Param>, Diagnostic> {
        let mut params = Vec::new();
        loop {
            let ty = self.parse_name()?;
            let name = self.expect_identifier("expected a parameter name")?;
            params.push(Param { mode, ty, name });

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
            TokenKind::Return => {
                self.bump();
                let expr = self.parse_expr()?;
                self.expect_simple(TokenKind::Semicolon, "expected `;` after return")?;
                Ok(Statement::Return { expr, position })
            }
            TokenKind::For => self.parse_for_statement(),
            TokenKind::If => self.parse_if_statement(),
            TokenKind::Case => self.parse_case_statement(),
            TokenKind::While => self.parse_while_statement(),
            TokenKind::Identifier(_) => {
                if self.looks_like_assignment() {
                    let target = self.parse_name()?;
                    self.expect_simple(TokenKind::Assign, "expected `=` in assignment")?;
                    let value = self.parse_expr()?;
                    self.expect_simple(TokenKind::Semicolon, "expected `;` after assignment")?;
                    Ok(Statement::Assign {
                        target,
                        value,
                        position,
                    })
                } else {
                    let expr = self.parse_expr()?;
                    self.expect_simple(TokenKind::Semicolon, "expected `;` after expression")?;
                    Ok(Statement::Expr { expr, position })
                }
            }
            _ => Err(self.error_here("expected a statement")),
        }
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
        let body = self.parse_statement_block()?;
        Ok(Statement::While {
            condition,
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
        let body = self.parse_statement_block()?;

        Ok(Statement::For(ForStatement {
            iterator_type,
            iterator,
            start,
            end,
            body,
            position,
        }))
    }

    fn parse_statement_block(&mut self) -> Result<StatementBlock, Diagnostic> {
        self.expect_simple(TokenKind::LBrace, "expected `{`")?;
        let mut statements = Vec::new();
        while !self.check(&TokenKind::RBrace) {
            statements.push(self.parse_statement()?);
        }
        self.expect_simple(TokenKind::RBrace, "expected `}`")?;
        Ok(StatementBlock { statements })
    }

    fn parse_expr(&mut self) -> Result<Expr, Diagnostic> {
        self.parse_or()
    }

    fn parse_or(&mut self) -> Result<Expr, Diagnostic> {
        let mut expr = self.parse_and()?;
        loop {
            if !self.matches_simple(&TokenKind::Or) {
                return Ok(expr);
            }

            let op = if self.matches_simple(&TokenKind::Else) {
                BinaryOp::ShortCircuitOr
            } else {
                BinaryOp::Or
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
            if !self.matches_simple(&TokenKind::And) {
                return Ok(expr);
            }

            let op = if self.matches_simple(&TokenKind::Then) {
                BinaryOp::ShortCircuitAnd
            } else {
                BinaryOp::And
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
                let position = expr.position();
                let member = self.expect_identifier("expected an identifier after `.`")?;
                expr = Expr::Member {
                    base: Box::new(expr),
                    member,
                    position,
                };
                continue;
            }

            if self.matches_simple(&TokenKind::LParen) {
                let position = expr.position();
                let mut args = Vec::new();
                if !self.check(&TokenKind::RParen) {
                    loop {
                        args.push(self.parse_expr()?);
                        if !self.matches_simple(&TokenKind::Comma) {
                            break;
                        }
                    }
                }
                self.expect_simple(TokenKind::RParen, "expected `)` after arguments")?;
                expr = Expr::Call {
                    callee: Box::new(expr),
                    args,
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
            TokenKind::LParen => {
                self.bump();
                let expr = self.parse_expr()?;
                self.expect_simple(TokenKind::RParen, "expected `)` after expression")?;
                Ok(expr)
            }
            _ => Err(self.error_here("expected an expression")),
        }
    }

    fn parse_name(&mut self) -> Result<Name, Diagnostic> {
        let mut segments = vec![self.expect_identifier("expected an identifier")?];
        while self.matches_simple(&TokenKind::Dot) {
            segments.push(self.expect_identifier("expected an identifier after `.`")?);
        }
        Ok(Name { segments })
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

    fn looks_like_assignment(&self) -> bool {
        let Some(cursor) = self.advance_name(self.index) else {
            return false;
        };
        if self.name_ends_with_attribute(cursor) {
            return false;
        }
        matches!(self.kind_at(cursor), Some(TokenKind::Assign))
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

    fn name_ends_with_attribute(&self, end: usize) -> bool {
        let last_identifier = end.checked_sub(1).and_then(|index| self.kind_at(index));
        match last_identifier {
            Some(TokenKind::Identifier(name)) => {
                matches!(name.as_str(), "length" | "range" | "image")
            }
            _ => false,
        }
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
