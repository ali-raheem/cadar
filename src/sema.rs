use std::collections::{HashMap, HashSet};

use crate::{
    ast::{
        BinaryOp, BlockItem, CallArg, DependTarget, DependsContract, Expr, GlobalContract, Item,
        LocalDecl, Name, Package, PackageItem, ParamMode, Program, RecordFieldInit, Statement,
        StatementBlock, Subprogram, TypeDecl, UnaryOp,
    },
    diagnostic::{Diagnostic, IndexedDiagnostic, Position},
};

type SignatureIndex = HashMap<String, Vec<SubprogramSignature>>;

pub fn validate_all(programs: &[Program]) -> Result<(), IndexedDiagnostic> {
    let summary = ProgramSummary::collect(programs)?;
    summary.validate_top_level_consistency()?;
    summary.validate_package_consistency()?;

    for (source_index, program) in programs.iter().enumerate() {
        let source_context = SourceContext::from_program(program);
        Validator::new(&summary, &source_context)
            .validate_program(program)
            .map_err(|diagnostic| IndexedDiagnostic::new(source_index, diagnostic))?;
    }

    Ok(())
}

#[derive(Debug, Clone, Default)]
struct SourceContext {
    imports: HashSet<String>,
    uses: HashSet<String>,
}

impl SourceContext {
    fn from_program(program: &Program) -> Self {
        let mut context = Self::default();
        for item in &program.items {
            match item {
                Item::Import(name) => {
                    context.imports.insert(name.as_string());
                }
                Item::Use(name) => {
                    context.uses.insert(name.as_string());
                }
                Item::Subprogram(_) | Item::Type(_) | Item::Package(_) => {}
            }
        }
        context
    }
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
    params: Vec<ParamProfile>,
    requires: Vec<Expr>,
    ensures: Vec<Expr>,
    global: Option<GlobalContract>,
    depends: Option<DependsContract>,
    position: Position,
    source_index: usize,
    owner_package: Option<String>,
}

#[derive(Debug, Clone)]
struct ParamProfile {
    name: String,
    mode: ParamMode,
    ty: String,
    default: Option<Expr>,
    has_default: bool,
}

impl SubprogramSignature {
    fn from_subprogram(
        subprogram: &Subprogram,
        source_index: usize,
        owner_package: Option<&str>,
    ) -> Self {
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
            params: subprogram
                .params
                .iter()
                .map(|param| ParamProfile {
                    name: param.name.clone(),
                    mode: param.mode,
                    ty: param.ty.as_string(),
                    default: param.default.clone(),
                    has_default: param.default.is_some(),
                })
                .collect(),
            requires: subprogram.requires.clone(),
            ensures: subprogram.ensures.clone(),
            global: subprogram.global.clone(),
            depends: subprogram.depends.clone(),
            position: subprogram.position,
            source_index,
            owner_package: owner_package.map(str::to_string),
        }
    }

    fn arity(&self) -> usize {
        self.params.len()
    }

    fn required_arity(&self) -> usize {
        self.params
            .iter()
            .filter(|param| !param.has_default)
            .count()
    }

    fn owner_package(&self) -> Option<&str> {
        self.owner_package.as_deref()
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
    Record {
        fields: HashMap<String, String>,
    },
    Enum,
    Range {
        base: String,
    },
    Array {
        element_type: String,
        length: Option<usize>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum InferredType {
    Known(String),
    Aggregate(Vec<InferredType>),
    Procedure,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ValueInfo {
    ty: String,
    assignable: bool,
}

#[derive(Debug, Clone, Copy)]
struct ExpectedTypeRef<'a> {
    ty: &'a str,
    context: Option<&'a str>,
}

#[derive(Debug, Default)]
struct ProgramSummary {
    top_level_declarations: SignatureIndex,
    top_level_definitions: SignatureIndex,
    top_level_types: HashSet<String>,
    top_level_enum_literals: HashSet<String>,
    top_level_type_kinds: HashMap<String, TypeKind>,
    top_level_enum_literal_types: HashMap<String, Vec<String>>,
    packages: HashMap<String, PackageSummary>,
    package_types: HashMap<String, HashSet<String>>,
    package_spec_values: HashMap<String, HashMap<String, ValueInfo>>,
    package_body_values: HashMap<String, HashMap<String, ValueInfo>>,
    package_enum_literals: HashMap<String, HashSet<String>>,
    package_type_kinds: HashMap<String, HashMap<String, TypeKind>>,
    package_enum_literal_types: HashMap<String, HashMap<String, Vec<String>>>,
}

impl ProgramSummary {
    fn collect(programs: &[Program]) -> Result<Self, IndexedDiagnostic> {
        let mut summary = Self::default();
        let mut top_level_types = HashSet::new();

        for (source_index, program) in programs.iter().enumerate() {
            for item in &program.items {
                match item {
                    Item::Import(_) | Item::Use(_) => {}
                    Item::Subprogram(subprogram) => {
                        let signature =
                            SubprogramSignature::from_subprogram(subprogram, source_index, None);
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
                        )
                        .map_err(|diagnostic| IndexedDiagnostic::new(source_index, diagnostic))?;
                    }
                    Item::Type(type_decl) => {
                        validate_type_decl(type_decl, &mut top_level_types, "top-level").map_err(
                            |diagnostic| IndexedDiagnostic::new(source_index, diagnostic),
                        )?;
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
                        let package_spec_values = summary
                            .package_spec_values
                            .entry(package_name.clone())
                            .or_default();
                        let package_body_values = summary
                            .package_body_values
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
                                return Err(IndexedDiagnostic::new(
                                    source_index,
                                    Diagnostic::new(
                                        format!("duplicate package body `{package_name}`"),
                                        package.position,
                                    ),
                                ));
                            }
                            entry.has_body = true;
                            collect_package_items(
                                package,
                                source_index,
                                PackageCollections {
                                    signatures: &mut entry.body_subprograms,
                                    values: package_body_values,
                                    type_names: package_types,
                                    enum_literals: package_enum_literals,
                                    type_kinds: package_type_kinds,
                                    enum_literal_types: package_enum_literal_types,
                                },
                            )
                            .map_err(|diagnostic| {
                                IndexedDiagnostic::new(source_index, diagnostic)
                            })?;
                        } else {
                            if entry.has_spec {
                                return Err(IndexedDiagnostic::new(
                                    source_index,
                                    Diagnostic::new(
                                        format!("duplicate package specification `{package_name}`"),
                                        package.position,
                                    ),
                                ));
                            }
                            entry.has_spec = true;
                            collect_package_items(
                                package,
                                source_index,
                                PackageCollections {
                                    signatures: &mut entry.spec_subprograms,
                                    values: package_spec_values,
                                    type_names: package_types,
                                    enum_literals: package_enum_literals,
                                    type_kinds: package_type_kinds,
                                    enum_literal_types: package_enum_literal_types,
                                },
                            )
                            .map_err(|diagnostic| {
                                IndexedDiagnostic::new(source_index, diagnostic)
                            })?;
                        }
                    }
                }
            }
        }

        Ok(summary)
    }

    fn validate_package_consistency(&self) -> Result<(), IndexedDiagnostic> {
        for (package_name, package) in &self.packages {
            if !(package.has_spec && package.has_body) {
                continue;
            }

            for body_signature in iter_signatures(&package.body_subprograms) {
                if let Some(spec_signature) =
                    find_signature(&package.spec_subprograms, &body_signature.key)
                {
                    validate_definition_matches_declaration(spec_signature, body_signature)
                        .map_err(|diagnostic| {
                            IndexedDiagnostic::new(body_signature.source_index, diagnostic)
                        })?;
                }
            }

            for spec_signature in iter_signatures(&package.spec_subprograms) {
                if !contains_signature(&package.body_subprograms, &spec_signature.key) {
                    return Err(IndexedDiagnostic::new(
                        spec_signature.source_index,
                        Diagnostic::new(
                            format!(
                                "package body `{package_name}` is missing a definition for `{}`",
                                spec_signature.key.name
                            ),
                            spec_signature.position,
                        ),
                    ));
                }
            }
        }

        Ok(())
    }

    fn validate_top_level_consistency(&self) -> Result<(), IndexedDiagnostic> {
        for definition in iter_signatures(&self.top_level_definitions) {
            let Some(declaration) = find_signature(&self.top_level_declarations, &definition.key)
            else {
                continue;
            };
            validate_definition_matches_declaration(declaration, definition).map_err(
                |diagnostic| IndexedDiagnostic::new(definition.source_index, diagnostic),
            )?;
        }

        Ok(())
    }
}

