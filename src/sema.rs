use std::collections::{HashMap, HashSet};

use crate::{
    ast::{
        BinaryOp, BlockItem, Expr, Item, Name, Package, PackageItem, ParamMode, Program, Statement,
        StatementBlock, Subprogram, TypeDecl, UnaryOp,
    },
    diagnostic::{Diagnostic, Position},
};

type SignatureIndex = HashMap<String, Vec<SubprogramSignature>>;

pub fn validate(program: &Program) -> Result<(), Diagnostic> {
    let summary = ProgramSummary::collect(program)?;
    summary.validate_package_consistency()?;
    Validator::new(&summary).validate_program(program)
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct SignatureKey {
    name: String,
    params: Vec<(ParamMode, String)>,
    return_type: Option<String>,
}

#[derive(Debug, Clone)]
struct SubprogramSignature {
    key: SignatureKey,
    position: Position,
}

impl SubprogramSignature {
    fn from_subprogram(subprogram: &Subprogram) -> Self {
        Self {
            key: SignatureKey {
                name: subprogram.name.clone(),
                params: subprogram
                    .params
                    .iter()
                    .map(|param| (param.mode, param.ty.as_string()))
                    .collect(),
                return_type: subprogram.return_type.as_ref().map(Name::as_string),
            },
            position: subprogram.position,
        }
    }

    fn arity(&self) -> usize {
        self.key.params.len()
    }
}

#[derive(Debug, Default)]
struct PackageSummary {
    has_spec: bool,
    has_body: bool,
    spec_subprograms: SignatureIndex,
    body_subprograms: SignatureIndex,
}

impl PackageSummary {
    fn visible_subprograms(&self) -> &SignatureIndex {
        if self.has_spec {
            &self.spec_subprograms
        } else {
            &self.body_subprograms
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum TypeKind {
    Record,
    Enum,
    Range { base: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum InferredType {
    Known(String),
    Procedure,
    Unknown,
}

#[derive(Debug, Default)]
struct ProgramSummary {
    imports: HashSet<String>,
    uses: HashSet<String>,
    top_level_declarations: SignatureIndex,
    top_level_definitions: SignatureIndex,
    top_level_types: HashSet<String>,
    top_level_enum_literals: HashSet<String>,
    top_level_type_kinds: HashMap<String, TypeKind>,
    top_level_enum_literal_types: HashMap<String, Vec<String>>,
    packages: HashMap<String, PackageSummary>,
    package_types: HashMap<String, HashSet<String>>,
    package_enum_literals: HashMap<String, HashSet<String>>,
    package_type_kinds: HashMap<String, HashMap<String, TypeKind>>,
    package_enum_literal_types: HashMap<String, HashMap<String, Vec<String>>>,
}

impl ProgramSummary {
    fn collect(program: &Program) -> Result<Self, Diagnostic> {
        let mut summary = Self::default();
        let mut top_level_types = HashSet::new();

        for item in &program.items {
            match item {
                Item::Import(name) => {
                    summary.imports.insert(name.as_string());
                }
                Item::Use(name) => {
                    summary.uses.insert(name.as_string());
                }
                Item::Subprogram(subprogram) => {
                    let signature = SubprogramSignature::from_subprogram(subprogram);
                    let scope = if subprogram.body.is_some() {
                        &mut summary.top_level_definitions
                    } else {
                        &mut summary.top_level_declarations
                    };
                    insert_signature(
                        scope,
                        signature,
                        subprogram.position,
                        "duplicate top-level subprogram signature",
                    )?;
                }
                Item::Type(type_decl) => {
                    validate_type_decl(type_decl, &mut top_level_types, "top-level")?;
                    collect_type_symbols(
                        type_decl,
                        &mut summary.top_level_types,
                        &mut summary.top_level_enum_literals,
                    );
                    collect_type_metadata(
                        type_decl,
                        &mut summary.top_level_type_kinds,
                        &mut summary.top_level_enum_literal_types,
                    );
                }
                Item::Package(package) => {
                    let package_name = package.name.as_string();
                    let entry = summary.packages.entry(package_name.clone()).or_default();
                    let package_types = summary
                        .package_types
                        .entry(package_name.clone())
                        .or_default();
                    let package_enum_literals = summary
                        .package_enum_literals
                        .entry(package_name.clone())
                        .or_default();
                    let package_type_kinds = summary
                        .package_type_kinds
                        .entry(package_name.clone())
                        .or_default();
                    let package_enum_literal_types = summary
                        .package_enum_literal_types
                        .entry(package_name.clone())
                        .or_default();

                    if package.is_body {
                        if entry.has_body {
                            return Err(Diagnostic::new(
                                format!("duplicate package body `{package_name}`"),
                                package.position,
                            ));
                        }
                        entry.has_body = true;
                        collect_package_items(
                            package,
                            &mut entry.body_subprograms,
                            package_types,
                            package_enum_literals,
                            package_type_kinds,
                            package_enum_literal_types,
                            &format!("package body `{package_name}`"),
                        )?;
                    } else {
                        if entry.has_spec {
                            return Err(Diagnostic::new(
                                format!("duplicate package specification `{package_name}`"),
                                package.position,
                            ));
                        }
                        entry.has_spec = true;
                        collect_package_items(
                            package,
                            &mut entry.spec_subprograms,
                            package_types,
                            package_enum_literals,
                            package_type_kinds,
                            package_enum_literal_types,
                            &format!("package `{package_name}`"),
                        )?;
                    }
                }
            }
        }

        Ok(summary)
    }

    fn validate_package_consistency(&self) -> Result<(), Diagnostic> {
        for (package_name, package) in &self.packages {
            if !(package.has_spec && package.has_body) {
                continue;
            }

            for body_signature in iter_signatures(&package.body_subprograms) {
                if !contains_signature(&package.spec_subprograms, &body_signature.key) {
                    return Err(Diagnostic::new(
                        format!(
                            "package body `{package_name}` defines `{}` without a matching declaration",
                            body_signature.key.name
                        ),
                        body_signature.position,
                    ));
                }
            }

            for spec_signature in iter_signatures(&package.spec_subprograms) {
                if !contains_signature(&package.body_subprograms, &spec_signature.key) {
                    return Err(Diagnostic::new(
                        format!(
                            "package body `{package_name}` is missing a definition for `{}`",
                            spec_signature.key.name
                        ),
                        spec_signature.position,
                    ));
                }
            }
        }

        Ok(())
    }
}

#[derive(Debug, Clone, Default)]
struct Scope {
    values: HashMap<String, String>,
}

impl Scope {
    fn with_value(&self, name: &str, ty: &str) -> Self {
        let mut scope = self.clone();
        scope.values.insert(name.to_string(), ty.to_string());
        scope
    }

    fn insert_value(&mut self, name: &str, ty: &str) -> Option<String> {
        self.values.insert(name.to_string(), ty.to_string())
    }

    fn contains_value(&self, name: &str) -> bool {
        self.values.contains_key(name)
    }

    fn value_type(&self, name: &str) -> Option<&str> {
        self.values.get(name).map(String::as_str)
    }
}

struct Validator<'a> {
    summary: &'a ProgramSummary,
}

impl<'a> Validator<'a> {
    fn new(summary: &'a ProgramSummary) -> Self {
        Self { summary }
    }

    fn validate_program(&self, program: &Program) -> Result<(), Diagnostic> {
        for item in &program.items {
            match item {
                Item::Subprogram(subprogram) => self.validate_subprogram(subprogram, None)?,
                Item::Package(package) => {
                    let package_name = package.name.as_string();
                    for item in &package.items {
                        if let PackageItem::Subprogram(subprogram) = item {
                            self.validate_subprogram(subprogram, Some(&package_name))?;
                        }
                    }
                }
                Item::Import(_) | Item::Use(_) | Item::Type(_) => {}
            }
        }

        Ok(())
    }

    fn validate_subprogram(
        &self,
        subprogram: &Subprogram,
        current_package: Option<&str>,
    ) -> Result<(), Diagnostic> {
        let mut scope = Scope::default();
        for param in &subprogram.params {
            if scope
                .insert_value(&param.name, &param.ty.as_string())
                .is_some()
            {
                return Err(Diagnostic::new(
                    format!("duplicate parameter `{}`", param.name),
                    subprogram.position,
                ));
            }
        }

        for require in &subprogram.requires {
            self.validate_contract_expr(
                require,
                "precondition",
                None,
                false,
                &scope,
                current_package,
            )?;
        }

        for ensure in &subprogram.ensures {
            self.validate_contract_expr(
                ensure,
                "postcondition",
                subprogram.return_type.as_ref(),
                true,
                &scope,
                current_package,
            )?;
        }

        let Some(body) = &subprogram.body else {
            return Ok(());
        };

        for item in &body.items {
            match item {
                BlockItem::LocalDecl(decl) => {
                    if let Some(initializer) = &decl.initializer {
                        self.validate_expr(initializer, &scope, current_package)?;
                        self.validate_value_type(
                            initializer,
                            Some(&decl.ty.as_string()),
                            "local initializer",
                            decl.position,
                            &scope,
                            current_package,
                        )?;
                    }
                    if scope
                        .insert_value(&decl.name, &decl.ty.as_string())
                        .is_some()
                    {
                        return Err(Diagnostic::new(
                            format!("duplicate local declaration `{}`", decl.name),
                            decl.position,
                        ));
                    }
                }
                BlockItem::Statement(statement) => {
                    self.validate_statement(
                        statement,
                        subprogram.return_type.as_ref(),
                        &scope,
                        current_package,
                    )?;
                }
            }
        }

        Ok(())
    }

    fn validate_statement(
        &self,
        statement: &Statement,
        expected_return_type: Option<&Name>,
        scope: &Scope,
        current_package: Option<&str>,
    ) -> Result<(), Diagnostic> {
        match statement {
            Statement::Null { .. } => Ok(()),
            Statement::Return { expr, position } => {
                self.validate_expr(expr, scope, current_package)?;
                let Some(expected_return_type) = expected_return_type else {
                    return Err(Diagnostic::new(
                        "procedures cannot return a value",
                        *position,
                    ));
                };
                self.validate_value_type(
                    expr,
                    Some(&expected_return_type.as_string()),
                    "return expression",
                    *position,
                    scope,
                    current_package,
                )
            }
            Statement::Expr { expr, position } => {
                self.validate_expr(expr, scope, current_package)?;
                if !matches!(expr, Expr::Call { .. }) {
                    return Err(Diagnostic::new(
                        "only call expressions are allowed as standalone statements",
                        *position,
                    ));
                }
                if matches!(
                    self.infer_expr_type(expr, scope, current_package)?,
                    InferredType::Known(_)
                ) {
                    return Err(Diagnostic::new(
                        "function calls cannot be used as standalone statements",
                        *position,
                    ));
                }
                Ok(())
            }
            Statement::Assign {
                target,
                value,
                position,
            } => {
                self.validate_assignment_target(target, *position, scope, current_package)?;
                self.validate_expr(value, scope, current_package)?;
                let expected_type =
                    self.lookup_assignment_target_type(target, scope, current_package);
                self.validate_value_type(
                    value,
                    expected_type.as_deref(),
                    "assignment",
                    *position,
                    scope,
                    current_package,
                )
            }
            Statement::If(if_statement) => {
                self.validate_expr(&if_statement.condition, scope, current_package)?;
                self.validate_boolean_expr(
                    &if_statement.condition,
                    "if condition",
                    if_statement.position,
                    scope,
                    current_package,
                )?;
                self.validate_statement_block(
                    &if_statement.then_branch,
                    expected_return_type,
                    scope,
                    current_package,
                )?;
                for branch in &if_statement.else_if_branches {
                    self.validate_expr(&branch.condition, scope, current_package)?;
                    self.validate_boolean_expr(
                        &branch.condition,
                        "else-if condition",
                        branch.position,
                        scope,
                        current_package,
                    )?;
                    self.validate_statement_block(
                        &branch.body,
                        expected_return_type,
                        scope,
                        current_package,
                    )?;
                }
                if let Some(else_branch) = &if_statement.else_branch {
                    self.validate_statement_block(
                        else_branch,
                        expected_return_type,
                        scope,
                        current_package,
                    )?;
                }
                Ok(())
            }
            Statement::Case(case_statement) => {
                self.validate_expr(&case_statement.expr, scope, current_package)?;
                let case_type =
                    self.infer_expr_type(&case_statement.expr, scope, current_package)?;
                self.validate_case_expr_type(&case_type, case_statement.position, current_package)?;
                for arm in &case_statement.arms {
                    for choice in &arm.choices {
                        self.validate_expr(choice, scope, current_package)?;
                        self.validate_case_choice_type(
                            &case_type,
                            choice,
                            arm.position,
                            scope,
                            current_package,
                        )?;
                    }
                    self.validate_statement_block(
                        &arm.body,
                        expected_return_type,
                        scope,
                        current_package,
                    )?;
                }
                if let Some(else_arm) = &case_statement.else_arm {
                    self.validate_statement_block(
                        else_arm,
                        expected_return_type,
                        scope,
                        current_package,
                    )?;
                }
                Ok(())
            }
            Statement::While {
                condition, body, ..
            } => {
                self.validate_expr(condition, scope, current_package)?;
                self.validate_boolean_expr(
                    condition,
                    "while condition",
                    condition.position(),
                    scope,
                    current_package,
                )?;
                self.validate_statement_block(body, expected_return_type, scope, current_package)
            }
            Statement::For(for_statement) => {
                self.validate_expr(&for_statement.start, scope, current_package)?;
                self.validate_expr(&for_statement.end, scope, current_package)?;
                self.validate_value_type(
                    &for_statement.start,
                    Some(&for_statement.iterator_type.as_string()),
                    "for-loop start",
                    for_statement.position,
                    scope,
                    current_package,
                )?;
                self.validate_value_type(
                    &for_statement.end,
                    Some(&for_statement.iterator_type.as_string()),
                    "for-loop end",
                    for_statement.position,
                    scope,
                    current_package,
                )?;
                let loop_scope = scope.with_value(
                    &for_statement.iterator,
                    &for_statement.iterator_type.as_string(),
                );
                self.validate_statement_block(
                    &for_statement.body,
                    expected_return_type,
                    &loop_scope,
                    current_package,
                )
            }
        }
    }

    fn validate_statement_block(
        &self,
        block: &StatementBlock,
        expected_return_type: Option<&Name>,
        scope: &Scope,
        current_package: Option<&str>,
    ) -> Result<(), Diagnostic> {
        for statement in &block.statements {
            self.validate_statement(statement, expected_return_type, scope, current_package)?;
        }
        Ok(())
    }

    fn validate_contract_expr(
        &self,
        expr: &Expr,
        context: &str,
        result_type: Option<&Name>,
        allow_result: bool,
        scope: &Scope,
        current_package: Option<&str>,
    ) -> Result<(), Diagnostic> {
        if !allow_result {
            if let Some(position) = find_result_reference(expr) {
                return Err(Diagnostic::new(
                    "`result` is only valid in postconditions of functions",
                    position,
                ));
            }
            self.validate_expr(expr, scope, current_package)?;
            return self.validate_boolean_expr(
                expr,
                context,
                expr.position(),
                scope,
                current_package,
            );
        }

        let Some(result_type) = result_type else {
            if let Some(position) = find_result_reference(expr) {
                return Err(Diagnostic::new(
                    "`result` is only valid in postconditions of functions",
                    position,
                ));
            }
            self.validate_expr(expr, scope, current_package)?;
            return self.validate_boolean_expr(
                expr,
                context,
                expr.position(),
                scope,
                current_package,
            );
        };

        let contract_scope = scope.with_value("result", &result_type.as_string());
        self.validate_expr(expr, &contract_scope, current_package)?;
        self.validate_boolean_expr(
            expr,
            context,
            expr.position(),
            &contract_scope,
            current_package,
        )
    }

    fn validate_value_type(
        &self,
        expr: &Expr,
        expected_type: Option<&str>,
        context: &str,
        position: Position,
        scope: &Scope,
        current_package: Option<&str>,
    ) -> Result<(), Diagnostic> {
        let inferred = self.infer_value_type(expr, scope, current_package)?;
        let Some(expected_type) = expected_type else {
            return Ok(());
        };
        let InferredType::Known(actual_type) = inferred else {
            return Ok(());
        };
        if self.types_are_compatible(expected_type, &actual_type, current_package) {
            return Ok(());
        }

        Err(Diagnostic::new(
            format!("{context} expects `{expected_type}`, found `{actual_type}`"),
            position,
        ))
    }

    fn validate_boolean_expr(
        &self,
        expr: &Expr,
        context: &str,
        position: Position,
        scope: &Scope,
        current_package: Option<&str>,
    ) -> Result<(), Diagnostic> {
        let inferred = self.infer_value_type(expr, scope, current_package)?;
        let InferredType::Known(actual_type) = inferred else {
            return Ok(());
        };

        match self.is_boolean_type(&actual_type, current_package) {
            Some(true) | None => Ok(()),
            Some(false) => Err(Diagnostic::new(
                format!("{context} must be Boolean, found `{actual_type}`"),
                position,
            )),
        }
    }

    fn validate_case_expr_type(
        &self,
        case_type: &InferredType,
        position: Position,
        current_package: Option<&str>,
    ) -> Result<(), Diagnostic> {
        match case_type {
            InferredType::Procedure => Err(Diagnostic::new(
                "procedures do not produce a value",
                position,
            )),
            InferredType::Known(actual_type) => {
                match self.is_discrete_type(actual_type, current_package) {
                    Some(true) | None => Ok(()),
                    Some(false) => Err(Diagnostic::new(
                        format!("case expression must be discrete, found `{actual_type}`"),
                        position,
                    )),
                }
            }
            InferredType::Unknown => Ok(()),
        }
    }

    fn validate_case_choice_type(
        &self,
        case_type: &InferredType,
        choice: &Expr,
        position: Position,
        scope: &Scope,
        current_package: Option<&str>,
    ) -> Result<(), Diagnostic> {
        let choice_type = self.infer_value_type(choice, scope, current_package)?;
        let (InferredType::Known(case_type), InferredType::Known(choice_type)) =
            (case_type, choice_type)
        else {
            return Ok(());
        };
        if self.types_are_compatible(case_type, &choice_type, current_package) {
            return Ok(());
        }

        Err(Diagnostic::new(
            format!("case choice must match `{case_type}`, found `{choice_type}`"),
            position,
        ))
    }

    fn lookup_assignment_target_type(
        &self,
        target: &Name,
        scope: &Scope,
        _current_package: Option<&str>,
    ) -> Option<String> {
        if target.segments.len() != 1 {
            return None;
        }

        scope.value_type(&target.segments[0]).map(str::to_string)
    }

    fn infer_value_type(
        &self,
        expr: &Expr,
        scope: &Scope,
        current_package: Option<&str>,
    ) -> Result<InferredType, Diagnostic> {
        let inferred = self.infer_expr_type(expr, scope, current_package)?;
        if matches!(inferred, InferredType::Procedure) {
            return Err(Diagnostic::new(
                "procedures do not produce a value",
                expr.position(),
            ));
        }
        Ok(inferred)
    }

    fn infer_expr_type(
        &self,
        expr: &Expr,
        scope: &Scope,
        current_package: Option<&str>,
    ) -> Result<InferredType, Diagnostic> {
        match expr {
            Expr::Bool { .. } => Ok(InferredType::Known("Boolean".to_string())),
            Expr::Integer { .. } => Ok(InferredType::Known("Integer".to_string())),
            Expr::String { .. } => Ok(InferredType::Known("String".to_string())),
            Expr::Name { name, .. } => Ok(self.infer_name_type(name, scope, current_package)),
            Expr::Member { base, member, .. } => {
                Ok(self.infer_member_type(base, member, current_package))
            }
            Expr::Call { callee, args, .. } => {
                Ok(self.infer_call_type(callee, args.len(), current_package))
            }
            Expr::Unary { op, expr, position } => {
                self.infer_unary_expr_type(*op, expr, *position, scope, current_package)
            }
            Expr::Binary {
                lhs,
                op,
                rhs,
                position,
            } => self.infer_binary_expr_type(lhs, *op, rhs, *position, scope, current_package),
        }
    }

    fn infer_name_type(
        &self,
        name: &Name,
        scope: &Scope,
        current_package: Option<&str>,
    ) -> InferredType {
        if name.segments.len() == 1 {
            if let Some(ty) = scope.value_type(&name.segments[0]) {
                return InferredType::Known(ty.to_string());
            }

            let literal_types = self.visible_enum_literal_types(&name.segments[0], current_package);
            return match literal_types.as_slice() {
                [ty] => InferredType::Known(ty.clone()),
                _ => InferredType::Unknown,
            };
        }

        let Some((package_name, member_name)) = split_qualified_type_str(&name.as_string()) else {
            return InferredType::Unknown;
        };
        match self
            .package_enum_literal_types(&package_name, &member_name)
            .as_slice()
        {
            [ty] => InferredType::Known(ty.clone()),
            _ => InferredType::Unknown,
        }
    }

    fn infer_member_type(
        &self,
        base: &Expr,
        member: &str,
        _current_package: Option<&str>,
    ) -> InferredType {
        if member == "length" {
            return InferredType::Known("Integer".to_string());
        }

        if member == "range" {
            return InferredType::Unknown;
        }

        let Some(package_name) = expr_to_name(base).map(|name| name.as_string()) else {
            return InferredType::Unknown;
        };
        match self
            .package_enum_literal_types(&package_name, member)
            .as_slice()
        {
            [ty] => InferredType::Known(ty.clone()),
            _ => InferredType::Unknown,
        }
    }

    fn infer_call_type(
        &self,
        callee: &Expr,
        arg_count: usize,
        current_package: Option<&str>,
    ) -> InferredType {
        if self.is_image_attribute_call(callee, current_package) {
            return if arg_count == 1 {
                InferredType::Known("String".to_string())
            } else {
                InferredType::Unknown
            };
        }

        let candidates = self.resolve_call_candidates(callee, current_package);
        if !candidates.is_empty() {
            let matching: Vec<_> = candidates
                .into_iter()
                .filter(|candidate| candidate.arity() == arg_count)
                .collect();

            let mut return_types = Vec::new();
            let mut saw_procedure = false;
            for signature in matching {
                match &signature.key.return_type {
                    Some(return_type) => {
                        if !return_types.contains(return_type) {
                            return_types.push(return_type.clone());
                        }
                    }
                    None => saw_procedure = true,
                }
            }

            return match (saw_procedure, return_types.as_slice()) {
                (true, []) => InferredType::Procedure,
                (false, [return_type]) => InferredType::Known(return_type.clone()),
                _ => InferredType::Unknown,
            };
        }

        if self.is_visible_type_expr(callee, current_package) && arg_count == 1 {
            return expr_to_name(callee)
                .map(|name| InferredType::Known(name.as_string()))
                .unwrap_or(InferredType::Unknown);
        }

        InferredType::Unknown
    }

    fn infer_unary_expr_type(
        &self,
        op: UnaryOp,
        expr: &Expr,
        position: Position,
        scope: &Scope,
        current_package: Option<&str>,
    ) -> Result<InferredType, Diagnostic> {
        let operand = self.infer_expr_type(expr, scope, current_package)?;
        match op {
            UnaryOp::Negate => {
                self.ensure_numeric_operand(
                    &operand,
                    position,
                    unary_op_text(op),
                    current_package,
                )?;
                Ok(match operand {
                    InferredType::Known(ty) => InferredType::Known(ty),
                    InferredType::Unknown | InferredType::Procedure => InferredType::Unknown,
                })
            }
            UnaryOp::Not => {
                self.ensure_boolean_operand(
                    &operand,
                    position,
                    unary_op_text(op),
                    current_package,
                )?;
                Ok(InferredType::Known("Boolean".to_string()))
            }
        }
    }

    fn infer_binary_expr_type(
        &self,
        lhs: &Expr,
        op: BinaryOp,
        rhs: &Expr,
        position: Position,
        scope: &Scope,
        current_package: Option<&str>,
    ) -> Result<InferredType, Diagnostic> {
        let lhs_type = self.infer_expr_type(lhs, scope, current_package)?;
        let rhs_type = self.infer_expr_type(rhs, scope, current_package)?;

        match op {
            BinaryOp::Add | BinaryOp::Subtract | BinaryOp::Multiply | BinaryOp::Divide => {
                self.ensure_numeric_operand(
                    &lhs_type,
                    position,
                    binary_op_text(op),
                    current_package,
                )?;
                self.ensure_numeric_operand(
                    &rhs_type,
                    position,
                    binary_op_text(op),
                    current_package,
                )?;
                Ok(match (&lhs_type, &rhs_type) {
                    (InferredType::Known(lhs_ty), InferredType::Known(rhs_ty)) => {
                        if self.is_float_type(lhs_ty, current_package) == Some(true)
                            || self.is_float_type(rhs_ty, current_package) == Some(true)
                        {
                            InferredType::Known("Float".to_string())
                        } else {
                            InferredType::Known(lhs_ty.clone())
                        }
                    }
                    _ => InferredType::Unknown,
                })
            }
            BinaryOp::And | BinaryOp::ShortCircuitAnd | BinaryOp::Or | BinaryOp::ShortCircuitOr => {
                self.ensure_boolean_operand(
                    &lhs_type,
                    position,
                    binary_op_text(op),
                    current_package,
                )?;
                self.ensure_boolean_operand(
                    &rhs_type,
                    position,
                    binary_op_text(op),
                    current_package,
                )?;
                Ok(InferredType::Known("Boolean".to_string()))
            }
            BinaryOp::Equal | BinaryOp::NotEqual => {
                self.ensure_comparable_pair(
                    &lhs_type,
                    &rhs_type,
                    position,
                    binary_op_text(op),
                    current_package,
                )?;
                Ok(InferredType::Known("Boolean".to_string()))
            }
            BinaryOp::Less | BinaryOp::LessEqual | BinaryOp::Greater | BinaryOp::GreaterEqual => {
                self.ensure_ordered_pair(
                    &lhs_type,
                    &rhs_type,
                    position,
                    binary_op_text(op),
                    current_package,
                )?;
                Ok(InferredType::Known("Boolean".to_string()))
            }
        }
    }

    fn ensure_numeric_operand(
        &self,
        operand: &InferredType,
        position: Position,
        op_name: &str,
        current_package: Option<&str>,
    ) -> Result<(), Diagnostic> {
        match operand {
            InferredType::Procedure => Err(Diagnostic::new(
                "procedures do not produce a value",
                position,
            )),
            InferredType::Known(ty) => match self.is_numeric_type(ty, current_package) {
                Some(true) | None => Ok(()),
                Some(false) => Err(Diagnostic::new(
                    format!("`{op_name}` requires numeric operands, found `{ty}`"),
                    position,
                )),
            },
            InferredType::Unknown => Ok(()),
        }
    }

    fn ensure_boolean_operand(
        &self,
        operand: &InferredType,
        position: Position,
        op_name: &str,
        current_package: Option<&str>,
    ) -> Result<(), Diagnostic> {
        match operand {
            InferredType::Procedure => Err(Diagnostic::new(
                "procedures do not produce a value",
                position,
            )),
            InferredType::Known(ty) => match self.is_boolean_type(ty, current_package) {
                Some(true) | None => Ok(()),
                Some(false) => Err(Diagnostic::new(
                    format!("`{op_name}` requires Boolean operands, found `{ty}`"),
                    position,
                )),
            },
            InferredType::Unknown => Ok(()),
        }
    }

    fn ensure_comparable_pair(
        &self,
        lhs: &InferredType,
        rhs: &InferredType,
        position: Position,
        op_name: &str,
        current_package: Option<&str>,
    ) -> Result<(), Diagnostic> {
        self.ensure_comparable_operand(lhs, position, op_name, current_package)?;
        self.ensure_comparable_operand(rhs, position, op_name, current_package)?;

        let (InferredType::Known(lhs_ty), InferredType::Known(rhs_ty)) = (lhs, rhs) else {
            return Ok(());
        };
        if self.types_are_compatible(lhs_ty, rhs_ty, current_package) {
            return Ok(());
        }

        Err(Diagnostic::new(
            format!("`{op_name}` requires compatible operands, found `{lhs_ty}` and `{rhs_ty}`"),
            position,
        ))
    }

    fn ensure_ordered_pair(
        &self,
        lhs: &InferredType,
        rhs: &InferredType,
        position: Position,
        op_name: &str,
        current_package: Option<&str>,
    ) -> Result<(), Diagnostic> {
        self.ensure_orderable_operand(lhs, position, op_name, current_package)?;
        self.ensure_orderable_operand(rhs, position, op_name, current_package)?;

        let (InferredType::Known(lhs_ty), InferredType::Known(rhs_ty)) = (lhs, rhs) else {
            return Ok(());
        };
        if self.types_are_compatible(lhs_ty, rhs_ty, current_package) {
            return Ok(());
        }

        Err(Diagnostic::new(
            format!("`{op_name}` requires compatible operands, found `{lhs_ty}` and `{rhs_ty}`"),
            position,
        ))
    }

    fn ensure_comparable_operand(
        &self,
        operand: &InferredType,
        position: Position,
        op_name: &str,
        current_package: Option<&str>,
    ) -> Result<(), Diagnostic> {
        match operand {
            InferredType::Procedure => Err(Diagnostic::new(
                "procedures do not produce a value",
                position,
            )),
            InferredType::Known(ty) => match self.is_comparable_type(ty, current_package) {
                Some(true) | None => Ok(()),
                Some(false) => Err(Diagnostic::new(
                    format!("`{op_name}` requires comparable operands, found `{ty}`"),
                    position,
                )),
            },
            InferredType::Unknown => Ok(()),
        }
    }

    fn ensure_orderable_operand(
        &self,
        operand: &InferredType,
        position: Position,
        op_name: &str,
        current_package: Option<&str>,
    ) -> Result<(), Diagnostic> {
        match operand {
            InferredType::Procedure => Err(Diagnostic::new(
                "procedures do not produce a value",
                position,
            )),
            InferredType::Known(ty) => match self.is_orderable_type(ty, current_package) {
                Some(true) | None => Ok(()),
                Some(false) => Err(Diagnostic::new(
                    format!("`{op_name}` requires ordered operands, found `{ty}`"),
                    position,
                )),
            },
            InferredType::Unknown => Ok(()),
        }
    }

    fn validate_expr(
        &self,
        expr: &Expr,
        scope: &Scope,
        current_package: Option<&str>,
    ) -> Result<(), Diagnostic> {
        match expr {
            Expr::Bool { .. } | Expr::Integer { .. } | Expr::String { .. } => Ok(()),
            Expr::Name { name, position } => {
                if self.is_visible_value_name(name, scope, current_package) {
                    Ok(())
                } else {
                    Err(Diagnostic::new(
                        format!("undefined identifier `{}`", name.as_string()),
                        *position,
                    ))
                }
            }
            Expr::Member { base, .. } => self.validate_member_base(base, scope, current_package),
            Expr::Call { callee, args, .. } => {
                for arg in args {
                    self.validate_expr(arg, scope, current_package)?;
                    self.infer_value_type(arg, scope, current_package)?;
                }
                self.validate_call(callee, args.len(), scope, current_package)
            }
            expr @ Expr::Unary { expr: inner, .. } => {
                self.validate_expr(inner, scope, current_package)?;
                self.infer_value_type(expr, scope, current_package)?;
                Ok(())
            }
            expr @ Expr::Binary { lhs, rhs, .. } => {
                self.validate_expr(lhs, scope, current_package)?;
                self.validate_expr(rhs, scope, current_package)?;
                self.infer_value_type(expr, scope, current_package)?;
                Ok(())
            }
        }
    }

    fn validate_call(
        &self,
        callee: &Expr,
        arg_count: usize,
        scope: &Scope,
        current_package: Option<&str>,
    ) -> Result<(), Diagnostic> {
        self.validate_callee_reference(callee, scope, current_package)?;

        if self.is_image_attribute_call(callee, current_package) {
            if arg_count == 1 {
                return Ok(());
            }

            let suffix = if arg_count == 1 { "" } else { "s" };
            return Err(Diagnostic::new(
                format!(
                    "attribute call `{}` requires 1 argument, found {arg_count} argument{suffix}",
                    display_callee(callee)
                ),
                callee.position(),
            ));
        }

        let candidates = self.resolve_call_candidates(callee, current_package);
        if !candidates.is_empty() {
            if candidates
                .iter()
                .any(|candidate| candidate.arity() == arg_count)
            {
                return Ok(());
            }

            let suffix = if arg_count == 1 { "" } else { "s" };
            return Err(Diagnostic::new(
                format!(
                    "`{}` does not accept {arg_count} argument{suffix}",
                    display_callee(callee)
                ),
                callee.position(),
            ));
        }

        if self.is_visible_type_expr(callee, current_package) {
            if arg_count == 1 {
                return Ok(());
            }

            let suffix = if arg_count == 1 { "" } else { "s" };
            return Err(Diagnostic::new(
                format!(
                    "type conversion `{}` requires 1 argument, found {arg_count} argument{suffix}",
                    display_callee(callee)
                ),
                callee.position(),
            ));
        }

        if let Expr::Name { name, position } = callee
            && name.segments.len() == 1
            && scope.contains_value(&name.segments[0])
        {
            return Err(Diagnostic::new(
                format!("`{}` is not callable", name.as_string()),
                *position,
            ));
        }

        Ok(())
    }

    fn resolve_call_candidates(
        &self,
        callee: &Expr,
        current_package: Option<&str>,
    ) -> Vec<&SubprogramSignature> {
        match callee {
            Expr::Name { name, .. } if name.segments.len() == 1 => {
                let mut candidates = Vec::new();
                if let Some(package_name) = current_package
                    && let Some(package) = self.summary.packages.get(package_name)
                {
                    extend_candidates(
                        &mut candidates,
                        package.visible_subprograms().get(&name.segments[0]),
                    );
                }
                extend_candidates(
                    &mut candidates,
                    self.summary.top_level_declarations.get(&name.segments[0]),
                );
                extend_candidates(
                    &mut candidates,
                    self.summary.top_level_definitions.get(&name.segments[0]),
                );
                for used_package in self.visible_used_internal_packages() {
                    extend_candidates(
                        &mut candidates,
                        used_package.visible_subprograms().get(&name.segments[0]),
                    );
                }
                candidates
            }
            Expr::Member { base, member, .. } => {
                let Some(package_name) = expr_to_name(base) else {
                    return Vec::new();
                };
                let Some(package) = self.summary.packages.get(&package_name.as_string()) else {
                    return Vec::new();
                };
                let mut candidates = Vec::new();
                extend_candidates(&mut candidates, package.visible_subprograms().get(member));
                candidates
            }
            _ => Vec::new(),
        }
    }

    fn validate_assignment_target(
        &self,
        target: &Name,
        position: Position,
        scope: &Scope,
        current_package: Option<&str>,
    ) -> Result<(), Diagnostic> {
        let Some(first_segment) = target.segments.first() else {
            return Ok(());
        };

        if scope.contains_value(first_segment)
            || self.is_visible_package_name(first_segment, current_package)
            || self.is_visible_type_name(first_segment, current_package)
        {
            return Ok(());
        }

        Err(Diagnostic::new(
            format!("undefined assignment target `{}`", target.as_string()),
            position,
        ))
    }

    fn validate_member_base(
        &self,
        base: &Expr,
        scope: &Scope,
        current_package: Option<&str>,
    ) -> Result<(), Diagnostic> {
        match base {
            Expr::Name { name, position } => {
                if self.is_visible_value_name(name, scope, current_package)
                    || self.is_visible_package_path(name, current_package)
                    || self.is_visible_type_path(name, current_package)
                {
                    Ok(())
                } else {
                    let _ = position;
                    Ok(())
                }
            }
            Expr::Member {
                base: inner_base,
                position,
                ..
            } => {
                if let Some(name) = expr_to_name(base)
                    && (self.is_visible_package_path(&name, current_package)
                        || self.is_visible_type_path(&name, current_package))
                {
                    return Ok(());
                }

                self.validate_member_base(inner_base, scope, current_package)
                    .map_err(|error| {
                        if error.position == Position::START {
                            Diagnostic::new(error.message, *position)
                        } else {
                            error
                        }
                    })
            }
            _ => self.validate_expr(base, scope, current_package),
        }
    }

    fn validate_callee_reference(
        &self,
        callee: &Expr,
        scope: &Scope,
        current_package: Option<&str>,
    ) -> Result<(), Diagnostic> {
        match callee {
            Expr::Name { .. } => Ok(()),
            Expr::Member { base, .. } => self.validate_member_base(base, scope, current_package),
            _ => self.validate_expr(callee, scope, current_package),
        }
    }

    fn is_visible_value_name(
        &self,
        name: &Name,
        scope: &Scope,
        current_package: Option<&str>,
    ) -> bool {
        if name.segments.len() != 1 {
            return false;
        }

        let identifier = &name.segments[0];
        scope.contains_value(identifier)
            || self.summary.top_level_enum_literals.contains(identifier)
            || current_package
                .and_then(|package| self.summary.package_enum_literals.get(package))
                .is_some_and(|literals| literals.contains(identifier))
            || self
                .visible_used_internal_package_enum_literals()
                .into_iter()
                .any(|literals| literals.contains(identifier))
    }

    fn is_visible_package_name(&self, name: &str, current_package: Option<&str>) -> bool {
        if current_package.is_some_and(|package| package == name) {
            return true;
        }

        self.summary.packages.contains_key(name)
            || self.summary.imports.contains(name)
            || self.summary.uses.contains(name)
    }

    fn is_visible_type_name(&self, name: &str, current_package: Option<&str>) -> bool {
        builtin_type_names().contains(&name)
            || self.summary.top_level_types.contains(name)
            || current_package
                .and_then(|package| self.summary.package_types.get(package))
                .is_some_and(|types| types.contains(name))
            || self
                .visible_used_internal_package_types()
                .into_iter()
                .any(|types| types.contains(name))
    }

    fn is_visible_package_path(&self, name: &Name, current_package: Option<&str>) -> bool {
        let package_name = name.as_string();
        self.is_visible_package_name(&package_name, current_package)
    }

    fn is_visible_type_path(&self, name: &Name, current_package: Option<&str>) -> bool {
        if name.segments.len() == 1 {
            return self.is_visible_type_name(&name.segments[0], current_package);
        }

        let Some((package_name, type_name)) = split_qualified_name(name) else {
            return false;
        };
        self.summary
            .package_types
            .get(&package_name)
            .is_some_and(|types| types.contains(&type_name))
    }

    fn is_visible_type_expr(&self, expr: &Expr, current_package: Option<&str>) -> bool {
        let Some(name) = expr_to_name(expr) else {
            return false;
        };
        self.is_visible_type_path(&name, current_package)
    }

    fn is_image_attribute_call(&self, callee: &Expr, current_package: Option<&str>) -> bool {
        let Expr::Member { base, member, .. } = callee else {
            return false;
        };
        member == "image" && self.is_visible_type_expr(base, current_package)
    }

    fn visible_enum_literal_types(
        &self,
        literal: &str,
        current_package: Option<&str>,
    ) -> Vec<String> {
        let mut types = self
            .summary
            .top_level_enum_literal_types
            .get(literal)
            .cloned()
            .unwrap_or_default();

        if let Some(package_name) = current_package {
            types.extend(self.package_enum_literal_types(package_name, literal));
        }

        for package_name in &self.summary.uses {
            types.extend(self.package_enum_literal_types(package_name, literal));
        }

        types.sort();
        types.dedup();
        types
    }

    fn package_enum_literal_types(&self, package_name: &str, literal: &str) -> Vec<String> {
        self.summary
            .package_enum_literal_types
            .get(package_name)
            .and_then(|types| types.get(literal))
            .map(|type_names| {
                type_names
                    .iter()
                    .map(|type_name| format!("{package_name}.{type_name}"))
                    .collect()
            })
            .unwrap_or_default()
    }

    fn types_are_compatible(
        &self,
        expected_type: &str,
        actual_type: &str,
        current_package: Option<&str>,
    ) -> bool {
        let Some(expected_canonical) = self.canonical_type_name(expected_type, current_package)
        else {
            return true;
        };
        let Some(actual_canonical) = self.canonical_type_name(actual_type, current_package) else {
            return true;
        };

        if expected_canonical == actual_canonical {
            return true;
        }

        let expected_root = self.root_canonical_type_name(&expected_canonical);
        let actual_root = self.root_canonical_type_name(&actual_canonical);
        expected_root == actual_root
            || (self.is_numeric_canonical_type(&expected_root)
                && self.is_numeric_canonical_type(&actual_root))
    }

    fn canonical_type_name(&self, ty: &str, current_package: Option<&str>) -> Option<String> {
        if builtin_type_names().contains(&ty) {
            return Some(ty.to_string());
        }

        if let Some((package_name, type_name)) = split_qualified_type_str(ty) {
            return self
                .summary
                .package_types
                .get(&package_name)
                .is_some_and(|types| types.contains(&type_name))
                .then(|| format!("{package_name}.{type_name}"));
        }

        if let Some(package_name) = current_package
            && self
                .summary
                .package_types
                .get(package_name)
                .is_some_and(|types| types.contains(ty))
        {
            return Some(format!("{package_name}.{ty}"));
        }

        if self.summary.top_level_types.contains(ty) {
            return Some(ty.to_string());
        }

        let mut matches = Vec::new();
        for package_name in &self.summary.uses {
            if self
                .summary
                .package_types
                .get(package_name)
                .is_some_and(|types| types.contains(ty))
            {
                matches.push(format!("{package_name}.{ty}"));
            }
        }

        match matches.as_slice() {
            [ty] => Some(ty.clone()),
            _ => None,
        }
    }

    fn root_canonical_type_name(&self, ty: &str) -> String {
        let mut current = ty.to_string();
        let mut seen = HashSet::new();

        loop {
            if !seen.insert(current.clone()) {
                return current;
            }

            let Some(type_kind) = self.lookup_type_kind(&current) else {
                return current;
            };
            let TypeKind::Range { base } = type_kind else {
                return current;
            };

            let package_context =
                split_qualified_type_str(&current).map(|(package_name, _)| package_name);
            let Some(next) = self.canonical_type_name(base, package_context.as_deref()) else {
                return current;
            };
            current = next;
        }
    }

    fn lookup_type_kind(&self, canonical_type: &str) -> Option<&TypeKind> {
        if let Some((package_name, type_name)) = split_qualified_type_str(canonical_type) {
            return self
                .summary
                .package_type_kinds
                .get(&package_name)
                .and_then(|types| types.get(&type_name));
        }

        self.summary.top_level_type_kinds.get(canonical_type)
    }

    fn is_boolean_type(&self, ty: &str, current_package: Option<&str>) -> Option<bool> {
        self.canonical_type_name(ty, current_package)
            .map(|canonical| self.root_canonical_type_name(&canonical) == "Boolean")
    }

    fn is_numeric_type(&self, ty: &str, current_package: Option<&str>) -> Option<bool> {
        self.canonical_type_name(ty, current_package)
            .map(|canonical| {
                self.is_numeric_canonical_type(&self.root_canonical_type_name(&canonical))
            })
    }

    fn is_float_type(&self, ty: &str, current_package: Option<&str>) -> Option<bool> {
        self.canonical_type_name(ty, current_package)
            .map(|canonical| self.root_canonical_type_name(&canonical) == "Float")
    }

    fn is_discrete_type(&self, ty: &str, current_package: Option<&str>) -> Option<bool> {
        let canonical = self.canonical_type_name(ty, current_package)?;
        let root = self.root_canonical_type_name(&canonical);
        Some(match root.as_str() {
            "Boolean" | "Character" | "Integer" => true,
            "Float" | "String" => false,
            _ => matches!(self.lookup_type_kind(&root), Some(TypeKind::Enum)),
        })
    }

    fn is_comparable_type(&self, ty: &str, current_package: Option<&str>) -> Option<bool> {
        self.canonical_type_name(ty, current_package).map(|_| true)
    }

    fn is_orderable_type(&self, ty: &str, current_package: Option<&str>) -> Option<bool> {
        let canonical = self.canonical_type_name(ty, current_package)?;
        let root = self.root_canonical_type_name(&canonical);
        Some(match root.as_str() {
            "Boolean" | "Character" | "Integer" | "Float" | "String" => true,
            _ => matches!(self.lookup_type_kind(&root), Some(TypeKind::Enum)),
        })
    }

    fn is_numeric_canonical_type(&self, canonical_type: &str) -> bool {
        matches!(canonical_type, "Integer" | "Float")
    }

    fn visible_used_internal_packages(&self) -> Vec<&PackageSummary> {
        self.summary
            .uses
            .iter()
            .filter_map(|name| self.summary.packages.get(name))
            .collect()
    }

    fn visible_used_internal_package_types(&self) -> Vec<&HashSet<String>> {
        self.summary
            .uses
            .iter()
            .filter_map(|name| self.summary.package_types.get(name))
            .collect()
    }

    fn visible_used_internal_package_enum_literals(&self) -> Vec<&HashSet<String>> {
        self.summary
            .uses
            .iter()
            .filter_map(|name| self.summary.package_enum_literals.get(name))
            .collect()
    }
}

fn collect_package_items(
    package: &Package,
    signatures: &mut SignatureIndex,
    type_names: &mut HashSet<String>,
    enum_literals: &mut HashSet<String>,
    type_kinds: &mut HashMap<String, TypeKind>,
    enum_literal_types: &mut HashMap<String, Vec<String>>,
    scope_name: &str,
) -> Result<(), Diagnostic> {
    for item in &package.items {
        match item {
            PackageItem::Subprogram(subprogram) => {
                insert_signature(
                    signatures,
                    SubprogramSignature::from_subprogram(subprogram),
                    subprogram.position,
                    &format!("duplicate subprogram signature in {scope_name}"),
                )?;
            }
            PackageItem::Type(type_decl) => {
                validate_type_decl(type_decl, type_names, scope_name)?;
                collect_type_symbols(type_decl, type_names, enum_literals);
                collect_type_metadata(type_decl, type_kinds, enum_literal_types);
            }
        }
    }

    Ok(())
}

fn validate_type_decl(
    type_decl: &TypeDecl,
    seen_names: &mut HashSet<String>,
    scope_name: &str,
) -> Result<(), Diagnostic> {
    match type_decl {
        TypeDecl::Record(record) => {
            if !seen_names.insert(record.name.clone()) {
                return Err(Diagnostic::new(
                    format!("duplicate type `{}` in {scope_name}", record.name),
                    record.position,
                ));
            }

            let mut field_names = HashSet::new();
            for field in &record.fields {
                if !field_names.insert(field.name.clone()) {
                    return Err(Diagnostic::new(
                        format!(
                            "record type `{}` contains duplicate field `{}`",
                            record.name, field.name
                        ),
                        record.position,
                    ));
                }
            }
        }
        TypeDecl::Enum(enum_type) => {
            if !seen_names.insert(enum_type.name.clone()) {
                return Err(Diagnostic::new(
                    format!("duplicate type `{}` in {scope_name}", enum_type.name),
                    enum_type.position,
                ));
            }

            let mut variant_names = HashSet::new();
            for variant in &enum_type.variants {
                if !variant_names.insert(variant.clone()) {
                    return Err(Diagnostic::new(
                        format!(
                            "enum type `{}` contains duplicate variant `{}`",
                            enum_type.name, variant
                        ),
                        enum_type.position,
                    ));
                }
            }
        }
        TypeDecl::Range(range_type) => {
            if !seen_names.insert(range_type.name.clone()) {
                return Err(Diagnostic::new(
                    format!("duplicate type `{}` in {scope_name}", range_type.name),
                    range_type.position,
                ));
            }
        }
    }

    Ok(())
}

fn insert_signature(
    index: &mut SignatureIndex,
    signature: SubprogramSignature,
    position: Position,
    message: &str,
) -> Result<(), Diagnostic> {
    let overloads = index.entry(signature.key.name.clone()).or_default();
    if overloads
        .iter()
        .any(|existing| existing.key == signature.key)
    {
        return Err(Diagnostic::new(message, position));
    }
    overloads.push(signature);
    Ok(())
}

fn contains_signature(index: &SignatureIndex, key: &SignatureKey) -> bool {
    index
        .get(&key.name)
        .is_some_and(|signatures| signatures.iter().any(|signature| signature.key == *key))
}

fn iter_signatures(index: &SignatureIndex) -> impl Iterator<Item = &SubprogramSignature> {
    index.values().flat_map(|signatures| signatures.iter())
}

fn extend_candidates<'a>(
    target: &mut Vec<&'a SubprogramSignature>,
    source: Option<&'a Vec<SubprogramSignature>>,
) {
    let Some(source) = source else {
        return;
    };

    for signature in source {
        if !target.iter().any(|existing| existing.key == signature.key) {
            target.push(signature);
        }
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

fn display_callee(expr: &Expr) -> String {
    expr_to_name(expr)
        .map(|name| name.as_string())
        .unwrap_or_else(|| "<call>".to_string())
}

fn collect_type_symbols(
    type_decl: &TypeDecl,
    type_names: &mut HashSet<String>,
    enum_literals: &mut HashSet<String>,
) {
    match type_decl {
        TypeDecl::Record(record) => {
            type_names.insert(record.name.clone());
        }
        TypeDecl::Enum(enum_type) => {
            type_names.insert(enum_type.name.clone());
            enum_literals.extend(enum_type.variants.iter().cloned());
        }
        TypeDecl::Range(range_type) => {
            type_names.insert(range_type.name.clone());
        }
    }
}

fn collect_type_metadata(
    type_decl: &TypeDecl,
    type_kinds: &mut HashMap<String, TypeKind>,
    enum_literal_types: &mut HashMap<String, Vec<String>>,
) {
    match type_decl {
        TypeDecl::Record(record) => {
            type_kinds.insert(record.name.clone(), TypeKind::Record);
        }
        TypeDecl::Enum(enum_type) => {
            type_kinds.insert(enum_type.name.clone(), TypeKind::Enum);
            for variant in &enum_type.variants {
                enum_literal_types
                    .entry(variant.clone())
                    .or_default()
                    .push(enum_type.name.clone());
            }
        }
        TypeDecl::Range(range_type) => {
            type_kinds.insert(
                range_type.name.clone(),
                TypeKind::Range {
                    base: range_type.base.as_string(),
                },
            );
        }
    }
}

fn builtin_type_names() -> &'static [&'static str] {
    &["Boolean", "Integer", "Float", "Character", "String"]
}

fn unary_op_text(op: UnaryOp) -> &'static str {
    match op {
        UnaryOp::Negate => "-",
        UnaryOp::Not => "not",
    }
}

fn binary_op_text(op: BinaryOp) -> &'static str {
    match op {
        BinaryOp::Or => "or",
        BinaryOp::ShortCircuitOr => "or else",
        BinaryOp::And => "and",
        BinaryOp::ShortCircuitAnd => "and then",
        BinaryOp::Add => "+",
        BinaryOp::Subtract => "-",
        BinaryOp::Multiply => "*",
        BinaryOp::Divide => "/",
        BinaryOp::Equal => "==",
        BinaryOp::NotEqual => "!=",
        BinaryOp::Less => "<",
        BinaryOp::LessEqual => "<=",
        BinaryOp::Greater => ">",
        BinaryOp::GreaterEqual => ">=",
    }
}

fn split_qualified_name(name: &Name) -> Option<(String, String)> {
    let type_name = name.segments.last()?.clone();
    let package_name = name.segments[..name.segments.len().saturating_sub(1)].join(".");
    if package_name.is_empty() {
        None
    } else {
        Some((package_name, type_name))
    }
}

fn split_qualified_type_str(name: &str) -> Option<(String, String)> {
    let (package_name, type_name) = name.rsplit_once('.')?;
    Some((package_name.to_string(), type_name.to_string()))
}

fn find_result_reference(expr: &Expr) -> Option<Position> {
    match expr {
        Expr::Name { name, position }
            if name.segments.len() == 1 && name.segments[0] == "result" =>
        {
            Some(*position)
        }
        Expr::Name { .. } | Expr::Bool { .. } | Expr::Integer { .. } | Expr::String { .. } => None,
        Expr::Member { base, .. } => find_result_reference(base),
        Expr::Call { callee, args, .. } => {
            find_result_reference(callee).or_else(|| args.iter().find_map(find_result_reference))
        }
        Expr::Unary { expr, .. } => find_result_reference(expr),
        Expr::Binary { lhs, rhs, .. } => {
            find_result_reference(lhs).or_else(|| find_result_reference(rhs))
        }
    }
}
