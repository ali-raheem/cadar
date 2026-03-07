use std::collections::HashSet;

use crate::{
    ast::{
        ArrayType, BinaryOp, BlockItem, CaseStatement, DependTarget, DependsContract, EnumType,
        Expr, ForStatement, GlobalContract, GlobalMode, IfStatement, Item, LocalDecl,
        LoopVariantDirection, Name, Package, PackageItem, ParamMode, Program, RangeType,
        RecordType, Statement, StatementBlock, Subprogram, TypeDecl, UnaryOp,
    },
    diagnostic::{Diagnostic, IndexedDiagnostic},
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
    pub spec_units: Vec<AdaUnit<AdaSpecUnit>>,
    pub body_units: Vec<AdaUnit<AdaBodyUnit>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdaUnit<T> {
    pub context: Vec<AdaContextItem>,
    pub item: T,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AdaContextKind {
    With,
    Use,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AdaContextItem {
    pub kind: AdaContextKind,
    pub name: Name,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AdaSubprogramSpec {
    pub name: String,
    pub params: Vec<AdaParam>,
    pub return_type: Option<Name>,
    pub global: Option<AdaGlobalContract>,
    pub depends: Option<AdaDependsContract>,
    pub preconditions: Vec<AdaExpr>,
    pub postconditions: Vec<AdaExpr>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AdaParam {
    pub mode: ParamMode,
    pub ty: Name,
    pub name: String,
    pub default: Option<AdaExpr>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AdaCallArg {
    pub name: Option<String>,
    pub value: AdaExpr,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AdaGlobalContract {
    pub items: Vec<AdaGlobalItem>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AdaGlobalItem {
    pub mode: AdaGlobalMode,
    pub names: Vec<Name>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AdaGlobalMode {
    Null,
    Input,
    Output,
    InOut,
    ProofIn,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AdaDependsContract {
    pub items: Vec<AdaDependItem>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AdaDependItem {
    pub target: AdaDependTarget,
    pub sources: Vec<Name>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum AdaDependTarget {
    Null,
    Result,
    Name(Name),
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
    Object(AdaObjectDecl),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AdaPackageBodyItem {
    Subprogram(AdaSubprogramBody),
    Type(AdaTypeDecl),
    Object(AdaObjectDecl),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AdaTypeDecl {
    Record(AdaRecordType),
    Enum(AdaEnumType),
    Range(AdaRangeType),
    Array(AdaArrayType),
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
pub struct AdaArrayType {
    pub name: String,
    pub start: AdaExpr,
    pub end: AdaExpr,
    pub element_type: Name,
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
    Block {
        declarations: Vec<AdaObjectDecl>,
        statements: Vec<AdaStatement>,
    },
    Assert(AdaExpr),
    Return(AdaExpr),
    Assign {
        target: AdaExpr,
        value: AdaExpr,
    },
    Call(AdaExpr),
    If(AdaIfStatement),
    Case(AdaCaseStatement),
    While {
        condition: AdaExpr,
        invariants: Vec<AdaExpr>,
        variants: Vec<AdaLoopVariant>,
        body: Vec<AdaStatement>,
    },
    For {
        iterator: String,
        start: AdaExpr,
        end: AdaExpr,
        invariants: Vec<AdaExpr>,
        variants: Vec<AdaLoopVariant>,
        body: Vec<AdaStatement>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdaLoopVariant {
    pub direction: AdaLoopVariantDirection,
    pub expr: AdaExpr,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdaLoopVariantDirection {
    Increases,
    Decreases,
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
    Float(String),
    Character(char),
    String(String),
    Name(Name),
    Result(String),
    Qualified {
        prefix: Box<AdaExpr>,
        member: String,
    },
    Index {
        base: Box<AdaExpr>,
        index: Box<AdaExpr>,
    },
    Attribute {
        prefix: Box<AdaExpr>,
        attribute: AdaAttribute,
    },
    Call {
        callee: Box<AdaExpr>,
        args: Vec<AdaCallArg>,
    },
    NamedAggregate(Vec<(String, AdaExpr)>),
    Aggregate(Vec<AdaExpr>),
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

pub fn lower_all(programs: Vec<Program>) -> Result<AdaProgram, IndexedDiagnostic> {
    let mut spec_units = Vec::new();
    let mut body_units = Vec::new();
    let mut seen_specs = HashSet::new();
    let mut seen_package_specs = HashSet::new();
    let explicit_package_specs: HashSet<Name> = programs
        .iter()
        .flat_map(|program| program.items.iter())
        .filter_map(|item| match item {
            Item::Package(package) if !package.is_body => Some(package.name.clone()),
            _ => None,
        })
        .collect();

    let mut aggregate_contexts = Vec::new();

    for (source_index, program) in programs.into_iter().enumerate() {
        let unit_context = collect_program_context(&program);
        aggregate_contexts.push(unit_context.clone());

        for item in program.items {
            match item {
                Item::Import(_) | Item::Use(_) => {}
                Item::Subprogram(subprogram) => {
                    let spec = lower_spec(&subprogram)
                        .map_err(|diagnostic| IndexedDiagnostic::new(source_index, diagnostic))?;
                    if seen_specs.insert(subprogram_identity(&spec)) {
                        spec_units.push(AdaUnit {
                            context: unit_context.clone(),
                            item: AdaSpecUnit::Subprogram(spec.clone()),
                        });
                    }
                    if let Some(body) = subprogram.body {
                        let body = lower_body(spec, body.items).map_err(|diagnostic| {
                            IndexedDiagnostic::new(source_index, diagnostic)
                        })?;
                        body_units.push(AdaUnit {
                            context: unit_context.clone(),
                            item: AdaBodyUnit::Subprogram(body),
                        });
                    }
                }
                Item::Type(type_decl) => {
                    spec_units.push(AdaUnit {
                        context: unit_context.clone(),
                        item: AdaSpecUnit::Type(lower_type_decl(type_decl)),
                    });
                }
                Item::Package(package) => match lower_package(package)
                    .map_err(|diagnostic| IndexedDiagnostic::new(source_index, diagnostic))?
                {
                    LoweredPackage::Spec(package_spec) => {
                        seen_package_specs.insert(package_spec.name.clone());
                        spec_units.push(AdaUnit {
                            context: unit_context.clone(),
                            item: AdaSpecUnit::Package(package_spec),
                        });
                    }
                    LoweredPackage::Body { derived_spec, body } => {
                        if !explicit_package_specs.contains(&derived_spec.name)
                            && seen_package_specs.insert(derived_spec.name.clone())
                        {
                            spec_units.push(AdaUnit {
                                context: unit_context.clone(),
                                item: AdaSpecUnit::Package(derived_spec),
                            });
                        }
                        body_units.push(AdaUnit {
                            context: unit_context.clone(),
                            item: AdaBodyUnit::Package(body),
                        });
                    }
                },
            }
        }
    }

    Ok(AdaProgram {
        context: merge_contexts(aggregate_contexts.iter().flat_map(|context| context.iter())),
        spec_units,
        body_units,
    })
}

fn collect_program_context(program: &Program) -> Vec<AdaContextItem> {
    program
        .items
        .iter()
        .filter_map(|item| match item {
            Item::Import(name) => Some(AdaContextItem {
                kind: AdaContextKind::With,
                name: name.clone(),
            }),
            Item::Use(name) => Some(AdaContextItem {
                kind: AdaContextKind::Use,
                name: name.clone(),
            }),
            Item::Subprogram(_) | Item::Type(_) | Item::Package(_) => None,
        })
        .collect()
}

fn merge_contexts<'a>(
    contexts: impl IntoIterator<Item = &'a AdaContextItem>,
) -> Vec<AdaContextItem> {
    let mut merged = Vec::new();
    let mut seen = HashSet::new();

    for item in contexts {
        if seen.insert(item.clone()) {
            merged.push(item.clone());
        }
    }

    merged
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
                PackageItem::Object(decl) => {
                    body_items.push(AdaPackageBodyItem::Object(lower_decl(decl)?));
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
                PackageItem::Object(decl) => {
                    items.push(AdaPackageSpecItem::Object(lower_decl(decl)?));
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
        TypeDecl::Array(ArrayType {
            name,
            start,
            end,
            element_type,
            ..
        }) => AdaTypeDecl::Array(AdaArrayType {
            name,
            start: lower_expr(start),
            end: lower_expr(end),
            element_type,
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
                default: param.default.clone().map(lower_expr),
            })
            .collect(),
        return_type: subprogram.return_type.clone(),
        global: subprogram.global.clone().map(lower_global_contract),
        depends: subprogram
            .depends
            .clone()
            .map(|depends| lower_depends_contract(depends, subprogram.return_type.is_some())),
        preconditions,
        postconditions,
    })
}

fn lower_global_contract(contract: GlobalContract) -> AdaGlobalContract {
    AdaGlobalContract {
        items: contract
            .items
            .into_iter()
            .map(|item| AdaGlobalItem {
                mode: match item.mode {
                    GlobalMode::Null => AdaGlobalMode::Null,
                    GlobalMode::Input => AdaGlobalMode::Input,
                    GlobalMode::Output => AdaGlobalMode::Output,
                    GlobalMode::InOut => AdaGlobalMode::InOut,
                    GlobalMode::ProofIn => AdaGlobalMode::ProofIn,
                },
                names: item.names,
            })
            .collect(),
    }
}

fn lower_depends_contract(contract: DependsContract, is_function: bool) -> AdaDependsContract {
    AdaDependsContract {
        items: contract
            .items
            .into_iter()
            .map(|item| AdaDependItem {
                target: match item.target {
                    DependTarget::Null => AdaDependTarget::Null,
                    DependTarget::Result if is_function => AdaDependTarget::Result,
                    DependTarget::Result => AdaDependTarget::Name(Name {
                        segments: vec!["result".to_string()],
                    }),
                    DependTarget::Name(name) => AdaDependTarget::Name(name),
                },
                sources: item.sources,
            })
            .collect(),
    }
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
        Statement::Assert { expr, .. } => Ok(AdaStatement::Assert(lower_expr(expr))),
        Statement::Return { expr, .. } => Ok(AdaStatement::Return(lower_expr(expr))),
        Statement::Assign { target, value, .. } => Ok(AdaStatement::Assign {
            target: lower_assignment_target(target)?,
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
            condition,
            invariants,
            variants,
            body,
            ..
        } => Ok(AdaStatement::While {
            condition: lower_expr(condition),
            invariants: invariants.into_iter().map(lower_expr).collect(),
            variants: variants.into_iter().map(lower_loop_variant).collect(),
            body: lower_statement_block(body)?,
        }),
        Statement::For(ForStatement {
            iterator_type: _,
            iterator,
            start,
            end,
            invariants,
            variants,
            body,
            ..
        }) => Ok(AdaStatement::For {
            iterator,
            start: lower_expr(start),
            end: lower_expr(end),
            invariants: invariants.into_iter().map(lower_expr).collect(),
            variants: variants.into_iter().map(lower_loop_variant).collect(),
            body: lower_statement_block(body)?,
        }),
    }
}

fn lower_loop_variant(variant: crate::ast::LoopVariant) -> AdaLoopVariant {
    AdaLoopVariant {
        direction: match variant.direction {
            LoopVariantDirection::Increases => AdaLoopVariantDirection::Increases,
            LoopVariantDirection::Decreases => AdaLoopVariantDirection::Decreases,
        },
        expr: lower_expr(variant.expr),
    }
}

fn lower_assignment_target(target: Expr) -> Result<AdaExpr, Diagnostic> {
    match target {
        Expr::Name { .. } | Expr::Member { .. } | Expr::Index { .. } => Ok(lower_expr(target)),
        _ => Err(Diagnostic::new(
            "invalid assignment target",
            target.position(),
        )),
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
    let mut declarations = Vec::new();
    let mut statements = Vec::new();
    let mut saw_statement = false;

    for item in block.items {
        match item {
            BlockItem::LocalDecl(decl) => {
                if saw_statement {
                    return Err(Diagnostic::new(
                        "local declarations must appear before statements in a nested block",
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

    if declarations.is_empty() {
        Ok(statements)
    } else {
        Ok(vec![AdaStatement::Block {
            declarations,
            statements,
        }])
    }
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
        Expr::Float { value, .. } => Ok(AdaExpr::Float(value)),
        Expr::Character { value, .. } => Ok(AdaExpr::Character(value)),
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
        Expr::Index { base, index, .. } => Ok(AdaExpr::Index {
            base: Box::new(lower_expr_internal(*base, contract)?),
            index: Box::new(lower_expr_internal(*index, contract)?),
        }),
        Expr::Call { callee, args, .. } => Ok(AdaExpr::Call {
            callee: Box::new(lower_expr_internal(*callee, contract)?),
            args: args
                .into_iter()
                .map(|arg| {
                    Ok(AdaCallArg {
                        name: arg.name,
                        value: lower_expr_internal(arg.value, contract)?,
                    })
                })
                .collect::<Result<Vec<_>, Diagnostic>>()?,
        }),
        Expr::RecordLiteral { fields, .. } => Ok(AdaExpr::NamedAggregate(
            fields
                .into_iter()
                .map(|field| Ok((field.name, lower_expr_internal(field.value, contract)?)))
                .collect::<Result<Vec<_>, Diagnostic>>()?,
        )),
        Expr::ArrayLiteral { elements, .. } => Ok(AdaExpr::Aggregate(
            elements
                .into_iter()
                .map(|element| lower_expr_internal(element, contract))
                .collect::<Result<Vec<_>, Diagnostic>>()?,
        )),
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
    let mut aggregate_spec_types = Vec::new();
    let top_level_subprogram_units = collect_top_level_subprogram_units(program);
    let support_package = program
        .spec_units
        .iter()
        .any(|unit| matches!(unit.item, AdaSpecUnit::Type(_)))
        .then(|| split_support_package_name(fallback_stem));
    let support_context = merge_contexts(
        program
            .spec_units
            .iter()
            .filter_map(|unit| {
                matches!(unit.item, AdaSpecUnit::Type(_)).then_some(unit.context.iter())
            })
            .flatten(),
    );

    for unit in &program.spec_units {
        match &unit.item {
            AdaSpecUnit::Subprogram(spec) => {
                let self_name = Name {
                    segments: vec![spec.name.clone()],
                };
                let extra_withs =
                    collect_subprogram_spec_dependencies(spec, &top_level_subprogram_units);
                let context = render_context_for_unit(
                    &unit.context,
                    Some(&self_name),
                    support_package.as_ref(),
                    &extra_withs,
                );
                files.push(GeneratedFile {
                    filename: format!("{}.ads", unit_stem_from_identifier(&spec.name)),
                    contents: compose_file(&context, render_subprogram_spec_lines(spec, 0)),
                });
            }
            AdaSpecUnit::Type(type_decl) => aggregate_spec_types.push(type_decl),
            AdaSpecUnit::Package(package) => {
                let extra_withs =
                    collect_package_spec_dependencies(package, &top_level_subprogram_units);
                let context = render_context_for_unit(
                    &unit.context,
                    Some(&package.name),
                    support_package.as_ref(),
                    &extra_withs,
                );
                files.push(GeneratedFile {
                    filename: format!("{}.ads", unit_stem_from_name(&package.name)),
                    contents: compose_file(&context, render_package_spec(package)),
                });
            }
        }
    }

    if let Some(support_package) = &support_package {
        let context = render_context_for_unit(&support_context, Some(support_package), None, &[]);
        files.push(GeneratedFile {
            filename: format!("{}.ads", unit_stem_from_name(support_package)),
            contents: compose_file(
                &context,
                render_split_support_package(support_package, &aggregate_spec_types),
            ),
        });
    }

    for unit in &program.body_units {
        match &unit.item {
            AdaBodyUnit::Subprogram(body) => {
                let self_name = Name {
                    segments: vec![body.spec.name.clone()],
                };
                let extra_withs =
                    collect_subprogram_body_dependencies(body, &top_level_subprogram_units);
                let context = render_context_for_unit(
                    &unit.context,
                    Some(&self_name),
                    support_package.as_ref(),
                    &extra_withs,
                );
                files.push(GeneratedFile {
                    filename: format!("{}.adb", unit_stem_from_identifier(&body.spec.name)),
                    contents: compose_file(&context, render_subprogram_body(body, 0)),
                });
            }
            AdaBodyUnit::Package(package) => {
                let extra_withs =
                    collect_package_body_dependencies(package, &top_level_subprogram_units);
                let context = render_context_for_unit(
                    &unit.context,
                    Some(&package.name),
                    support_package.as_ref(),
                    &extra_withs,
                );
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
        match &unit.item {
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
        match &unit.item {
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

fn render_context_for_unit(
    context: &[AdaContextItem],
    self_name: Option<&Name>,
    support_package: Option<&Name>,
    extra_withs: &[Name],
) -> Vec<String> {
    let mut lines: Vec<_> = context
        .iter()
        .filter(|item| match self_name {
            Some(self_name) => item.name != *self_name,
            None => true,
        })
        .map(render_context_item)
        .collect();

    for dependency in extra_withs {
        if self_name == Some(dependency) {
            continue;
        }
        let has_with = context
            .iter()
            .any(|item| item.kind == AdaContextKind::With && item.name == *dependency);
        if !has_with {
            lines.push(render_context_item(&AdaContextItem {
                kind: AdaContextKind::With,
                name: dependency.clone(),
            }));
        }
    }

    if let Some(support_package) = support_package
        && self_name != Some(support_package)
    {
        let has_with = context
            .iter()
            .any(|item| item.kind == AdaContextKind::With && item.name == *support_package);
        let has_use = context
            .iter()
            .any(|item| item.kind == AdaContextKind::Use && item.name == *support_package);

        if !has_with {
            lines.push(render_context_item(&AdaContextItem {
                kind: AdaContextKind::With,
                name: support_package.clone(),
            }));
        }
        if !has_use {
            lines.push(render_context_item(&AdaContextItem {
                kind: AdaContextKind::Use,
                name: support_package.clone(),
            }));
        }
    }

    lines
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
            AdaPackageSpecItem::Object(decl) => {
                lines.push(format!("{}{}", indent(1), render_object_decl(decl)));
            }
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
            AdaPackageBodyItem::Object(decl) => {
                lines.push(format!("{}{}", indent(1), render_object_decl(decl)));
            }
        }
        if index + 1 != package.items.len() {
            lines.push(String::new());
        }
    }

    lines.push(format!("end {};", package.name.as_string()));
    lines
}

fn render_split_support_package(name: &Name, types: &[&AdaTypeDecl]) -> Vec<String> {
    let mut lines = vec![format!("package {} is", name.as_string())];

    for (index, type_decl) in types.iter().enumerate() {
        lines.extend(render_type_decl(type_decl, 1));
        if index + 1 != types.len() {
            lines.push(String::new());
        }
    }

    lines.push(format!("end {};", name.as_string()));
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
        AdaTypeDecl::Array(array_type) => vec![format!(
            "{base_indent}type {} is array ({} .. {}) of {};",
            array_type.name,
            render_expr(&array_type.start, 0),
            render_expr(&array_type.end, 0),
            array_type.element_type.as_string()
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
    let base_indent = indent(indent_level);
    match statement {
        AdaStatement::Null => vec![format!("{base_indent}null;")],
        AdaStatement::Block {
            declarations,
            statements,
        } => {
            let declaration_indent = indent(indent_level + 1);
            let mut lines = vec![format!("{base_indent}declare")];
            for declaration in declarations {
                lines.push(format!(
                    "{declaration_indent}{}",
                    render_object_decl(declaration)
                ));
            }
            lines.push(format!("{base_indent}begin"));
            lines.extend(render_nested_block(statements, indent_level + 1));
            lines.push(format!("{base_indent}end;"));
            lines
        }
        AdaStatement::Assert(expr) => {
            vec![format!(
                "{base_indent}pragma Assert ({});",
                render_expr(expr, 0)
            )]
        }
        AdaStatement::Return(expr) => {
            vec![format!("{base_indent}return {};", render_expr(expr, 0))]
        }
        AdaStatement::Assign { target, value } => {
            vec![format!(
                "{base_indent}{} := {};",
                render_expr(target, 0),
                render_expr(value, 0)
            )]
        }
        AdaStatement::Call(expr) => vec![format!("{base_indent}{};", render_expr(expr, 0))],
        AdaStatement::If(if_statement) => render_if_statement(if_statement, indent_level),
        AdaStatement::Case(case_statement) => render_case_statement(case_statement, indent_level),
        AdaStatement::While {
            condition,
            invariants,
            variants,
            body,
        } => {
            let mut lines = vec![format!(
                "{base_indent}while {} loop",
                render_expr(condition, 0)
            )];
            lines.extend(render_loop_annotations(
                invariants,
                variants,
                indent_level + 1,
            ));
            lines.extend(render_nested_block(body, indent_level + 1));
            lines.push(format!("{base_indent}end loop;"));
            lines
        }
        AdaStatement::For {
            iterator,
            start,
            end,
            invariants,
            variants,
            body,
        } => {
            let mut lines = vec![format!(
                "{base_indent}for {iterator} in {} .. {} loop",
                render_expr(start, 0),
                render_expr(end, 0)
            )];
            lines.extend(render_loop_annotations(
                invariants,
                variants,
                indent_level + 1,
            ));
            lines.extend(render_nested_block(body, indent_level + 1));
            lines.push(format!("{base_indent}end loop;"));
            lines
        }
    }
}

fn render_loop_annotations(
    invariants: &[AdaExpr],
    variants: &[AdaLoopVariant],
    indent_level: usize,
) -> Vec<String> {
    let indent = indent(indent_level);
    let mut lines = invariants
        .iter()
        .map(|expr| format!("{indent}pragma Loop_Invariant ({});", render_expr(expr, 0)))
        .collect::<Vec<_>>();

    if !variants.is_empty() {
        let rendered = variants
            .iter()
            .map(|variant| {
                let direction = match variant.direction {
                    AdaLoopVariantDirection::Increases => "Increases",
                    AdaLoopVariantDirection::Decreases => "Decreases",
                };
                format!("{direction} => {}", render_expr(&variant.expr, 0))
            })
            .collect::<Vec<_>>()
            .join(", ");
        lines.push(format!("{indent}pragma Loop_Variant ({rendered});"));
    }

    lines
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

    if let Some(global) = &spec.global {
        aspects.push(format!("Global => {}", render_global_contract(global)));
    }
    if let Some(depends) = &spec.depends {
        aspects.push(format!(
            "Depends => {}",
            render_depends_contract(depends, &spec.name)
        ));
    }
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

fn render_global_contract(contract: &AdaGlobalContract) -> String {
    if matches!(
        contract.items.as_slice(),
        [AdaGlobalItem {
            mode: AdaGlobalMode::Null,
            names,
        }] if names.is_empty()
    ) {
        return "null".to_string();
    }

    format!(
        "({})",
        contract
            .items
            .iter()
            .map(|item| {
                let mode = match item.mode {
                    AdaGlobalMode::Null => "null",
                    AdaGlobalMode::Input => "Input",
                    AdaGlobalMode::Output => "Output",
                    AdaGlobalMode::InOut => "In_Out",
                    AdaGlobalMode::ProofIn => "Proof_In",
                };
                if item.mode == AdaGlobalMode::Null {
                    mode.to_string()
                } else {
                    format!("{mode} => {}", render_name_list(&item.names))
                }
            })
            .collect::<Vec<_>>()
            .join(", ")
    )
}

fn render_depends_contract(contract: &AdaDependsContract, subprogram_name: &str) -> String {
    format!(
        "({})",
        contract
            .items
            .iter()
            .map(|item| {
                let target = match &item.target {
                    AdaDependTarget::Null => "null".to_string(),
                    AdaDependTarget::Result => format!("{subprogram_name}'Result"),
                    AdaDependTarget::Name(name) => name.as_string(),
                };
                format!("{target} => {}", render_name_list(&item.sources))
            })
            .collect::<Vec<_>>()
            .join(", ")
    )
}

fn render_name_list(names: &[Name]) -> String {
    match names {
        [name] => name.as_string(),
        _ => format!(
            "({})",
            names
                .iter()
                .map(Name::as_string)
                .collect::<Vec<_>>()
                .join(", ")
        ),
    }
}

fn render_subprogram_spec_lines(spec: &AdaSubprogramSpec, indent_level: usize) -> Vec<String> {
    let mut lines = render_subprogram_signature_and_aspects(spec, indent_level);
    let last = lines.pop().expect("spec lines should not be empty");
    lines.push(format!("{last};"));
    lines
}

fn render_param(param: &AdaParam) -> String {
    let rendered = match param.mode {
        ParamMode::In => format!("{} : {}", param.name, param.ty.as_string()),
        ParamMode::Out => format!("{} : out {}", param.name, param.ty.as_string()),
        ParamMode::InOut => format!("{} : in out {}", param.name, param.ty.as_string()),
    };

    match &param.default {
        Some(default) => format!("{rendered} := {}", render_expr(default, 0)),
        None => rendered,
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
        AdaExpr::Float(value) => value.clone(),
        AdaExpr::Character(value) => format!("'{}'", value),
        AdaExpr::String(value) => format!("\"{}\"", value.replace('"', "\"\"")),
        AdaExpr::Name(name) => name.as_string(),
        AdaExpr::Result(name) => format!("{name}'Result"),
        AdaExpr::Qualified { prefix, member } => {
            format!("{}.{}", render_expr(prefix, precedence), member)
        }
        AdaExpr::Index { base, index } => {
            format!(
                "{}({})",
                render_expr(base, precedence),
                render_expr(index, 0)
            )
        }
        AdaExpr::Attribute { prefix, attribute } => {
            let attribute = match attribute {
                AdaAttribute::Length => "Length",
                AdaAttribute::Range => "Range",
                AdaAttribute::Image => "Image",
            };
            format!("{}'{}", render_expr(prefix, precedence), attribute)
        }
        AdaExpr::Call { callee, args } => {
            if args.is_empty() {
                render_expr(callee, precedence)
            } else {
                format!(
                    "{}({})",
                    render_expr(callee, precedence),
                    args.iter()
                        .map(|arg| match &arg.name {
                            Some(name) => format!("{name} => {}", render_expr(&arg.value, 0)),
                            None => render_expr(&arg.value, 0),
                        })
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            }
        }
        AdaExpr::NamedAggregate(fields) => format!(
            "({})",
            fields
                .iter()
                .map(|(name, value)| format!("{name} => {}", render_expr(value, 0)))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        AdaExpr::Aggregate(elements) => format!(
            "({})",
            elements
                .iter()
                .map(|element| render_expr(element, 0))
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
        AdaExpr::Call { .. }
        | AdaExpr::Qualified { .. }
        | AdaExpr::Index { .. }
        | AdaExpr::Attribute { .. } => 7,
        AdaExpr::Bool(_)
        | AdaExpr::Integer(_)
        | AdaExpr::Float(_)
        | AdaExpr::Character(_)
        | AdaExpr::String(_)
        | AdaExpr::NamedAggregate(_)
        | AdaExpr::Aggregate(_)
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

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct SubprogramIdentity {
    name: String,
    params: Vec<(ParamMode, String)>,
    return_type: Option<String>,
}

fn subprogram_identity(spec: &AdaSubprogramSpec) -> SubprogramIdentity {
    SubprogramIdentity {
        name: spec.name.clone(),
        params: spec
            .params
            .iter()
            .map(|param| (param.mode, param.ty.as_string()))
            .collect(),
        return_type: spec.return_type.as_ref().map(Name::as_string),
    }
}

fn collect_top_level_subprogram_units(program: &AdaProgram) -> HashSet<Name> {
    program
        .spec_units
        .iter()
        .filter_map(|unit| match &unit.item {
            AdaSpecUnit::Subprogram(spec) => Some(Name {
                segments: vec![spec.name.clone()],
            }),
            AdaSpecUnit::Type(_) | AdaSpecUnit::Package(_) => None,
        })
        .collect()
}

fn collect_subprogram_spec_dependencies(
    spec: &AdaSubprogramSpec,
    top_level_subprogram_units: &HashSet<Name>,
) -> Vec<Name> {
    let mut dependencies = HashSet::new();
    collect_subprogram_spec_dependency_set(spec, top_level_subprogram_units, &mut dependencies);
    sort_names(dependencies)
}

fn collect_subprogram_spec_dependency_set(
    spec: &AdaSubprogramSpec,
    top_level_subprogram_units: &HashSet<Name>,
    dependencies: &mut HashSet<Name>,
) {
    for param in &spec.params {
        if let Some(default) = &param.default {
            collect_expr_dependencies(default, top_level_subprogram_units, dependencies);
        }
    }
    for expr in &spec.preconditions {
        collect_expr_dependencies(expr, top_level_subprogram_units, dependencies);
    }
    for expr in &spec.postconditions {
        collect_expr_dependencies(expr, top_level_subprogram_units, dependencies);
    }
}

fn collect_subprogram_body_dependencies(
    body: &AdaSubprogramBody,
    top_level_subprogram_units: &HashSet<Name>,
) -> Vec<Name> {
    let mut dependencies = HashSet::new();
    collect_subprogram_spec_dependency_set(
        &body.spec,
        top_level_subprogram_units,
        &mut dependencies,
    );
    for declaration in &body.declarations {
        if let Some(initializer) = &declaration.initializer {
            collect_expr_dependencies(initializer, top_level_subprogram_units, &mut dependencies);
        }
    }
    for statement in &body.statements {
        collect_statement_dependencies(statement, top_level_subprogram_units, &mut dependencies);
    }
    sort_names(dependencies)
}

fn collect_package_spec_dependencies(
    package: &AdaPackageSpec,
    top_level_subprogram_units: &HashSet<Name>,
) -> Vec<Name> {
    let mut dependencies = HashSet::new();
    for item in &package.items {
        match item {
            AdaPackageSpecItem::Subprogram(spec) => {
                collect_subprogram_spec_dependency_set(
                    spec,
                    top_level_subprogram_units,
                    &mut dependencies,
                );
            }
            AdaPackageSpecItem::Object(decl) => {
                if let Some(initializer) = &decl.initializer {
                    collect_expr_dependencies(
                        initializer,
                        top_level_subprogram_units,
                        &mut dependencies,
                    );
                }
            }
            AdaPackageSpecItem::Type(type_decl) => {
                collect_type_decl_dependencies(
                    type_decl,
                    top_level_subprogram_units,
                    &mut dependencies,
                );
            }
        }
    }
    sort_names(dependencies)
}

fn collect_package_body_dependencies(
    package: &AdaPackageBody,
    top_level_subprogram_units: &HashSet<Name>,
) -> Vec<Name> {
    let mut dependencies = HashSet::new();
    for item in &package.items {
        match item {
            AdaPackageBodyItem::Subprogram(body) => {
                for dependency in
                    collect_subprogram_body_dependencies(body, top_level_subprogram_units)
                {
                    dependencies.insert(dependency);
                }
            }
            AdaPackageBodyItem::Object(decl) => {
                if let Some(initializer) = &decl.initializer {
                    collect_expr_dependencies(
                        initializer,
                        top_level_subprogram_units,
                        &mut dependencies,
                    );
                }
            }
            AdaPackageBodyItem::Type(type_decl) => {
                collect_type_decl_dependencies(
                    type_decl,
                    top_level_subprogram_units,
                    &mut dependencies,
                );
            }
        }
    }
    sort_names(dependencies)
}

fn collect_type_decl_dependencies(
    type_decl: &AdaTypeDecl,
    top_level_subprogram_units: &HashSet<Name>,
    dependencies: &mut HashSet<Name>,
) {
    match type_decl {
        AdaTypeDecl::Range(range_type) => {
            collect_expr_dependencies(&range_type.start, top_level_subprogram_units, dependencies);
            collect_expr_dependencies(&range_type.end, top_level_subprogram_units, dependencies);
        }
        AdaTypeDecl::Array(array_type) => {
            collect_expr_dependencies(&array_type.start, top_level_subprogram_units, dependencies);
            collect_expr_dependencies(&array_type.end, top_level_subprogram_units, dependencies);
        }
        AdaTypeDecl::Record(_) | AdaTypeDecl::Enum(_) => {}
    }
}

fn collect_statement_dependencies(
    statement: &AdaStatement,
    top_level_subprogram_units: &HashSet<Name>,
    dependencies: &mut HashSet<Name>,
) {
    match statement {
        AdaStatement::Null => {}
        AdaStatement::Block {
            declarations,
            statements,
        } => {
            for declaration in declarations {
                if let Some(initializer) = &declaration.initializer {
                    collect_expr_dependencies(
                        initializer,
                        top_level_subprogram_units,
                        dependencies,
                    );
                }
            }
            for nested in statements {
                collect_statement_dependencies(nested, top_level_subprogram_units, dependencies);
            }
        }
        AdaStatement::Assert(expr) | AdaStatement::Return(expr) | AdaStatement::Call(expr) => {
            collect_expr_dependencies(expr, top_level_subprogram_units, dependencies);
        }
        AdaStatement::Assign { target, value } => {
            collect_expr_dependencies(target, top_level_subprogram_units, dependencies);
            collect_expr_dependencies(value, top_level_subprogram_units, dependencies);
        }
        AdaStatement::If(statement) => {
            collect_expr_dependencies(
                &statement.condition,
                top_level_subprogram_units,
                dependencies,
            );
            for nested in &statement.then_branch {
                collect_statement_dependencies(nested, top_level_subprogram_units, dependencies);
            }
            for branch in &statement.else_if_branches {
                collect_expr_dependencies(
                    &branch.condition,
                    top_level_subprogram_units,
                    dependencies,
                );
                for nested in &branch.body {
                    collect_statement_dependencies(
                        nested,
                        top_level_subprogram_units,
                        dependencies,
                    );
                }
            }
            if let Some(else_branch) = &statement.else_branch {
                for nested in else_branch {
                    collect_statement_dependencies(
                        nested,
                        top_level_subprogram_units,
                        dependencies,
                    );
                }
            }
        }
        AdaStatement::Case(statement) => {
            collect_expr_dependencies(&statement.expr, top_level_subprogram_units, dependencies);
            for arm in &statement.arms {
                for choice in &arm.choices {
                    collect_expr_dependencies(choice, top_level_subprogram_units, dependencies);
                }
                for nested in &arm.body {
                    collect_statement_dependencies(
                        nested,
                        top_level_subprogram_units,
                        dependencies,
                    );
                }
            }
            if let Some(else_arm) = &statement.else_arm {
                for nested in else_arm {
                    collect_statement_dependencies(
                        nested,
                        top_level_subprogram_units,
                        dependencies,
                    );
                }
            }
        }
        AdaStatement::While {
            condition,
            invariants,
            variants,
            body,
        } => {
            collect_expr_dependencies(condition, top_level_subprogram_units, dependencies);
            for invariant in invariants {
                collect_expr_dependencies(invariant, top_level_subprogram_units, dependencies);
            }
            for variant in variants {
                collect_expr_dependencies(&variant.expr, top_level_subprogram_units, dependencies);
            }
            for nested in body {
                collect_statement_dependencies(nested, top_level_subprogram_units, dependencies);
            }
        }
        AdaStatement::For {
            start,
            end,
            invariants,
            variants,
            body,
            ..
        } => {
            collect_expr_dependencies(start, top_level_subprogram_units, dependencies);
            collect_expr_dependencies(end, top_level_subprogram_units, dependencies);
            for invariant in invariants {
                collect_expr_dependencies(invariant, top_level_subprogram_units, dependencies);
            }
            for variant in variants {
                collect_expr_dependencies(&variant.expr, top_level_subprogram_units, dependencies);
            }
            for nested in body {
                collect_statement_dependencies(nested, top_level_subprogram_units, dependencies);
            }
        }
    }
}

fn collect_expr_dependencies(
    expr: &AdaExpr,
    top_level_subprogram_units: &HashSet<Name>,
    dependencies: &mut HashSet<Name>,
) {
    match expr {
        AdaExpr::Call { callee, args } => {
            if let AdaExpr::Name(name) = callee.as_ref()
                && top_level_subprogram_units.contains(name)
            {
                dependencies.insert(name.clone());
            }
            collect_expr_dependencies(callee, top_level_subprogram_units, dependencies);
            for arg in args {
                collect_expr_dependencies(&arg.value, top_level_subprogram_units, dependencies);
            }
        }
        AdaExpr::Qualified { prefix, .. } | AdaExpr::Attribute { prefix, .. } => {
            collect_expr_dependencies(prefix, top_level_subprogram_units, dependencies);
        }
        AdaExpr::Index { base, index } => {
            collect_expr_dependencies(base, top_level_subprogram_units, dependencies);
            collect_expr_dependencies(index, top_level_subprogram_units, dependencies);
        }
        AdaExpr::NamedAggregate(fields) => {
            for (_, value) in fields {
                collect_expr_dependencies(value, top_level_subprogram_units, dependencies);
            }
        }
        AdaExpr::Aggregate(elements) => {
            for element in elements {
                collect_expr_dependencies(element, top_level_subprogram_units, dependencies);
            }
        }
        AdaExpr::Unary { expr, .. } => {
            collect_expr_dependencies(expr, top_level_subprogram_units, dependencies);
        }
        AdaExpr::Binary { lhs, rhs, .. } => {
            collect_expr_dependencies(lhs, top_level_subprogram_units, dependencies);
            collect_expr_dependencies(rhs, top_level_subprogram_units, dependencies);
        }
        AdaExpr::Bool(_)
        | AdaExpr::Integer(_)
        | AdaExpr::Float(_)
        | AdaExpr::Character(_)
        | AdaExpr::String(_)
        | AdaExpr::Name(_)
        | AdaExpr::Result(_) => {}
    }
}

fn sort_names(names: HashSet<Name>) -> Vec<Name> {
    let mut names = names.into_iter().collect::<Vec<_>>();
    names.sort_by_key(Name::as_string);
    names
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

fn split_support_package_name(fallback_stem: &str) -> Name {
    let mut sanitized = String::from("Cadar_");
    let mut last_was_underscore = false;

    for ch in fallback_stem.chars() {
        if ch.is_ascii_alphanumeric() {
            sanitized.push(ch);
            last_was_underscore = false;
        } else if !last_was_underscore {
            sanitized.push('_');
            last_was_underscore = true;
        }
    }

    if sanitized.ends_with('_') {
        sanitized.pop();
    }

    sanitized.push_str("_Support");

    Name {
        segments: vec![sanitized],
    }
}