#[derive(Debug, Clone, Default)]
struct Scope {
    values: HashMap<String, ValueInfo>,
    subprograms: SignatureIndex,
}

impl Scope {
    fn with_value(&self, name: &str, ty: &str, assignable: bool) -> Self {
        let mut scope = self.clone();
        scope.values.insert(
            name.to_string(),
            ValueInfo {
                ty: ty.to_string(),
                assignable,
            },
        );
        scope
    }

    fn set_value(&mut self, name: &str, ty: &str, assignable: bool) {
        self.values.insert(
            name.to_string(),
            ValueInfo {
                ty: ty.to_string(),
                assignable,
            },
        );
    }

    fn extend_values(&mut self, values: &HashMap<String, ValueInfo>) {
        self.values.extend(values.clone());
    }

    fn extend_subprograms(&mut self, subprograms: &SignatureIndex) {
        self.subprograms.extend(subprograms.clone());
    }

    fn contains_value(&self, name: &str) -> bool {
        self.values.contains_key(name)
    }

    fn value_type(&self, name: &str) -> Option<&str> {
        self.values.get(name).map(|value| value.ty.as_str())
    }

    fn value_info(&self, name: &str) -> Option<&ValueInfo> {
        self.values.get(name)
    }

    fn subprogram_overloads(&self, name: &str) -> Option<&[SubprogramSignature]> {
        self.subprograms.get(name).map(Vec::as_slice)
    }
}

struct Validator<'a> {
    summary: &'a ProgramSummary,
    source_context: &'a SourceContext,
}

impl<'a> Validator<'a> {
    fn new(summary: &'a ProgramSummary, source_context: &'a SourceContext) -> Self {
        Self {
            summary,
            source_context,
        }
    }

    fn validate_program(&self, program: &Program) -> Result<(), Diagnostic> {
        for item in &program.items {
            match item {
                Item::Subprogram(subprogram) => {
                    self.validate_subprogram(subprogram, None, &Scope::default())?
                }
                Item::Package(package) => self.validate_package(package)?,
                Item::Import(_) | Item::Use(_) | Item::Type(_) => {}
            }
        }

        Ok(())
    }

    fn validate_package(&self, package: &Package) -> Result<(), Diagnostic> {
        let package_name = package.name.as_string();
        let mut scope = self.package_scope(&package_name, package.is_body);
        if package.is_body
            && let Some(package_summary) = self.summary.packages.get(&package_name)
        {
            scope.extend_subprograms(&package_summary.body_subprograms);
        }

        for item in &package.items {
            match item {
                PackageItem::Object(decl) => {
                    self.validate_object_decl(decl, &scope, Some(&package_name))?;
                }
                PackageItem::Subprogram(subprogram) => {
                    self.validate_subprogram(subprogram, Some(&package_name), &scope)?;
                }
                PackageItem::Type(_) => {}
            }
        }

        Ok(())
    }

