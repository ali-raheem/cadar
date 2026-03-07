use std::collections::HashSet;

use crate::{
    ast::{
        BinaryOp, BlockItem, CaseStatement, EnumType, Expr, ForStatement, IfStatement, Item,
        LocalDecl, Name, Package, PackageItem, ParamMode, Program, RangeType, RecordType,
        Statement, StatementBlock, Subprogram, TypeDecl, UnaryOp,
    },
    diagnostic::Diagnostic,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdaOutputs {
    pub spec: String,
    pub body: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeneratedFile {
    pub filename: String,
    pub contents: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdaProgram {
    pub context: Vec<AdaContextItem>,
    pub spec_units: Vec<AdaSpecUnit>,
    pub body_units: Vec<AdaBodyUnit>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AdaSpecUnit {
    Subprogram(AdaSubprogramSpec),
    Type(AdaTypeDecl),
    Package(AdaPackageSpec),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AdaBodyUnit {
    Subprogram(AdaSubprogramBody),
    Package(AdaPackageBody),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AdaContextKind {
    With,
    Use,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdaContextItem {
    pub kind: AdaContextKind,
    pub name: Name,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AdaSubprogramSpec {
    pub name: String,
    pub params: Vec<AdaParam>,
    pub return_type: Option<Name>,
    pub preconditions: Vec<AdaExpr>,
    pub postconditions: Vec<AdaExpr>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AdaParam {
    pub mode: ParamMode,
    pub ty: Name,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdaSubprogramBody {
    pub spec: AdaSubprogramSpec,
    pub declarations: Vec<AdaObjectDecl>,
    pub statements: Vec<AdaStatement>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdaPackageSpec {
    pub name: Name,
    pub items: Vec<AdaPackageSpecItem>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdaPackageBody {
    pub name: Name,
    pub items: Vec<AdaPackageBodyItem>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AdaPackageSpecItem {
    Subprogram(AdaSubprogramSpec),
    Type(AdaTypeDecl),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AdaPackageBodyItem {
    Subprogram(AdaSubprogramBody),
    Type(AdaTypeDecl),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AdaTypeDecl {
    Record(AdaRecordType),
    Enum(AdaEnumType),
    Range(AdaRangeType),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdaRecordType {
    pub name: String,
    pub fields: Vec<AdaRecordField>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdaRecordField {
    pub ty: Name,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdaEnumType {
    pub name: String,
    pub variants: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdaRangeType {
    pub name: String,
    pub base: Name,
    pub start: AdaExpr,
    pub end: AdaExpr,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdaObjectDecl {
    pub is_const: bool,
    pub ty: Name,
    pub name: String,
    pub initializer: Option<AdaExpr>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AdaStatement {
    Null,
    Return(AdaExpr),
    Assign {
        target: Name,
        value: AdaExpr,
    },
    Call(AdaExpr),
    If(AdaIfStatement),
    Case(AdaCaseStatement),
    While {
        condition: AdaExpr,
        body: Vec<AdaStatement>,
    },
    For {
        iterator: String,
        start: AdaExpr,
        end: AdaExpr,
        body: Vec<AdaStatement>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdaIfStatement {
    pub condition: AdaExpr,
    pub then_branch: Vec<AdaStatement>,
    pub else_if_branches: Vec<AdaElseIfBranch>,
    pub else_branch: Option<Vec<AdaStatement>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdaCaseStatement {
    pub expr: AdaExpr,
    pub arms: Vec<AdaCaseArm>,
    pub else_arm: Option<Vec<AdaStatement>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdaCaseArm {
    pub choices: Vec<AdaExpr>,
    pub body: Vec<AdaStatement>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdaElseIfBranch {
    pub condition: AdaExpr,
    pub body: Vec<AdaStatement>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AdaAttribute {
    Length,
    Range,
    Image,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum AdaExpr {
    Bool(bool),
    Integer(String),
    String(String),
    Name(Name),
    Result(String),
    Qualified {
        prefix: Box<AdaExpr>,
        member: String,
    },
    Attribute {
        prefix: Box<AdaExpr>,
        attribute: AdaAttribute,
    },
    Call {
        callee: Box<AdaExpr>,
        args: Vec<AdaExpr>,
    },
    Unary {
        op: UnaryOp,
        expr: Box<AdaExpr>,
    },
    Binary {
        lhs: Box<AdaExpr>,
        op: BinaryOp,
        rhs: Box<AdaExpr>,
    },
}

pub fn lower(program: Program) -> Result<AdaProgram, Diagnostic> {
    let mut context = Vec::new();
    let mut spec_units = Vec::new();
    let mut body_units = Vec::new();
    let mut seen_specs = HashSet::new();
    let mut seen_package_specs = HashSet::new();
    let explicit_package_specs: HashSet<Name> = program
        .items
        .iter()
        .filter_map(|item| match item {
            Item::Package(package) if !package.is_body => Some(package.name.clone()),
            _ => None,
        })
        .collect();

    for item in program.items {
        match item {
            Item::Import(name) => context.push(AdaContextItem {
                kind: AdaContextKind::With,
                name,
            }),
            Item::Use(name) => context.push(AdaContextItem {
                kind: AdaContextKind::Use,
                name,
            }),
            Item::Subprogram(subprogram) => {
                let spec = lower_spec(&subprogram)?;
                if seen_specs.insert(spec.clone()) {
                    spec_units.push(AdaSpecUnit::Subprogram(spec.clone()));
                }
                if let Some(body) = subprogram.body {
                    body_units.push(AdaBodyUnit::Subprogram(lower_body(spec, body.items)?));
                }
            }
            Item::Type(type_decl) => {
                spec_units.push(AdaSpecUnit::Type(lower_type_decl(type_decl)));
            }
            Item::Package(package) => match lower_package(package)? {
                LoweredPackage::Spec(package_spec) => {
                    seen_package_specs.insert(package_spec.name.clone());
                    spec_units.push(AdaSpecUnit::Package(package_spec));
                }
                LoweredPackage::Body { derived_spec, body } => {
                    if !explicit_package_specs.contains(&derived_spec.name)
                        && seen_package_specs.insert(derived_spec.name.clone())
                    {
                        spec_units.push(AdaSpecUnit::Package(derived_spec));
                    }
                    body_units.push(AdaBodyUnit::Package(body));
                }
            },
        }
    }

    Ok(AdaProgram {
        context,
        spec_units,
        body_units,
    })
}

enum LoweredPackage {
    Spec(AdaPackageSpec),
    Body {
        derived_spec: AdaPackageSpec,
        body: AdaPackageBody,
    },
}

fn lower_package(package: Package) -> Result<LoweredPackage, Diagnostic> {
    if package.is_body {
        let mut spec_items = Vec::new();
        let mut body_items = Vec::new();
        for item in package.items {
            match item {
                PackageItem::Subprogram(subprogram) => {
                    let spec = lower_spec(&subprogram)?;
                    spec_items.push(AdaPackageSpecItem::Subprogram(spec.clone()));
                    let Some(body) = subprogram.body else {
                        return Err(Diagnostic::new(
                            "package bodies must contain subprogram definitions",
                            subprogram.position,
                        ));
                    };
                    body_items.push(AdaPackageBodyItem::Subprogram(lower_body(
                        spec, body.items,
                    )?));
                }
                PackageItem::Type(type_decl) => {
                    body_items.push(AdaPackageBodyItem::Type(lower_type_decl(type_decl)));
                }
            }
        }
        Ok(LoweredPackage::Body {
            derived_spec: AdaPackageSpec {
                name: package.name.clone(),
                items: spec_items,
            },
            body: AdaPackageBody {
                name: package.name,
                items: body_items,
            },
        })
    } else {
        let mut items = Vec::new();
        for item in package.items {
            match item {
                PackageItem::Subprogram(subprogram) => {
                    if subprogram.body.is_some() {
                        return Err(Diagnostic::new(
                            "package specifications cannot contain subprogram bodies",
                            subprogram.position,
                        ));
                    }
                    items.push(AdaPackageSpecItem::Subprogram(lower_spec(&subprogram)?));
                }
                PackageItem::Type(type_decl) => {
                    items.push(AdaPackageSpecItem::Type(lower_type_decl(type_decl)));
                }
            }
        }
        Ok(LoweredPackage::Spec(AdaPackageSpec {
            name: package.name,
            items,
        }))
    }
}

fn lower_type_decl(type_decl: TypeDecl) -> AdaTypeDecl {
    match type_decl {
        TypeDecl::Record(RecordType { name, fields, .. }) => AdaTypeDecl::Record(AdaRecordType {
            name,
            fields: fields
                .into_iter()
                .map(|field| AdaRecordField {
                    ty: field.ty,
                    name: field.name,
                })
                .collect(),
        }),
        TypeDecl::Enum(EnumType { name, variants, .. }) => {
            AdaTypeDecl::Enum(AdaEnumType { name, variants })
        }
        TypeDecl::Range(RangeType {
            name,
            base,
            start,
            end,
            ..
        }) => AdaTypeDecl::Range(AdaRangeType {
            name,
            base,
            start: lower_expr(start),
            end: lower_expr(end),
        }),
    }
}

fn lower_spec(subprogram: &Subprogram) -> Result<AdaSubprogramSpec, Diagnostic> {
    let preconditions = subprogram
        .requires
        .iter()
        .cloned()
        .map(lower_expr)
        .collect();
    let postconditions = subprogram
        .ensures
        .iter()
        .cloned()
        .map(|expr| lower_contract_expr(expr, &subprogram.name, subprogram.return_type.is_some()))
        .collect::<Result<Vec<_>, Diagnostic>>()?;
    Ok(AdaSubprogramSpec {
        name: subprogram.name.clone(),
        params: subprogram
            .params
            .iter()
            .map(|param| AdaParam {
                mode: param.mode,
                ty: param.ty.clone(),
                name: param.name.clone(),
            })
            .collect(),
        return_type: subprogram.return_type.clone(),
        preconditions,
        postconditions,
    })
}

fn lower_body(
    spec: AdaSubprogramSpec,
    items: Vec<BlockItem>,
) -> Result<AdaSubprogramBody, Diagnostic> {
    let mut declarations = Vec::new();
    let mut statements = Vec::new();
    let mut saw_statement = false;

    for item in items {
        match item {
            BlockItem::LocalDecl(decl) => {
                if saw_statement {
                    return Err(Diagnostic::new(
                        "local declarations must appear before statements in a subprogram body",
                        decl.position,
                    ));
                }
                declarations.push(lower_decl(decl)?);
            }
            BlockItem::Statement(statement) => {
                saw_statement = true;
                statements.push(lower_statement(statement)?);
            }
        }
    }

    Ok(AdaSubprogramBody {
        spec,
        declarations,
        statements,
    })
}

fn lower_decl(decl: LocalDecl) -> Result<AdaObjectDecl, Diagnostic> {
    if decl.is_const && decl.initializer.is_none() {
        return Err(Diagnostic::new(
            format!("constant `{}` requires an initializer", decl.name),
            decl.position,
        ));
    }

    Ok(AdaObjectDecl {
        is_const: decl.is_const,
        ty: decl.ty,
        name: decl.name,
        initializer: decl.initializer.map(lower_expr),
    })
}

fn lower_statement(statement: Statement) -> Result<AdaStatement, Diagnostic> {
    match statement {
        Statement::Null { .. } => Ok(AdaStatement::Null),
        Statement::Return { expr, .. } => Ok(AdaStatement::Return(lower_expr(expr))),
        Statement::Assign { target, value, .. } => Ok(AdaStatement::Assign {
            target,
            value: lower_expr(value),
        }),
        Statement::Expr { expr, position } => {
            let lowered = lower_expr(expr);
            if matches!(lowered, AdaExpr::Call { .. }) {
                Ok(AdaStatement::Call(lowered))
            } else {
                Err(Diagnostic::new(
                    "only call expressions are allowed as standalone statements",
                    position,
                ))
            }
        }
        Statement::If(if_statement) => Ok(AdaStatement::If(lower_if_statement(if_statement)?)),
        Statement::Case(case_statement) => {
            Ok(AdaStatement::Case(lower_case_statement(case_statement)?))
        }
        Statement::While {
            condition, body, ..
        } => Ok(AdaStatement::While {
            condition: lower_expr(condition),
            body: lower_statement_block(body)?,
        }),
        Statement::For(ForStatement {
            iterator_type: _,
            iterator,
            start,
            end,
            body,
            ..
        }) => Ok(AdaStatement::For {
            iterator,
            start: lower_expr(start),
            end: lower_expr(end),
            body: lower_statement_block(body)?,
        }),
    }
}

fn lower_if_statement(statement: IfStatement) -> Result<AdaIfStatement, Diagnostic> {
    Ok(AdaIfStatement {
        condition: lower_expr(statement.condition),
        then_branch: lower_statement_block(statement.then_branch)?,
        else_if_branches: statement
            .else_if_branches
            .into_iter()
            .map(|branch| {
                Ok(AdaElseIfBranch {
                    condition: lower_expr(branch.condition),
                    body: lower_statement_block(branch.body)?,
                })
            })
            .collect::<Result<Vec<_>, Diagnostic>>()?,
        else_branch: statement
            .else_branch
            .map(lower_statement_block)
            .transpose()?,
    })
}

fn lower_case_statement(statement: CaseStatement) -> Result<AdaCaseStatement, Diagnostic> {
    Ok(AdaCaseStatement {
        expr: lower_expr(statement.expr),
        arms: statement
            .arms
            .into_iter()
            .map(|arm| {
                Ok(AdaCaseArm {
                    choices: arm.choices.into_iter().map(lower_expr).collect(),
                    body: lower_statement_block(arm.body)?,
                })
            })
            .collect::<Result<Vec<_>, Diagnostic>>()?,
        else_arm: statement.else_arm.map(lower_statement_block).transpose()?,
    })
}

fn lower_statement_block(block: StatementBlock) -> Result<Vec<AdaStatement>, Diagnostic> {
    block
        .statements
        .into_iter()
        .map(lower_statement)
        .collect::<Result<Vec<_>, Diagnostic>>()
}

fn lower_contract_expr(
    expr: Expr,
    subprogram_name: &str,
    allow_result: bool,
) -> Result<AdaExpr, Diagnostic> {
    lower_expr_internal(
        expr,
        Some(ContractContext {
            subprogram_name,
            allow_result,
        }),
    )
}

fn lower_expr(expr: Expr) -> AdaExpr {
    lower_expr_internal(expr, None).expect("runtime expression lowering should not fail")
}

#[derive(Clone, Copy)]
struct ContractContext<'a> {
    subprogram_name: &'a str,
    allow_result: bool,
}

fn lower_expr_internal(
    expr: Expr,
    contract: Option<ContractContext<'_>>,
) -> Result<AdaExpr, Diagnostic> {
    match expr {
        Expr::Bool { value, .. } => Ok(AdaExpr::Bool(value)),
        Expr::Integer { value, .. } => Ok(AdaExpr::Integer(value)),
        Expr::String { value, .. } => Ok(AdaExpr::String(value)),
        Expr::Name { name, position } => {
            if let Some(contract) = contract
                && name.segments.len() == 1
                && name.segments[0] == "result"
            {
                if contract.allow_result {
                    return Ok(AdaExpr::Result(contract.subprogram_name.to_string()));
                }
                return Err(Diagnostic::new(
                    "`result` is only valid in postconditions of functions",
                    position,
                ));
            }
            Ok(AdaExpr::Name(name))
        }
        Expr::Member { base, member, .. } => {
            let prefix = Box::new(lower_expr_internal(*base, contract)?);
            match member.as_str() {
                "length" => Ok(AdaExpr::Attribute {
                    prefix,
                    attribute: AdaAttribute::Length,
                }),
                "range" => Ok(AdaExpr::Attribute {
                    prefix,
                    attribute: AdaAttribute::Range,
                }),
                "image" => Ok(AdaExpr::Attribute {
                    prefix,
                    attribute: AdaAttribute::Image,
                }),
                _ => Ok(AdaExpr::Qualified { prefix, member }),
            }
        }
        Expr::Call { callee, args, .. } => Ok(AdaExpr::Call {
            callee: Box::new(lower_expr_internal(*callee, contract)?),
            args: args
                .into_iter()
                .map(|arg| lower_expr_internal(arg, contract))
                .collect::<Result<Vec<_>, Diagnostic>>()?,
        }),
        Expr::Unary { op, expr, .. } => Ok(AdaExpr::Unary {
            op,
            expr: Box::new(lower_expr_internal(*expr, contract)?),
        }),
        Expr::Binary { lhs, op, rhs, .. } => Ok(AdaExpr::Binary {
            lhs: Box::new(lower_expr_internal(*lhs, contract)?),
            op,
            rhs: Box::new(lower_expr_internal(*rhs, contract)?),
        }),
    }
}

pub fn render(program: &AdaProgram) -> AdaOutputs {
    AdaOutputs {
        spec: render_spec(program),
        body: render_body(program),
    }
}

pub fn render_files(program: &AdaProgram, fallback_stem: &str) -> Vec<GeneratedFile> {
    let mut files = Vec::new();
    let mut aggregate_spec_blocks = Vec::new();

    for unit in &program.spec_units {
        match unit {
            AdaSpecUnit::Subprogram(spec) => {
                let self_name = Name {
                    segments: vec![spec.name.clone()],
                };
                let context = render_context_for_unit(&program.context, Some(&self_name));
                files.push(GeneratedFile {
                    filename: format!("{}.ads", unit_stem_from_identifier(&spec.name)),
                    contents: compose_file(&context, render_subprogram_spec_lines(spec, 0)),
                });
            }
            AdaSpecUnit::Type(type_decl) => {
                aggregate_spec_blocks.push(render_type_decl(type_decl, 0))
            }
            AdaSpecUnit::Package(package) => {
                let context = render_context_for_unit(&program.context, Some(&package.name));
                files.push(GeneratedFile {
                    filename: format!("{}.ads", unit_stem_from_name(&package.name)),
                    contents: compose_file(&context, render_package_spec(package)),
                });
            }
        }
    }

    if !aggregate_spec_blocks.is_empty() {
        let context = render_context(&program.context);
        files.push(GeneratedFile {
            filename: format!("{fallback_stem}.ads"),
            contents: compose_blocks(&context, aggregate_spec_blocks),
        });
    }

    for unit in &program.body_units {
        match unit {
            AdaBodyUnit::Subprogram(body) => {
                let self_name = Name {
                    segments: vec![body.spec.name.clone()],
                };
                let context = render_context_for_unit(&program.context, Some(&self_name));
                files.push(GeneratedFile {
                    filename: format!("{}.adb", unit_stem_from_identifier(&body.spec.name)),
                    contents: compose_file(&context, render_subprogram_body(body, 0)),
                });
            }
            AdaBodyUnit::Package(package) => {
                let context = render_context_for_unit(&program.context, Some(&package.name));
                files.push(GeneratedFile {
                    filename: format!("{}.adb", unit_stem_from_name(&package.name)),
                    contents: compose_file(&context, render_package_body(package)),
                });
            }
        }
    }

    files
}

fn render_spec(program: &AdaProgram) -> String {
    let mut lines = render_context(&program.context);
    if !program.context.is_empty() && !program.spec_units.is_empty() {
        lines.push(String::new());
    }

    for (index, unit) in program.spec_units.iter().enumerate() {
        match unit {
            AdaSpecUnit::Subprogram(spec) => lines.extend(render_subprogram_spec_lines(spec, 0)),
            AdaSpecUnit::Type(type_decl) => lines.extend(render_type_decl(type_decl, 0)),
            AdaSpecUnit::Package(package) => lines.extend(render_package_spec(package)),
        }
        if index + 1 != program.spec_units.len() {
            lines.push(String::new());
        }
    }

    lines.join("\n")
}

fn render_body(program: &AdaProgram) -> String {
    let mut lines = render_context(&program.context);
    if !program.context.is_empty() && !program.body_units.is_empty() {
        lines.push(String::new());
    }

    for (index, unit) in program.body_units.iter().enumerate() {
        match unit {
            AdaBodyUnit::Subprogram(body) => lines.extend(render_subprogram_body(body, 0)),
            AdaBodyUnit::Package(package) => lines.extend(render_package_body(package)),
        }
        if index + 1 != program.body_units.len() {
            lines.push(String::new());
        }
    }

    lines.join("\n")
}

fn render_context(context: &[AdaContextItem]) -> Vec<String> {
    context.iter().map(render_context_item).collect()
}

fn render_context_for_unit(context: &[AdaContextItem], self_name: Option<&Name>) -> Vec<String> {
    context
        .iter()
        .filter(|item| match self_name {
            Some(self_name) => item.name != *self_name,
            None => true,
        })
        .map(render_context_item)
        .collect()
}

fn render_context_item(item: &AdaContextItem) -> String {
    match item.kind {
        AdaContextKind::With => format!("with {};", item.name.as_string()),
        AdaContextKind::Use => format!("use {};", item.name.as_string()),
    }
}

fn render_package_spec(package: &AdaPackageSpec) -> Vec<String> {
    let mut lines = vec![format!("package {} is", package.name.as_string())];

    for item in &package.items {
        match item {
            AdaPackageSpecItem::Subprogram(spec) => {
                lines.extend(render_subprogram_spec_lines(spec, 1));
            }
            AdaPackageSpecItem::Type(type_decl) => lines.extend(render_type_decl(type_decl, 1)),
        }
    }

    lines.push(format!("end {};", package.name.as_string()));
    lines
}

fn render_package_body(package: &AdaPackageBody) -> Vec<String> {
    let mut lines = vec![format!("package body {} is", package.name.as_string())];

    for (index, item) in package.items.iter().enumerate() {
        match item {
            AdaPackageBodyItem::Subprogram(body) => lines.extend(render_subprogram_body(body, 1)),
            AdaPackageBodyItem::Type(type_decl) => lines.extend(render_type_decl(type_decl, 1)),
        }
        if index + 1 != package.items.len() {
            lines.push(String::new());
        }
    }

    lines.push(format!("end {};", package.name.as_string()));
    lines
}

fn render_type_decl(type_decl: &AdaTypeDecl, indent_level: usize) -> Vec<String> {
    let base_indent = indent(indent_level);
    match type_decl {
        AdaTypeDecl::Record(record) => {
            let field_indent = indent(indent_level + 1);
            let mut lines = vec![format!("{base_indent}type {} is record", record.name)];
            for field in &record.fields {
                lines.push(format!(
                    "{field_indent}{} : {};",
                    field.name,
                    field.ty.as_string()
                ));
            }
            lines.push(format!("{base_indent}end record;"));
            lines
        }
        AdaTypeDecl::Enum(enum_type) => vec![format!(
            "{base_indent}type {} is ({});",
            enum_type.name,
            enum_type.variants.join(", ")
        )],
        AdaTypeDecl::Range(range_type) => vec![format!(
            "{base_indent}subtype {} is {} range {} .. {};",
            range_type.name,
            range_type.base.as_string(),
            render_expr(&range_type.start, 0),
            render_expr(&range_type.end, 0)
        )],
    }
}

fn render_subprogram_body(body: &AdaSubprogramBody, indent_level: usize) -> Vec<String> {
    let base_indent = indent(indent_level);
    let declaration_indent = indent(indent_level + 1);
    let mut lines = vec![format!(
        "{}{} is",
        base_indent,
        render_subprogram_signature(&body.spec)
    )];

    for declaration in &body.declarations {
        lines.push(format!(
            "{declaration_indent}{}",
            render_object_decl(declaration)
        ));
    }

    lines.push(format!("{base_indent}begin"));
    lines.extend(render_nested_block(&body.statements, indent_level + 1));
    lines.push(format!("{base_indent}end {};", body.spec.name));

    lines
}

fn render_object_decl(declaration: &AdaObjectDecl) -> String {
    let type_prefix = if declaration.is_const {
        format!("constant {}", declaration.ty.as_string())
    } else {
        declaration.ty.as_string()
    };

    match &declaration.initializer {
        Some(initializer) => format!(
            "{} : {} := {};",
            declaration.name,
            type_prefix,
            render_expr(initializer, 0)
        ),
        None => format!("{} : {};", declaration.name, type_prefix),
    }
}

fn render_statement(statement: &AdaStatement, indent_level: usize) -> Vec<String> {
    let indent = indent(indent_level);
    match statement {
        AdaStatement::Null => vec![format!("{indent}null;")],
        AdaStatement::Return(expr) => vec![format!("{indent}return {};", render_expr(expr, 0))],
        AdaStatement::Assign { target, value } => {
            vec![format!(
                "{indent}{} := {};",
                target.as_string(),
                render_expr(value, 0)
            )]
        }
        AdaStatement::Call(expr) => vec![format!("{indent}{};", render_expr(expr, 0))],
        AdaStatement::If(if_statement) => render_if_statement(if_statement, indent_level),
        AdaStatement::Case(case_statement) => render_case_statement(case_statement, indent_level),
        AdaStatement::While { condition, body } => {
            let mut lines = vec![format!("{indent}while {} loop", render_expr(condition, 0))];
            lines.extend(render_nested_block(body, indent_level + 1));
            lines.push(format!("{indent}end loop;"));
            lines
        }
        AdaStatement::For {
            iterator,
            start,
            end,
            body,
        } => {
            let mut lines = vec![format!(
                "{indent}for {iterator} in {} .. {} loop",
                render_expr(start, 0),
                render_expr(end, 0)
            )];
            lines.extend(render_nested_block(body, indent_level + 1));
            lines.push(format!("{indent}end loop;"));
            lines
        }
    }
}

fn render_if_statement(statement: &AdaIfStatement, indent_level: usize) -> Vec<String> {
    let indent = indent(indent_level);
    let mut lines = vec![format!(
        "{indent}if {} then",
        render_expr(&statement.condition, 0)
    )];

    lines.extend(render_nested_block(
        &statement.then_branch,
        indent_level + 1,
    ));

    for branch in &statement.else_if_branches {
        lines.push(format!(
            "{indent}elsif {} then",
            render_expr(&branch.condition, 0)
        ));
        lines.extend(render_nested_block(&branch.body, indent_level + 1));
    }

    if let Some(else_branch) = &statement.else_branch {
        lines.push(format!("{indent}else"));
        lines.extend(render_nested_block(else_branch, indent_level + 1));
    }

    lines.push(format!("{indent}end if;"));
    lines
}

fn render_case_statement(statement: &AdaCaseStatement, indent_level: usize) -> Vec<String> {
    let base_indent = indent(indent_level);
    let arm_indent = indent(indent_level + 1);
    let mut lines = vec![format!(
        "{base_indent}case {} is",
        render_expr(&statement.expr, 0)
    )];

    for arm in &statement.arms {
        let choices = arm
            .choices
            .iter()
            .map(|choice| render_expr(choice, 0))
            .collect::<Vec<_>>()
            .join(" | ");
        lines.push(format!("{arm_indent}when {choices} =>"));
        lines.extend(render_nested_block(&arm.body, indent_level + 2));
    }

    if let Some(else_arm) = &statement.else_arm {
        lines.push(format!("{arm_indent}when others =>"));
        lines.extend(render_nested_block(else_arm, indent_level + 2));
    }

    lines.push(format!("{base_indent}end case;"));
    lines
}

fn render_subprogram_signature(spec: &AdaSubprogramSpec) -> String {
    let params = if spec.params.is_empty() {
        String::new()
    } else {
        format!(
            "({})",
            spec.params
                .iter()
                .map(render_param)
                .collect::<Vec<_>>()
                .join("; ")
        )
    };

    match &spec.return_type {
        Some(return_type) => format!(
            "function {}{} return {}",
            spec.name,
            params,
            return_type.as_string()
        ),
        None => format!("procedure {}{}", spec.name, params),
    }
}

fn render_subprogram_signature_and_aspects(
    spec: &AdaSubprogramSpec,
    indent_level: usize,
) -> Vec<String> {
    let base_indent = indent(indent_level);
    let mut lines = vec![format!(
        "{base_indent}{}",
        render_subprogram_signature(spec)
    )];
    let mut aspects = Vec::new();

    if !spec.preconditions.is_empty() {
        aspects.push(format!(
            "Pre => {}",
            render_contract_exprs(&spec.preconditions)
        ));
    }
    if !spec.postconditions.is_empty() {
        aspects.push(format!(
            "Post => {}",
            render_contract_exprs(&spec.postconditions)
        ));
    }

    if !aspects.is_empty() {
        let aspect_indent = indent(indent_level + 1);
        let continuation_indent = format!("{aspect_indent}     ");
        for (index, aspect) in aspects.iter().enumerate() {
            let is_last = index + 1 == aspects.len();
            let prefix = if index == 0 {
                format!("{aspect_indent}with ")
            } else {
                continuation_indent.clone()
            };
            let suffix = if is_last { "" } else { "," };
            lines.push(format!("{prefix}{aspect}{suffix}"));
        }
    }

    lines
}

fn render_subprogram_spec_lines(spec: &AdaSubprogramSpec, indent_level: usize) -> Vec<String> {
    let mut lines = render_subprogram_signature_and_aspects(spec, indent_level);
    let last = lines.pop().expect("spec lines should not be empty");
    lines.push(format!("{last};"));
    lines
}

fn render_param(param: &AdaParam) -> String {
    match param.mode {
        ParamMode::In => format!("{} : {}", param.name, param.ty.as_string()),
        ParamMode::Out => format!("{} : out {}", param.name, param.ty.as_string()),
        ParamMode::InOut => format!("{} : in out {}", param.name, param.ty.as_string()),
    }
}

fn render_expr(expr: &AdaExpr, parent_precedence: u8) -> String {
    let precedence = expr_precedence(expr);
    let rendered = match expr {
        AdaExpr::Bool(value) => {
            if *value {
                "True".to_string()
            } else {
                "False".to_string()
            }
        }
        AdaExpr::Integer(value) => value.clone(),
        AdaExpr::String(value) => format!("\"{}\"", value.replace('"', "\"\"")),
        AdaExpr::Name(name) => name.as_string(),
        AdaExpr::Result(name) => format!("{name}'Result"),
        AdaExpr::Qualified { prefix, member } => {
            format!("{}.{}", render_expr(prefix, precedence), member)
        }
        AdaExpr::Attribute { prefix, attribute } => {
            let attribute = match attribute {
                AdaAttribute::Length => "Length",
                AdaAttribute::Range => "Range",
                AdaAttribute::Image => "Image",
            };
            format!("{}'{}", render_expr(prefix, precedence), attribute)
        }
        AdaExpr::Call { callee, args } => format!(
            "{}({})",
            render_expr(callee, precedence),
            args.iter()
                .map(|arg| render_expr(arg, 0))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        AdaExpr::Unary { op, expr } => match op {
            UnaryOp::Negate => format!("-{}", render_expr(expr, precedence)),
            UnaryOp::Not => format!("not {}", render_expr(expr, precedence)),
        },
        AdaExpr::Binary { lhs, op, rhs } => {
            let operator = match op {
                BinaryOp::Or => "or",
                BinaryOp::ShortCircuitOr => "or else",
                BinaryOp::And => "and",
                BinaryOp::ShortCircuitAnd => "and then",
                BinaryOp::Add => "+",
                BinaryOp::Subtract => "-",
                BinaryOp::Multiply => "*",
                BinaryOp::Divide => "/",
                BinaryOp::Equal => "=",
                BinaryOp::NotEqual => "/=",
                BinaryOp::Less => "<",
                BinaryOp::LessEqual => "<=",
                BinaryOp::Greater => ">",
                BinaryOp::GreaterEqual => ">=",
            };
            format!(
                "{} {} {}",
                render_expr(lhs, precedence),
                operator,
                render_expr(rhs, precedence + 1)
            )
        }
    };

    if precedence < parent_precedence {
        format!("({rendered})")
    } else {
        rendered
    }
}

fn render_contract_exprs(exprs: &[AdaExpr]) -> String {
    exprs
        .iter()
        .map(|expr| render_expr(expr, 0))
        .collect::<Vec<_>>()
        .join(" and then ")
}

fn expr_precedence(expr: &AdaExpr) -> u8 {
    match expr {
        AdaExpr::Binary { op, .. } => match op {
            BinaryOp::Or | BinaryOp::ShortCircuitOr => 1,
            BinaryOp::And | BinaryOp::ShortCircuitAnd => 2,
            BinaryOp::Equal
            | BinaryOp::NotEqual
            | BinaryOp::Less
            | BinaryOp::LessEqual
            | BinaryOp::Greater
            | BinaryOp::GreaterEqual => 3,
            BinaryOp::Add | BinaryOp::Subtract => 4,
            BinaryOp::Multiply | BinaryOp::Divide => 5,
        },
        AdaExpr::Unary { .. } => 6,
        AdaExpr::Call { .. } | AdaExpr::Qualified { .. } | AdaExpr::Attribute { .. } => 7,
        AdaExpr::Bool(_)
        | AdaExpr::Integer(_)
        | AdaExpr::String(_)
        | AdaExpr::Name(_)
        | AdaExpr::Result(_) => 8,
    }
}

fn indent(level: usize) -> String {
    "   ".repeat(level)
}

fn render_nested_block(statements: &[AdaStatement], indent_level: usize) -> Vec<String> {
    if statements.is_empty() {
        return vec![format!("{}null;", indent(indent_level))];
    }

    let mut lines = Vec::new();
    for statement in statements {
        lines.extend(render_statement(statement, indent_level));
    }
    lines
}

fn compose_file(context: &[String], unit_lines: Vec<String>) -> String {
    compose_blocks(context, vec![unit_lines])
}

fn compose_blocks(context: &[String], blocks: Vec<Vec<String>>) -> String {
    let mut lines = context.to_vec();
    if !context.is_empty() && !blocks.is_empty() {
        lines.push(String::new());
    }

    let block_count = blocks.len();
    for (index, block) in blocks.into_iter().enumerate() {
        lines.extend(block);
        if index + 1 != block_count {
            lines.push(String::new());
        }
    }

    lines.join("\n")
}

fn unit_stem_from_name(name: &Name) -> String {
    name.segments
        .iter()
        .map(|segment| segment.to_ascii_lowercase())
        .collect::<Vec<_>>()
        .join("-")
}

fn unit_stem_from_identifier(name: &str) -> String {
    name.to_ascii_lowercase()
}