    fn validate_subprogram(
        &self,
        subprogram: &Subprogram,
        current_package: Option<&str>,
        initial_scope: &Scope,
    ) -> Result<(), Diagnostic> {
        let mut scope = initial_scope.clone();
        let mut declared_names = HashSet::new();
        for param in &subprogram.params {
            if param.mode != ParamMode::In && param.default.is_some() {
                return Err(Diagnostic::new(
                    "default values are only allowed for `in` parameters",
                    param
                        .default
                        .as_ref()
                        .map(Expr::position)
                        .unwrap_or(subprogram.position),
                ));
            }
            if let Some(default) = &param.default {
                self.validate_expr(default, &scope, current_package)?;
                self.validate_value_type(
                    default,
                    Some(&param.ty.as_string()),
                    "default parameter",
                    default.position(),
                    &scope,
                    current_package,
                )?;
            }
            if !declared_names.insert(param.name.clone()) {
                return Err(Diagnostic::new(
                    format!("duplicate parameter `{}`", param.name),
                    subprogram.position,
                ));
            }
            scope.set_value(
                &param.name,
                &param.ty.as_string(),
                param.mode != ParamMode::In,
            );
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

        self.validate_dataflow_contracts(subprogram)?;

        let Some(body) = &subprogram.body else {
            return Ok(());
        };

        for item in &body.items {
            match item {
                BlockItem::LocalDecl(decl) => {
                    self.validate_object_decl(decl, &scope, current_package)?;
                    if !declared_names.insert(decl.name.clone()) {
                        return Err(Diagnostic::new(
                            format!("duplicate local declaration `{}`", decl.name),
                            decl.position,
                        ));
                    }
                    scope.set_value(&decl.name, &decl.ty.as_string(), !decl.is_const);
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

    fn validate_object_decl(
        &self,
        decl: &LocalDecl,
        scope: &Scope,
        current_package: Option<&str>,
    ) -> Result<(), Diagnostic> {
        if let Some(initializer) = &decl.initializer {
            self.validate_expr(initializer, scope, current_package)?;
            self.validate_value_type(
                initializer,
                Some(&decl.ty.as_string()),
                if current_package.is_some() {
                    "object initializer"
                } else {
                    "local initializer"
                },
                decl.position,
                scope,
                current_package,
            )?;
        }

        Ok(())
    }

    fn package_scope(&self, package_name: &str, include_body_values: bool) -> Scope {
        let mut scope = Scope::default();
        if let Some(values) = self.summary.package_spec_values.get(package_name) {
            scope.extend_values(values);
        }
        if include_body_values
            && let Some(values) = self.summary.package_body_values.get(package_name)
        {
            scope.extend_values(values);
        }
        scope
    }

    fn validate_dataflow_contracts(&self, subprogram: &Subprogram) -> Result<(), Diagnostic> {
        if let Some(global) = &subprogram.global {
            self.validate_global_contract(global, subprogram.position)?;
        }
        if let Some(depends) = &subprogram.depends {
            self.validate_depends_contract(
                depends,
                subprogram.return_type.is_some(),
                subprogram.position,
            )?;
        }
        Ok(())
    }

    fn validate_global_contract(
        &self,
        global: &GlobalContract,
        position: Position,
    ) -> Result<(), Diagnostic> {
        for item in &global.items {
            for name in &item.names {
                if name.segments.len() == 1 && name.segments[0] == "result" {
                    return Err(Diagnostic::new(
                        "`result` is not valid in `global` clauses",
                        position,
                    ));
                }
            }
        }
        Ok(())
    }

    fn validate_depends_contract(
        &self,
        depends: &DependsContract,
        is_function: bool,
        position: Position,
    ) -> Result<(), Diagnostic> {
        let mut result_targets = 0usize;
        for item in &depends.items {
            if matches!(item.target, DependTarget::Result) {
                if !is_function {
                    return Err(Diagnostic::new(
                        "`result` is only valid in `depends` clauses of functions",
                        position,
                    ));
                }
                result_targets += 1;
            }

            for source in &item.sources {
                if source.segments.len() == 1 && source.segments[0] == "result" {
                    return Err(Diagnostic::new(
                        "`result` is not valid as a dependency source",
                        position,
                    ));
                }
            }
        }

        if is_function && result_targets != 1 {
            return Err(Diagnostic::new(
                "function `depends` clauses must mention `result` exactly once",
                position,
            ));
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
            Statement::Assert { expr, position } => {
                self.validate_expr(expr, scope, current_package)?;
                self.validate_boolean_expr(expr, "assertion", *position, scope, current_package)
            }
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
                condition,
                invariants,
                variants,
                body,
                ..
            } => {
                self.validate_expr(condition, scope, current_package)?;
                self.validate_boolean_expr(
                    condition,
                    "while condition",
                    condition.position(),
                    scope,
                    current_package,
                )?;
                for invariant in invariants {
                    self.validate_expr(invariant, scope, current_package)?;
                    self.validate_boolean_expr(
                        invariant,
                        "loop invariant",
                        invariant.position(),
                        scope,
                        current_package,
                    )?;
                }
                for variant in variants {
                    self.validate_expr(&variant.expr, scope, current_package)?;
                    self.validate_numeric_expr(
                        &variant.expr,
                        "loop variant",
                        variant.expr.position(),
                        scope,
                        current_package,
                    )?;
                }
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
                    false,
                );
                for invariant in &for_statement.invariants {
                    self.validate_expr(invariant, &loop_scope, current_package)?;
                    self.validate_boolean_expr(
                        invariant,
                        "loop invariant",
                        invariant.position(),
                        &loop_scope,
                        current_package,
                    )?;
                }
                for variant in &for_statement.variants {
                    self.validate_expr(&variant.expr, &loop_scope, current_package)?;
                    self.validate_numeric_expr(
                        &variant.expr,
                        "loop variant",
                        variant.expr.position(),
                        &loop_scope,
                        current_package,
                    )?;
                }
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
        let mut scope = scope.clone();
        let mut declared_names = HashSet::new();
        let mut saw_statement = false;

        for item in &block.items {
            match item {
                BlockItem::LocalDecl(decl) => {
                    if saw_statement {
                        return Err(Diagnostic::new(
                            "local declarations must appear before statements in a nested block",
                            decl.position,
                        ));
                    }
                    self.validate_object_decl(decl, &scope, current_package)?;
                    if !declared_names.insert(decl.name.clone()) {
                        return Err(Diagnostic::new(
                            format!("duplicate local declaration `{}`", decl.name),
                            decl.position,
                        ));
                    }
                    scope.set_value(&decl.name, &decl.ty.as_string(), !decl.is_const);
                }
                BlockItem::Statement(statement) => {
                    saw_statement = true;
                    self.validate_statement(
                        statement,
                        expected_return_type,
                        &scope,
                        current_package,
                    )?;
                }
            }
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

        let contract_scope = scope.with_value("result", &result_type.as_string(), false);
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
        match inferred {
            InferredType::Known(actual_type) => {
                if self.types_are_compatible(expected_type, &actual_type, current_package) {
                    return Ok(());
                }

                Err(Diagnostic::new(
                    format!("{context} expects `{expected_type}`, found `{actual_type}`"),
                    position,
                ))
            }
            InferredType::Aggregate(_) => self.validate_array_literal_type(
                expr,
                expected_type,
                context,
                position,
                scope,
                current_package,
            ),
            InferredType::Procedure | InferredType::Unknown => Ok(()),
        }
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
        match inferred {
            InferredType::Known(actual_type) => {
                match self.is_boolean_type(&actual_type, current_package) {
                    Some(true) | None => Ok(()),
                    Some(false) => Err(Diagnostic::new(
                        format!("{context} must be Boolean, found `{actual_type}`"),
                        position,
                    )),
                }
            }
            InferredType::Aggregate(_) => Err(Diagnostic::new(
                format!("{context} must be Boolean, found array literal"),
                position,
            )),
            InferredType::Procedure | InferredType::Unknown => Ok(()),
        }
    }

    fn validate_numeric_expr(
        &self,
        expr: &Expr,
        context: &str,
        position: Position,
        scope: &Scope,
        current_package: Option<&str>,
    ) -> Result<(), Diagnostic> {
        let inferred = self.infer_value_type(expr, scope, current_package)?;
        match inferred {
            InferredType::Known(actual_type) => {
                match self.is_numeric_type(&actual_type, current_package) {
                    Some(true) | None => Ok(()),
                    Some(false) => Err(Diagnostic::new(
                        format!("{context} must be numeric, found `{actual_type}`"),
                        position,
                    )),
                }
            }
            InferredType::Aggregate(_) => Err(Diagnostic::new(
                format!("{context} must be numeric, found array literal"),
                position,
            )),
            InferredType::Procedure | InferredType::Unknown => Ok(()),
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
            InferredType::Aggregate(_) => Err(Diagnostic::new(
                "case expression must be discrete, found array literal",
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
        match (case_type, choice_type) {
            (InferredType::Known(case_type), InferredType::Known(choice_type)) => {
                if self.types_are_compatible(case_type, &choice_type, current_package) {
                    return Ok(());
                }

                Err(Diagnostic::new(
                    format!("case choice must match `{case_type}`, found `{choice_type}`"),
                    position,
                ))
            }
            (InferredType::Known(case_type), InferredType::Aggregate(_)) => Err(Diagnostic::new(
                format!("case choice must match `{case_type}`, found array literal"),
                position,
            )),
            _ => Ok(()),
        }
    }

    fn lookup_assignment_target_type(
        &self,
        target: &Expr,
        scope: &Scope,
        current_package: Option<&str>,
    ) -> Option<String> {
        match target {
            Expr::Name { name, .. } => {
                if name.segments.len() != 1 {
                    return None;
                }

                scope
                    .value_type(&name.segments[0])
                    .map(str::to_string)
                    .or_else(|| {
                        self.visible_used_package_value_info(&name.segments[0])
                            .map(|value| value.ty.clone())
                    })
            }
            Expr::Index { base, .. } => {
                let base_type = self.infer_expr_type(base, scope, current_package).ok()?;
                let InferredType::Known(base_type) = base_type else {
                    return None;
                };
                self.indexed_element_type(&base_type, current_package)
            }
            Expr::Member { base, member, .. } => {
                if let Some(package_name) = expr_to_name(base)
                    .filter(|name| self.is_visible_package_path(name, current_package))
                    .map(|name| name.as_string())
                    && let Some(value) = self.package_public_value_info(&package_name, member)
                {
                    return Some(value.ty.clone());
                }

                let member_type = self.infer_expr_type(target, scope, current_package).ok()?;
                let InferredType::Known(member_type) = member_type else {
                    return None;
                };
                Some(member_type)
            }
            Expr::Bool { .. }
            | Expr::Integer { .. }
            | Expr::Float { .. }
            | Expr::Character { .. }
            | Expr::String { .. }
            | Expr::Call { .. }
            | Expr::RecordLiteral { .. }
            | Expr::ArrayLiteral { .. }
            | Expr::Unary { .. }
            | Expr::Binary { .. } => None,
        }
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
            Expr::Float { .. } => Ok(InferredType::Known("Float".to_string())),
            Expr::Character { .. } => Ok(InferredType::Known("Character".to_string())),
            Expr::String { .. } => Ok(InferredType::Known("String".to_string())),
            Expr::Name { name, .. } => Ok(self.infer_name_type(name, scope, current_package)),
            Expr::Member {
                base,
                member,
                position,
            } => self.infer_member_type(base, member, *position, scope, current_package),
            Expr::Index {
                base,
                index,
                position,
            } => self.infer_index_expr_type(base, index, *position, scope, current_package),
            Expr::Call { callee, args, .. } => {
                self.infer_call_type(callee, args, scope, current_package)
            }
            Expr::RecordLiteral {
                ty,
                fields,
                position,
            } => self.infer_record_literal_type(ty, fields, *position, scope, current_package),
            Expr::ArrayLiteral { elements, .. } => Ok(InferredType::Aggregate(
                elements
                    .iter()
                    .map(|element| self.infer_expr_type(element, scope, current_package))
                    .collect::<Result<Vec<_>, _>>()?,
            )),
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

            if let Some(value) = self.visible_used_package_value_info(&name.segments[0]) {
                return InferredType::Known(value.ty.clone());
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
        position: Position,
        scope: &Scope,
        current_package: Option<&str>,
    ) -> Result<InferredType, Diagnostic> {
        if let Some(type_name) = expr_to_name(base)
            .filter(|name| self.is_visible_type_path(name, current_package))
            .map(|name| name.as_string())
        {
            return self.infer_type_member_type(&type_name, member, position, current_package);
        }

        if let Some(package_name) = expr_to_name(base)
            .filter(|name| self.is_visible_package_path(name, current_package))
            .map(|name| name.as_string())
        {
            if let Some(value) = self.package_public_value_info(&package_name, member) {
                return Ok(InferredType::Known(value.ty.clone()));
            }

            return Ok(
                match self
                    .package_enum_literal_types(&package_name, member)
                    .as_slice()
                {
                    [ty] => InferredType::Known(ty.clone()),
                    _ => InferredType::Unknown,
                },
            );
        }

        let base_type = self.infer_expr_type(base, scope, current_package)?;
        match (member, base_type) {
            ("length", InferredType::Known(base_type)) => {
                match self.is_array_like_type(&base_type, current_package) {
                    Some(true) | None => Ok(InferredType::Known("Integer".to_string())),
                    Some(false) => Err(Diagnostic::new(
                        format!("attribute `length` requires an array value, found `{base_type}`"),
                        position,
                    )),
                }
            }
            ("range", InferredType::Known(base_type)) => {
                match self.is_array_like_type(&base_type, current_package) {
                    Some(true) | None => Ok(InferredType::Unknown),
                    Some(false) => Err(Diagnostic::new(
                        format!("attribute `range` requires an array value, found `{base_type}`"),
                        position,
                    )),
                }
            }
            ("image", InferredType::Known(base_type)) => Err(Diagnostic::new(
                format!("attribute `image` requires a type name, found `{base_type}`"),
                position,
            )),
            (_, InferredType::Known(base_type)) => {
                if let Some(field_type) =
                    self.record_field_type(&base_type, member, current_package)
                {
                    Ok(InferredType::Known(field_type))
                } else if self
                    .canonical_type_name(&base_type, current_package)
                    .is_some()
                {
                    Err(Diagnostic::new(
                        format!("type `{base_type}` has no field `{member}`"),
                        position,
                    ))
                } else {
                    Ok(InferredType::Unknown)
                }
            }
            ("image", InferredType::Aggregate(_)) => Err(Diagnostic::new(
                "attribute `image` requires a type name, found array literal",
                position,
            )),
            ("length", InferredType::Aggregate(_)) | ("range", InferredType::Aggregate(_)) => {
                Err(Diagnostic::new(
                    format!("attribute `{member}` requires an array value, found array literal"),
                    position,
                ))
            }
            (_, InferredType::Aggregate(_)) => Err(Diagnostic::new(
                format!("array literal has no field `{member}`"),
                position,
            )),
            (_, InferredType::Procedure) => Err(Diagnostic::new(
                "procedures do not produce a value",
                position,
            )),
            (_, InferredType::Unknown) => Ok(InferredType::Unknown),
        }
    }

    fn infer_index_expr_type(
        &self,
        base: &Expr,
        index: &Expr,
        position: Position,
        scope: &Scope,
        current_package: Option<&str>,
    ) -> Result<InferredType, Diagnostic> {
        let base_type = self.infer_expr_type(base, scope, current_package)?;
        let index_type = self.infer_expr_type(index, scope, current_package)?;
        self.ensure_numeric_operand(&index_type, position, "[]", current_package)?;

        match base_type {
            InferredType::Known(base_type) => {
                if let Some(element_type) = self.indexed_element_type(&base_type, current_package) {
                    Ok(InferredType::Known(element_type))
                } else {
                    Err(Diagnostic::new(
                        format!("indexed expression must be an array, found `{base_type}`"),
                        position,
                    ))
                }
            }
            InferredType::Aggregate(_) => Err(Diagnostic::new(
                "indexed expression must be an array, found array literal",
                position,
            )),
            InferredType::Procedure => Err(Diagnostic::new(
                "procedures do not produce a value",
                position,
            )),
            InferredType::Unknown => Ok(InferredType::Unknown),
        }
    }

    fn infer_call_type(
        &self,
        callee: &Expr,
        args: &[CallArg],
        scope: &Scope,
        current_package: Option<&str>,
    ) -> Result<InferredType, Diagnostic> {
        self.validate_call_arg_syntax(args)?;

        if self.is_image_attribute_call(callee, current_package) {
            return Ok(if args.len() == 1 && args[0].name.is_none() {
                InferredType::Known("String".to_string())
            } else {
                InferredType::Unknown
            });
        }

        let candidates = self.resolve_call_candidates(callee, scope, current_package);
        if !candidates.is_empty() {
            let matching: Vec<_> = candidates
                .into_iter()
                .filter(|candidate| {
                    candidate.required_arity() <= args.len() && args.len() <= candidate.arity()
                })
                .collect();
            let matching =
                self.filter_compatible_call_candidates(matching, args, scope, current_package)?;

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

            return Ok(match (saw_procedure, return_types.as_slice()) {
                (true, []) => InferredType::Procedure,
                (false, [return_type]) => InferredType::Known(return_type.clone()),
                _ => InferredType::Unknown,
            });
        }

        if self.is_visible_type_expr(callee, current_package)
            && args.len() == 1
            && args[0].name.is_none()
        {
            return Ok(expr_to_name(callee)
                .map(|name| InferredType::Known(name.as_string()))
                .unwrap_or(InferredType::Unknown));
        }

        Ok(InferredType::Unknown)
    }

    fn infer_record_literal_type(
        &self,
        ty: &Name,
        fields: &[RecordFieldInit],
        position: Position,
        scope: &Scope,
        current_package: Option<&str>,
    ) -> Result<InferredType, Diagnostic> {
        let type_name = ty.as_string();
        let mut seen_fields = HashSet::new();
        for field in fields {
            if !seen_fields.insert(field.name.clone()) {
                return Err(Diagnostic::new(
                    format!(
                        "record aggregate for `{type_name}` contains duplicate field `{}`",
                        field.name
                    ),
                    position,
                ));
            }
        }

        if self.is_visible_value_name(ty, scope, current_package)
            && !self.is_visible_type_path(ty, current_package)
        {
            return Err(Diagnostic::new(
                format!("record aggregate requires a type name, found value `{type_name}`"),
                position,
            ));
        }

        let Some(canonical_type) = self.canonical_type_name(&type_name, current_package) else {
            return Ok(InferredType::Known(type_name));
        };

        let Some(record_fields) = self.record_field_types(&canonical_type) else {
            return Err(Diagnostic::new(
                format!("type `{type_name}` is not a record type"),
                position,
            ));
        };

        for field in fields {
            let Some(expected_type) = record_fields.get(&field.name) else {
                return Err(Diagnostic::new(
                    format!("type `{type_name}` has no field `{}`", field.name),
                    position,
                ));
            };
            self.validate_expected_value_type(
                &field.value,
                ExpectedTypeRef {
                    ty: expected_type,
                    context: current_package_for_canonical(&canonical_type).as_deref(),
                },
                &format!("record field `{}`", field.name),
                field.value.position(),
                scope,
                current_package,
            )?;
        }

        for expected_field in record_fields.keys() {
            if !seen_fields.contains(expected_field) {
                return Err(Diagnostic::new(
                    format!(
                        "record aggregate for `{type_name}` is missing field `{expected_field}`"
                    ),
                    position,
                ));
            }
        }

        Ok(InferredType::Known(type_name))
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
                    InferredType::Aggregate(_)
                    | InferredType::Unknown
                    | InferredType::Procedure => InferredType::Unknown,
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
            InferredType::Aggregate(_) => Err(Diagnostic::new(
                format!("`{op_name}` requires numeric operands, found array literal"),
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
            InferredType::Aggregate(_) => Err(Diagnostic::new(
                format!("`{op_name}` requires Boolean operands, found array literal"),
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
            InferredType::Aggregate(_) => Err(Diagnostic::new(
                format!("`{op_name}` requires comparable operands, found array literal"),
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
            InferredType::Aggregate(_) => Err(Diagnostic::new(
                format!("`{op_name}` requires ordered operands, found array literal"),
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
            Expr::Bool { .. }
            | Expr::Integer { .. }
            | Expr::Float { .. }
            | Expr::Character { .. }
            | Expr::String { .. } => Ok(()),
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
            expr @ Expr::Member { base, .. } => {
                self.validate_member_base(base, scope, current_package)?;
                self.infer_expr_type(expr, scope, current_package)?;
                Ok(())
            }
            expr @ Expr::Index { base, index, .. } => {
                self.validate_expr(base, scope, current_package)?;
                self.validate_expr(index, scope, current_package)?;
                self.infer_value_type(expr, scope, current_package)?;
                Ok(())
            }
            Expr::Call { callee, args, .. } => {
                for arg in args {
                    self.validate_expr(&arg.value, scope, current_package)?;
                    self.infer_value_type(&arg.value, scope, current_package)?;
                }
                self.validate_call(callee, args, scope, current_package)
            }
            expr @ Expr::RecordLiteral { fields, .. } => {
                for field in fields {
                    self.validate_expr(&field.value, scope, current_package)?;
                    self.infer_value_type(&field.value, scope, current_package)?;
                }
                self.infer_value_type(expr, scope, current_package)?;
                Ok(())
            }
            Expr::ArrayLiteral { elements, .. } => {
                for element in elements {
                    self.validate_expr(element, scope, current_package)?;
                    self.infer_value_type(element, scope, current_package)?;
                }
                Ok(())
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
        args: &[CallArg],
        scope: &Scope,
        current_package: Option<&str>,
    ) -> Result<(), Diagnostic> {
        self.validate_callee_reference(callee, scope, current_package)?;
        self.validate_call_arg_syntax(args)?;

        if self.is_image_attribute_call(callee, current_package) {
            if args.len() == 1 && args[0].name.is_none() {
                return Ok(());
            }

            return Err(Diagnostic::new(
                format!(
                    "attribute call `{}` requires 1 positional argument",
                    display_callee(callee)
                ),
                callee.position(),
            ));
        }

        let candidates = self.resolve_call_candidates(callee, scope, current_package);
        if !candidates.is_empty() {
            let matching: Vec<_> = candidates
                .into_iter()
                .filter(|candidate| {
                    candidate.required_arity() <= args.len() && args.len() <= candidate.arity()
                })
                .collect();
            if matching.is_empty() {
                let arg_count = args.len();
                let suffix = if arg_count == 1 { "" } else { "s" };
                return Err(Diagnostic::new(
                    format!(
                        "`{}` does not accept {arg_count} argument{suffix}",
                        display_callee(callee)
                    ),
                    callee.position(),
                ));
            }

            if !self
                .filter_compatible_call_candidates(matching, args, scope, current_package)?
                .is_empty()
            {
                return Ok(());
            }

            return Err(Diagnostic::new(
                format!(
                    "no matching overload for `{}` with argument types ({})",
                    display_callee(callee),
                    self.render_call_argument_types(args, scope, current_package)?
                ),
                callee.position(),
            ));
        }

        if self.is_visible_type_expr(callee, current_package) {
            if args.len() == 1 && args[0].name.is_none() {
                return Ok(());
            }

            return Err(Diagnostic::new(
                format!(
                    "type conversion `{}` requires 1 positional argument",
                    display_callee(callee)
                ),
                callee.position(),
            ));
        }

        if let Expr::Name { name, position } = callee
            && name.segments.len() == 1
            && self.is_visible_value_name(name, scope, current_package)
        {
            return Err(Diagnostic::new(
                format!("`{}` is not callable", name.as_string()),
                *position,
            ));
        }

        if let Expr::Member {
            base,
            member,
            position,
        } = callee
            && let Some(package_name) = expr_to_name(base)
                .filter(|name| self.is_visible_package_path(name, current_package))
                .map(|name| name.as_string())
            && self
                .package_public_value_info(&package_name, member)
                .is_some()
        {
            return Err(Diagnostic::new(
                format!("`{}` is not callable", display_callee(callee)),
                *position,
            ));
        }

        Ok(())
    }

    fn validate_call_arg_syntax(&self, args: &[CallArg]) -> Result<(), Diagnostic> {
        let mut saw_named = false;
        let mut seen_named = HashSet::new();

        for arg in args {
            match &arg.name {
                Some(name) => {
                    saw_named = true;
                    if !seen_named.insert(name.clone()) {
                        return Err(Diagnostic::new(
                            format!("duplicate named argument `{name}`"),
                            arg.position,
                        ));
                    }
                }
                None if saw_named => {
                    return Err(Diagnostic::new(
                        "positional arguments cannot follow named arguments",
                        arg.position,
                    ));
                }
                None => {}
            }
        }

        Ok(())
    }

    fn resolve_call_candidates<'b>(
        &'b self,
        callee: &Expr,
        scope: &'b Scope,
        current_package: Option<&str>,
    ) -> Vec<&'b SubprogramSignature> {
        match callee {
            Expr::Name { name, .. } if name.segments.len() == 1 => {
                let mut candidates = Vec::new();
                if let Some(subprograms) = scope.subprogram_overloads(&name.segments[0]) {
                    candidates.extend(subprograms.iter());
                }
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
                let mut candidates = Vec::new();
                if current_package.is_some_and(|current| current == package_name.as_string())
                    && let Some(subprograms) = scope.subprogram_overloads(member)
                {
                    candidates.extend(subprograms.iter());
                }
                let Some(package) = self.summary.packages.get(&package_name.as_string()) else {
                    return candidates;
                };
                extend_candidates(&mut candidates, package.visible_subprograms().get(member));
                candidates
            }
            _ => Vec::new(),
        }
    }

    fn validate_assignment_target(
        &self,
        target: &Expr,
        position: Position,
        scope: &Scope,
        current_package: Option<&str>,
    ) -> Result<(), Diagnostic> {
        match target {
            Expr::Name { name, .. } => {
                let Some(first_segment) = name.segments.first() else {
                    return Ok(());
                };

                if let Some(value) = scope.value_info(first_segment) {
                    return if value.assignable {
                        Ok(())
                    } else {
                        Err(Diagnostic::new(
                            format!("cannot assign to immutable value `{}`", name.as_string()),
                            position,
                        ))
                    };
                }

                if let Some(value) = self.visible_used_package_value_info(first_segment) {
                    return if value.assignable {
                        Ok(())
                    } else {
                        Err(Diagnostic::new(
                            format!("cannot assign to immutable value `{}`", name.as_string()),
                            position,
                        ))
                    };
                }

                if self.is_visible_value_name(name, scope, current_package)
                    || self.is_visible_package_path(name, current_package)
                    || self.is_visible_type_path(name, current_package)
                {
                    return Err(Diagnostic::new(
                        format!("invalid assignment target `{}`", name.as_string()),
                        position,
                    ));
                }

                Err(Diagnostic::new(
                    format!("undefined assignment target `{}`", name.as_string()),
                    position,
                ))
            }
            Expr::Member { base, member, .. } => {
                if is_readonly_attribute(member) {
                    return Err(Diagnostic::new(
                        format!("invalid assignment target `{}`", display_expr(target)),
                        position,
                    ));
                }

                if let Some(package_name) = expr_to_name(base)
                    .filter(|name| self.is_visible_package_path(name, current_package))
                    .map(|name| name.as_string())
                {
                    if let Some(value) = self.package_public_value_info(&package_name, member) {
                        return if value.assignable {
                            Ok(())
                        } else {
                            Err(Diagnostic::new(
                                format!(
                                    "cannot assign to immutable value `{}`",
                                    display_expr(target)
                                ),
                                position,
                            ))
                        };
                    }

                    if self.summary.packages.contains_key(&package_name) {
                        return Err(Diagnostic::new(
                            format!("invalid assignment target `{}`", display_expr(target)),
                            position,
                        ));
                    }

                    return Ok(());
                }

                self.validate_assignment_target(base, position, scope, current_package)?;
                self.infer_expr_type(target, scope, current_package)?;
                Ok(())
            }
            Expr::Index { base, index, .. } => {
                self.validate_assignment_target(base, position, scope, current_package)?;
                self.validate_expr(index, scope, current_package)?;
                self.infer_expr_type(target, scope, current_package)?;
                Ok(())
            }
            Expr::Bool { .. }
            | Expr::Integer { .. }
            | Expr::Float { .. }
            | Expr::Character { .. }
            | Expr::String { .. }
            | Expr::Call { .. }
            | Expr::RecordLiteral { .. }
            | Expr::ArrayLiteral { .. }
            | Expr::Unary { .. }
            | Expr::Binary { .. } => Err(Diagnostic::new("invalid assignment target", position)),
        }
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
            Expr::Member { .. } => {
                if let Some(name) = expr_to_name(base)
                    && (self.is_visible_package_path(&name, current_package)
                        || self.is_visible_type_path(&name, current_package))
                {
                    return Ok(());
                }

                self.validate_expr(base, scope, current_package)
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
            || self.has_visible_used_package_value(identifier)
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
            || self.source_context.imports.contains(name)
            || self.source_context.uses.contains(name)
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

    fn infer_type_member_type(
        &self,
        type_name: &str,
        member: &str,
        position: Position,
        current_package: Option<&str>,
    ) -> Result<InferredType, Diagnostic> {
        match member {
            "length" => {
                if self.is_array_like_type(type_name, current_package) == Some(true) {
                    Ok(InferredType::Known("Integer".to_string()))
                } else {
                    Err(Diagnostic::new(
                        format!("attribute `length` requires an array type, found `{type_name}`"),
                        position,
                    ))
                }
            }
            "range" => {
                if self.is_array_like_type(type_name, current_package) == Some(true) {
                    Ok(InferredType::Unknown)
                } else {
                    Err(Diagnostic::new(
                        format!("attribute `range` requires an array type, found `{type_name}`"),
                        position,
                    ))
                }
            }
            "image" => Ok(InferredType::Unknown),
            _ => Err(Diagnostic::new(
                format!("type `{type_name}` has no field `{member}`"),
                position,
            )),
        }
    }

    fn filter_compatible_call_candidates<'b>(
        &self,
        candidates: Vec<&'b SubprogramSignature>,
        args: &[CallArg],
        scope: &Scope,
        current_package: Option<&str>,
    ) -> Result<Vec<&'b SubprogramSignature>, Diagnostic> {
        let mut compatible = Vec::new();
        for candidate in candidates {
            if self.call_candidate_accepts_arguments(candidate, args, scope, current_package)? {
                compatible.push(candidate);
            }
        }
        Ok(compatible)
    }

    fn call_candidate_accepts_arguments(
        &self,
        candidate: &SubprogramSignature,
        args: &[CallArg],
        scope: &Scope,
        current_package: Option<&str>,
    ) -> Result<bool, Diagnostic> {
        let Some(bound_args) = self.bind_call_arguments(candidate, args)? else {
            return Ok(false);
        };

        for (param, arg) in candidate.params.iter().zip(bound_args) {
            let Some(arg) = arg else {
                continue;
            };
            if !self.argument_matches_expected_mode(&arg.value, param.mode, scope, current_package)
            {
                return Ok(false);
            }
            if !self.argument_matches_expected_type(
                &arg.value,
                &param.ty,
                candidate.owner_package(),
                scope,
                current_package,
            )? {
                return Ok(false);
            }
        }

        Ok(true)
    }

    fn argument_matches_expected_mode(
        &self,
        arg: &Expr,
        mode: ParamMode,
        scope: &Scope,
        current_package: Option<&str>,
    ) -> bool {
        if mode == ParamMode::In {
            return true;
        }

        self.validate_assignment_target(arg, arg.position(), scope, current_package)
            .is_ok()
    }

    fn bind_call_arguments<'b>(
        &self,
        candidate: &'b SubprogramSignature,
        args: &'b [CallArg],
    ) -> Result<Option<Vec<Option<&'b CallArg>>>, Diagnostic> {
        let mut bound_args = vec![None; candidate.params.len()];
        let mut next_positional = 0usize;

        for arg in args {
            match &arg.name {
                Some(name) => {
                    let Some(index) = candidate
                        .params
                        .iter()
                        .position(|param| param.name == *name)
                    else {
                        return Ok(None);
                    };
                    if bound_args[index].is_some() {
                        return Ok(None);
                    }
                    bound_args[index] = Some(arg);
                }
                None => {
                    while next_positional < bound_args.len()
                        && bound_args[next_positional].is_some()
                    {
                        next_positional += 1;
                    }
                    if next_positional >= bound_args.len() {
                        return Ok(None);
                    }
                    bound_args[next_positional] = Some(arg);
                    next_positional += 1;
                }
            }
        }

        if bound_args
            .iter()
            .zip(&candidate.params)
            .any(|(arg, param)| arg.is_none() && !param.has_default)
        {
            return Ok(None);
        }

        Ok(Some(bound_args))
    }

    fn argument_matches_expected_type(
        &self,
        arg: &Expr,
        expected_type: &str,
        expected_context: Option<&str>,
        scope: &Scope,
        actual_context: Option<&str>,
    ) -> Result<bool, Diagnostic> {
        match self.infer_value_type(arg, scope, actual_context)? {
            InferredType::Known(actual_type) => Ok(self.types_are_compatible_in_contexts(
                expected_type,
                expected_context,
                &actual_type,
                actual_context,
            )),
            InferredType::Aggregate(_) => self.array_literal_matches_named_type(
                arg,
                expected_type,
                expected_context,
                scope,
                actual_context,
            ),
            InferredType::Unknown => Ok(true),
            InferredType::Procedure => Err(Diagnostic::new(
                "procedures do not produce a value",
                arg.position(),
            )),
        }
    }

    fn render_call_argument_types(
        &self,
        args: &[CallArg],
        scope: &Scope,
        current_package: Option<&str>,
    ) -> Result<String, Diagnostic> {
        let mut rendered = Vec::new();
        for arg in args {
            let text = match self.infer_value_type(&arg.value, scope, current_package)? {
                InferredType::Known(ty) => ty,
                InferredType::Aggregate(_) if arg.name.is_some() => "array literal".to_string(),
                InferredType::Aggregate(_) => "array literal".to_string(),
                InferredType::Procedure => "procedure".to_string(),
                InferredType::Unknown => "unknown".to_string(),
            };
            rendered.push(match &arg.name {
                Some(name) => format!("{name} => {text}"),
                None => text,
            });
        }

        Ok(rendered.join(", "))
    }

    fn validate_array_literal_type(
        &self,
        expr: &Expr,
        expected_type: &str,
        context: &str,
        position: Position,
        scope: &Scope,
        current_package: Option<&str>,
    ) -> Result<(), Diagnostic> {
        let Expr::ArrayLiteral { elements, .. } = expr else {
            return Ok(());
        };

        let Some((element_type, expected_length)) =
            self.named_array_type(expected_type, current_package)
        else {
            return if self
                .canonical_type_name(expected_type, current_package)
                .is_some()
            {
                Err(Diagnostic::new(
                    format!("{context} expects `{expected_type}`, found array literal"),
                    position,
                ))
            } else {
                Ok(())
            };
        };

        if let Some(expected_length) = expected_length
            && elements.len() != expected_length
        {
            return Err(Diagnostic::new(
                format!(
                    "{context} expects `{expected_type}` with {expected_length} elements, found array literal with {} elements",
                    elements.len()
                ),
                position,
            ));
        }

        for element in elements {
            self.validate_value_type(
                element,
                Some(&element_type),
                "array element",
                element.position(),
                scope,
                current_package,
            )?;
        }

        Ok(())
    }

    fn validate_expected_value_type(
        &self,
        expr: &Expr,
        expected: ExpectedTypeRef<'_>,
        context: &str,
        position: Position,
        scope: &Scope,
        actual_context: Option<&str>,
    ) -> Result<(), Diagnostic> {
        match self.infer_value_type(expr, scope, actual_context)? {
            InferredType::Known(actual_type) => {
                if self.types_are_compatible_in_contexts(
                    expected.ty,
                    expected.context,
                    &actual_type,
                    actual_context,
                ) {
                    Ok(())
                } else {
                    Err(Diagnostic::new(
                        format!("{context} expects `{}`, found `{actual_type}`", expected.ty),
                        position,
                    ))
                }
            }
            InferredType::Aggregate(_) => {
                if self.array_literal_matches_named_type(
                    expr,
                    expected.ty,
                    expected.context,
                    scope,
                    actual_context,
                )? {
                    Ok(())
                } else {
                    Err(Diagnostic::new(
                        format!("{context} expects `{}`, found array literal", expected.ty),
                        position,
                    ))
                }
            }
            InferredType::Procedure => Err(Diagnostic::new(
                "procedures do not produce a value",
                expr.position(),
            )),
            InferredType::Unknown => Ok(()),
        }
    }

    fn array_literal_matches_named_type(
        &self,
        expr: &Expr,
        expected_type: &str,
        expected_context: Option<&str>,
        scope: &Scope,
        actual_context: Option<&str>,
    ) -> Result<bool, Diagnostic> {
        let Expr::ArrayLiteral { elements, .. } = expr else {
            return Ok(false);
        };

        let Some((element_type, expected_length)) =
            self.named_array_type(expected_type, expected_context)
        else {
            return Ok(false);
        };

        if let Some(expected_length) = expected_length
            && elements.len() != expected_length
        {
            return Ok(false);
        }

        for element in elements {
            if !self.argument_matches_expected_type(
                element,
                &element_type,
                expected_context,
                scope,
                actual_context,
            )? {
                return Ok(false);
            }
        }

        Ok(true)
    }

    fn indexed_element_type(
        &self,
        base_type: &str,
        current_package: Option<&str>,
    ) -> Option<String> {
        if self
            .canonical_type_name(base_type, current_package)
            .is_some_and(|canonical| self.root_canonical_type_name(&canonical) == "String")
        {
            return Some("Character".to_string());
        }

        self.named_array_type(base_type, current_package)
            .map(|(element_type, _)| element_type)
    }

    fn named_array_type(
        &self,
        ty: &str,
        current_package: Option<&str>,
    ) -> Option<(String, Option<usize>)> {
        let canonical = self.canonical_type_name(ty, current_package)?;
        let TypeKind::Array {
            element_type,
            length,
        } = self.lookup_type_kind(&canonical)?
        else {
            return None;
        };

        let package_context =
            split_qualified_type_str(&canonical).map(|(package_name, _)| package_name);
        let resolved_element_type = self
            .canonical_type_name(element_type, package_context.as_deref())
            .unwrap_or_else(|| element_type.clone());
        Some((resolved_element_type, *length))
    }

    fn record_field_type(
        &self,
        ty: &str,
        field: &str,
        current_package: Option<&str>,
    ) -> Option<String> {
        let canonical = self.canonical_type_name(ty, current_package)?;
        self.record_field_types(&canonical)?.get(field).cloned()
    }

    fn record_field_types(&self, canonical_type: &str) -> Option<HashMap<String, String>> {
        let TypeKind::Record { fields } = self.lookup_type_kind(canonical_type)? else {
            return None;
        };

        let package_context = current_package_for_canonical(canonical_type);
        Some(
            fields
                .iter()
                .map(|(field_name, field_type)| {
                    let resolved_type = self
                        .canonical_type_name(field_type, package_context.as_deref())
                        .unwrap_or_else(|| field_type.clone());
                    (field_name.clone(), resolved_type)
                })
                .collect(),
        )
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

        for package_name in &self.source_context.uses {
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
        self.types_are_compatible_in_contexts(
            expected_type,
            current_package,
            actual_type,
            current_package,
        )
    }

    fn types_are_compatible_in_contexts(
        &self,
        expected_type: &str,
        expected_context: Option<&str>,
        actual_type: &str,
        actual_context: Option<&str>,
    ) -> bool {
        let Some(expected_canonical) = self.canonical_type_name(expected_type, expected_context)
        else {
            return true;
        };
        let Some(actual_canonical) = self.canonical_type_name(actual_type, actual_context) else {
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
        for package_name in &self.source_context.uses {
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

    fn is_array_like_type(&self, ty: &str, current_package: Option<&str>) -> Option<bool> {
        let canonical = self.canonical_type_name(ty, current_package)?;
        if self.root_canonical_type_name(&canonical) == "String" {
            return Some(true);
        }

        Some(matches!(
            self.lookup_type_kind(&canonical),
            Some(TypeKind::Array { .. })
        ))
    }

    fn visible_used_internal_packages(&self) -> Vec<&PackageSummary> {
        self.source_context
            .uses
            .iter()
            .filter_map(|name| self.summary.packages.get(name))
            .collect()
    }

    fn package_public_value_info(
        &self,
        package_name: &str,
        value_name: &str,
    ) -> Option<&ValueInfo> {
        self.summary
            .package_spec_values
            .get(package_name)
            .and_then(|values| values.get(value_name))
    }

    fn has_visible_used_package_value(&self, value_name: &str) -> bool {
        self.source_context.uses.iter().any(|package_name| {
            self.package_public_value_info(package_name, value_name)
                .is_some()
        })
    }

    fn visible_used_package_value_info(&self, value_name: &str) -> Option<&ValueInfo> {
        let mut matches =
            self.source_context.uses.iter().filter_map(|package_name| {
                self.package_public_value_info(package_name, value_name)
            });
        let first = matches.next()?;
        if matches.next().is_some() {
            return None;
        }
        Some(first)
    }

    fn visible_used_internal_package_types(&self) -> Vec<&HashSet<String>> {
        self.source_context
            .uses
            .iter()
            .filter_map(|name| self.summary.package_types.get(name))
            .collect()
    }

    fn visible_used_internal_package_enum_literals(&self) -> Vec<&HashSet<String>> {
        self.source_context
            .uses
            .iter()
            .filter_map(|name| self.summary.package_enum_literals.get(name))
            .collect()
    }
}

struct PackageCollections<'a> {
    signatures: &'a mut SignatureIndex,
    values: &'a mut HashMap<String, ValueInfo>,
    type_names: &'a mut HashSet<String>,
    enum_literals: &'a mut HashSet<String>,
    type_kinds: &'a mut HashMap<String, TypeKind>,
    enum_literal_types: &'a mut HashMap<String, Vec<String>>,
}

fn collect_package_items(
    package: &Package,
    source_index: usize,
    collections: PackageCollections<'_>,
) -> Result<(), Diagnostic> {
    let package_name = package.name.as_string();
    let scope_name = if package.is_body {
        format!("package body `{package_name}`")
    } else {
        format!("package `{package_name}`")
    };

    let PackageCollections {
        signatures,
        values,
        type_names,
        enum_literals,
        type_kinds,
        enum_literal_types,
    } = collections;

    for item in &package.items {
        match item {
            PackageItem::Subprogram(subprogram) => {
                insert_signature(
                    signatures,
                    SubprogramSignature::from_subprogram(
                        subprogram,
                        source_index,
                        Some(&package_name),
                    ),
                    subprogram.position,
                    &format!("duplicate subprogram signature in {scope_name}"),
                )?;
            }
            PackageItem::Type(type_decl) => {
                validate_type_decl(type_decl, type_names, &scope_name)?;
                collect_type_symbols(type_decl, type_names, enum_literals);
                collect_type_metadata(type_decl, type_kinds, enum_literal_types);
            }
            PackageItem::Object(decl) => {
                if values
                    .insert(
                        decl.name.clone(),
                        ValueInfo {
                            ty: decl.ty.as_string(),
                            assignable: !decl.is_const,
                        },
                    )
                    .is_some()
                {
                    return Err(Diagnostic::new(
                        format!("duplicate object `{}` in {scope_name}", decl.name),
                        decl.position,
                    ));
                }
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
        TypeDecl::Array(array_type) => {
            if !seen_names.insert(array_type.name.clone()) {
                return Err(Diagnostic::new(
                    format!("duplicate type `{}` in {scope_name}", array_type.name),
                    array_type.position,
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

fn find_signature<'a>(
    index: &'a SignatureIndex,
    key: &SignatureKey,
) -> Option<&'a SubprogramSignature> {
    index
        .get(&key.name)?
        .iter()
        .find(|signature| signature.key == *key)
}

fn validate_definition_matches_declaration(
    declaration: &SubprogramSignature,
    definition: &SubprogramSignature,
) -> Result<(), Diagnostic> {
    for (decl_param, def_param) in declaration.params.iter().zip(&definition.params) {
        if decl_param.name != def_param.name {
            return Err(Diagnostic::new(
                format!(
                    "definition of `{}` must use parameter name `{}` to match its declaration",
                    definition.key.name, decl_param.name
                ),
                definition.position,
            ));
        }

        if !optional_exprs_match(&decl_param.default, &def_param.default) {
            return Err(Diagnostic::new(
                format!(
                    "definition of `{}` must repeat the default for parameter `{}`",
                    definition.key.name, decl_param.name
                ),
                definition.position,
            ));
        }
    }

    if !(definition.requires.is_empty() && definition.ensures.is_empty()) {
        if declaration.requires.is_empty() && declaration.ensures.is_empty() {
            return Err(Diagnostic::new(
                format!(
                    "definition of `{}` cannot introduce contracts that are absent from its declaration",
                    definition.key.name
                ),
                definition.position,
            ));
        }

        if !expr_lists_match(&declaration.requires, &definition.requires)
            || !expr_lists_match(&declaration.ensures, &definition.ensures)
        {
            return Err(Diagnostic::new(
                format!(
                    "definition of `{}` must use the same contracts as its declaration",
                    definition.key.name
                ),
                definition.position,
            ));
        }
    }

    if definition.global.is_some() || definition.depends.is_some() {
        if declaration.global.is_none() && declaration.depends.is_none() {
            return Err(Diagnostic::new(
                format!(
                    "definition of `{}` cannot introduce dataflow contracts that are absent from its declaration",
                    definition.key.name
                ),
                definition.position,
            ));
        }
        if declaration.global != definition.global || declaration.depends != definition.depends {
            return Err(Diagnostic::new(
                format!(
                    "definition of `{}` must use the same dataflow contracts as its declaration",
                    definition.key.name
                ),
                definition.position,
            ));
        }
    }

    Ok(())
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

fn optional_exprs_match(lhs: &Option<Expr>, rhs: &Option<Expr>) -> bool {
    match (lhs, rhs) {
        (Some(lhs), Some(rhs)) => exprs_match(lhs, rhs),
        (None, None) => true,
        _ => false,
    }
}

fn expr_lists_match(lhs: &[Expr], rhs: &[Expr]) -> bool {
    lhs.len() == rhs.len() && lhs.iter().zip(rhs).all(|(lhs, rhs)| exprs_match(lhs, rhs))
}

fn exprs_match(lhs: &Expr, rhs: &Expr) -> bool {
    match (lhs, rhs) {
        (Expr::Bool { value: lhs, .. }, Expr::Bool { value: rhs, .. }) => lhs == rhs,
        (Expr::Integer { value: lhs, .. }, Expr::Integer { value: rhs, .. }) => lhs == rhs,
        (Expr::Float { value: lhs, .. }, Expr::Float { value: rhs, .. }) => lhs == rhs,
        (Expr::Character { value: lhs, .. }, Expr::Character { value: rhs, .. }) => lhs == rhs,
        (Expr::String { value: lhs, .. }, Expr::String { value: rhs, .. }) => lhs == rhs,
        (Expr::Name { name: lhs, .. }, Expr::Name { name: rhs, .. }) => lhs == rhs,
        (
            Expr::Member {
                base: lhs_base,
                member: lhs_member,
                ..
            },
            Expr::Member {
                base: rhs_base,
                member: rhs_member,
                ..
            },
        ) => lhs_member == rhs_member && exprs_match(lhs_base, rhs_base),
        (
            Expr::Index {
                base: lhs_base,
                index: lhs_index,
                ..
            },
            Expr::Index {
                base: rhs_base,
                index: rhs_index,
                ..
            },
        ) => exprs_match(lhs_base, rhs_base) && exprs_match(lhs_index, rhs_index),
        (
            Expr::Call {
                callee: lhs_callee,
                args: lhs_args,
                ..
            },
            Expr::Call {
                callee: rhs_callee,
                args: rhs_args,
                ..
            },
        ) => {
            exprs_match(lhs_callee, rhs_callee)
                && lhs_args.len() == rhs_args.len()
                && lhs_args.iter().zip(rhs_args).all(|(lhs_arg, rhs_arg)| {
                    lhs_arg.name == rhs_arg.name && exprs_match(&lhs_arg.value, &rhs_arg.value)
                })
        }
        (
            Expr::RecordLiteral {
                ty: lhs_ty,
                fields: lhs_fields,
                ..
            },
            Expr::RecordLiteral {
                ty: rhs_ty,
                fields: rhs_fields,
                ..
            },
        ) => {
            lhs_ty == rhs_ty
                && lhs_fields.len() == rhs_fields.len()
                && lhs_fields
                    .iter()
                    .zip(rhs_fields)
                    .all(|(lhs_field, rhs_field)| {
                        lhs_field.name == rhs_field.name
                            && exprs_match(&lhs_field.value, &rhs_field.value)
                    })
        }
        (
            Expr::ArrayLiteral {
                elements: lhs_elements,
                ..
            },
            Expr::ArrayLiteral {
                elements: rhs_elements,
                ..
            },
        ) => {
            lhs_elements.len() == rhs_elements.len()
                && lhs_elements
                    .iter()
                    .zip(rhs_elements)
                    .all(|(lhs, rhs)| exprs_match(lhs, rhs))
        }
        (
            Expr::Unary {
                op: lhs_op,
                expr: lhs_expr,
                ..
            },
            Expr::Unary {
                op: rhs_op,
                expr: rhs_expr,
                ..
            },
        ) => lhs_op == rhs_op && exprs_match(lhs_expr, rhs_expr),
        (
            Expr::Binary {
                lhs: lhs_left,
                op: lhs_op,
                rhs: lhs_right,
                ..
            },
            Expr::Binary {
                lhs: rhs_left,
                op: rhs_op,
                rhs: rhs_right,
                ..
            },
        ) => {
            lhs_op == rhs_op && exprs_match(lhs_left, rhs_left) && exprs_match(lhs_right, rhs_right)
        }
        _ => false,
    }
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
        TypeDecl::Array(array_type) => {
            type_names.insert(array_type.name.clone());
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
            type_kinds.insert(
                record.name.clone(),
                TypeKind::Record {
                    fields: record
                        .fields
                        .iter()
                        .map(|field| (field.name.clone(), field.ty.as_string()))
                        .collect(),
                },
            );
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
        TypeDecl::Array(array_type) => {
            type_kinds.insert(
                array_type.name.clone(),
                TypeKind::Array {
                    element_type: array_type.element_type.as_string(),
                    length: static_array_length(&array_type.start, &array_type.end),
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

fn current_package_for_canonical(canonical_type: &str) -> Option<String> {
    split_qualified_type_str(canonical_type).map(|(package_name, _)| package_name)
}

fn is_readonly_attribute(member: &str) -> bool {
    matches!(member, "length" | "range" | "image")
}

fn display_expr(expr: &Expr) -> String {
    match expr {
        Expr::Bool { value, .. } => {
            if *value {
                "true".to_string()
            } else {
                "false".to_string()
            }
        }
        Expr::Integer { value, .. } => value.clone(),
        Expr::Float { value, .. } => value.clone(),
        Expr::Character { value, .. } => format!("'{}'", value),
        Expr::String { value, .. } => format!("{value:?}"),
        Expr::Name { name, .. } => name.as_string(),
        Expr::Member { base, member, .. } => format!("{}.{}", display_expr(base), member),
        Expr::Index { base, index, .. } => {
            format!("{}[{}]", display_expr(base), display_expr(index))
        }
        Expr::Call { .. } => display_callee(expr),
        Expr::RecordLiteral { ty, .. } => format!("{} {{ ... }}", ty.as_string()),
        Expr::ArrayLiteral { .. } => "array literal".to_string(),
        Expr::Unary { .. } | Expr::Binary { .. } => "<expr>".to_string(),
    }
}

fn integer_constant_value(expr: &Expr) -> Option<i128> {
    match expr {
        Expr::Integer { value, .. } => value.parse().ok(),
        Expr::Unary {
            op: UnaryOp::Negate,
            expr,
            ..
        } => integer_constant_value(expr).map(|value| -value),
        _ => None,
    }
}

fn static_array_length(start: &Expr, end: &Expr) -> Option<usize> {
    let start = integer_constant_value(start)?;
    let end = integer_constant_value(end)?;
    let length = if end < start { 0 } else { end - start + 1 };
    usize::try_from(length).ok()
}

fn find_result_reference(expr: &Expr) -> Option<Position> {
    match expr {
        Expr::Name { name, position }
            if name.segments.len() == 1 && name.segments[0] == "result" =>
        {
            Some(*position)
        }
        Expr::Name { .. }
        | Expr::Bool { .. }
        | Expr::Integer { .. }
        | Expr::Float { .. }
        | Expr::Character { .. }
        | Expr::String { .. } => None,
        Expr::Member { base, .. } => find_result_reference(base),
        Expr::Index { base, index, .. } => {
            find_result_reference(base).or_else(|| find_result_reference(index))
        }
        Expr::Call { callee, args, .. } => find_result_reference(callee).or_else(|| {
            args.iter()
                .find_map(|arg| find_result_reference(&arg.value))
        }),
        Expr::RecordLiteral { fields, .. } => fields
            .iter()
            .find_map(|field| find_result_reference(&field.value)),
        Expr::ArrayLiteral { elements, .. } => elements.iter().find_map(find_result_reference),
        Expr::Unary { expr, .. } => find_result_reference(expr),
        Expr::Binary { lhs, rhs, .. } => {
            find_result_reference(lhs).or_else(|| find_result_reference(rhs))
        }
    }
}
