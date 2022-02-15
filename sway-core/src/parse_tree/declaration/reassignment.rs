use crate::{
    build_config::BuildConfig,
    error::{err, ok, CompileError, CompileResult},
    parse_array_index,
    parse_tree::{ident, Expression},
    parser::Rule,
};

use sway_types::{span::Span, Ident};

use pest::iterators::Pair;

/// Represents the left hand side of a reassignment, which could either be a regular variable
/// expression, denoted by [ReassignmentTarget::VariableExpression], or, a storage field, denoted
/// by [ReassignmentTarget::StorageField].
#[derive(Debug, Clone)]
pub enum ReassignmentTarget {
    VariableExpression(Expression),
    StorageField(Expression),
}

#[derive(Debug, Clone)]
pub struct Reassignment {
    // the thing being reassigned
    pub lhs: ReassignmentTarget,
    // the expression that is being assigned to the lhs
    pub rhs: Expression,
    pub(crate) span: Span,
}

impl Reassignment {
    pub(crate) fn parse_from_pair(
        pair: Pair<Rule>,
        config: Option<&BuildConfig>,
    ) -> CompileResult<Reassignment> {
        let path = config.map(|c| c.path());
        let span = Span {
            span: pair.as_span(),
            path: path.clone(),
        };
        let mut warnings = vec![];
        let mut errors = vec![];
        let mut iter = pair.into_inner();
        let variable_or_struct_reassignment = iter.next().expect("guaranteed by grammar");
        match variable_or_struct_reassignment.as_rule() {
            Rule::variable_reassignment => {
                let (lhs, rhs) = check!(
                    parse_simple_variable_reassignment(
                        variable_or_struct_reassignment,
                        config,
                        path,
                    ),
                    return err(warnings, errors),
                    warnings,
                    errors
                );

                ok(
                    Reassignment {
                        lhs: ReassignmentTarget::VariableExpression(lhs),
                        rhs,
                        span,
                    },
                    warnings,
                    errors,
                )
            }
            Rule::struct_field_reassignment => {
                let (lhs, rhs) = check!(
                    parse_subfield_reassignment(variable_or_struct_reassignment, config, path),
                    return err(warnings, errors),
                    warnings,
                    errors
                );

                ok(
                    Reassignment {
                        lhs: ReassignmentTarget::VariableExpression(lhs),
                        rhs,
                        span,
                    },
                    warnings,
                    errors,
                )
            }
            Rule::storage_reassignment => {
                // there is some ugly handling of the pest data structure here
                // because we are reusing code from struct field reassignments
                // this leads to some nested `Rule`s, so the actual storage
                // reassignment requires some `expect()`s to extract.
                let mut iter = variable_or_struct_reassignment.into_inner();
                let reassignment = iter.next().expect("guaranteed by grammar");
                let mut iter = reassignment.into_inner();

                let pair = iter.next().expect("guaranteed by grammar");

                match pair.as_rule() {
                    Rule::variable_reassignment => {
                        // then this is a single field reassignment with no inner struct fields
                        dbg!(&pair);
                        let (storage_field_to_access, value_to_assign) = check!(
                            dbg!(parse_simple_variable_reassignment(pair, config, path)),
                            return err(warnings, errors),
                            warnings,
                            errors
                        );
                        ok(
                            Reassignment {
                                lhs: ReassignmentTarget::StorageField(storage_field_to_access),
                                rhs: value_to_assign,
                                span,
                            },
                            warnings,
                            errors,
                        )
                    }
                    Rule::struct_field_reassignment => {
                        // then this is a reassignment to an inner field of a struct within storage
                        let (subfield_storage_path, body) = check!(
                            parse_subfield_reassignment(pair, config, path),
                            return err(warnings, errors),
                            warnings,
                            errors
                        );

                        ok(
                            Reassignment {
                                lhs: ReassignmentTarget::StorageField(subfield_storage_path),
                                rhs: body,
                                span,
                            },
                            warnings,
                            errors,
                        )
                    }
                    Rule::storage_reassignment => {
                        errors.push(todo!(
                            "Nested storage reassignment is invalid; parsing error"
                        ));
                        return err(warnings, errors);
                    }
                    a => unreachable!("guaranteed by grammar: {:?}", a),
                }
            }
            _ => unreachable!("guaranteed by grammar"),
        }
    }
}

fn parse_subfield_path_ensure_only_var(
    item: Pair<Rule>,
    config: Option<&BuildConfig>,
) -> CompileResult<Expression> {
    let warnings = vec![];
    let mut errors = vec![];
    let path = config.map(|c| c.path());
    let item = item.into_inner().next().expect("guarenteed by grammar");
    match item.as_rule() {
        Rule::call_item => parse_call_item_ensure_only_var(item, config),
        Rule::array_index => parse_array_index(item, config),
        a => {
            eprintln!(
                "Unimplemented subfield path: {:?} ({:?}) ({:?})",
                a,
                item.as_str(),
                item.as_rule()
            );
            errors.push(CompileError::UnimplementedRule(
                a,
                Span {
                    span: item.as_span(),
                    path: path.clone(),
                },
            ));
            // construct unit expression for error recovery
            let exp = Expression::Tuple {
                fields: vec![],
                span: Span {
                    span: item.as_span(),
                    path,
                },
            };
            ok(exp, warnings, errors)
        }
    }
}

/// Parses a `call_item` rule but ensures that it is only a variable expression, since generic
/// expressions on the LHS of a reassignment are invalid.
/// valid:
/// ```ignore
/// x.y.foo = 5;
/// ```
///
/// invalid:
/// ```ignore
/// (foo()).x = 5;
/// ```
fn parse_call_item_ensure_only_var(
    item: Pair<Rule>,
    config: Option<&BuildConfig>,
) -> CompileResult<Expression> {
    let path = config.map(|c| c.path());
    let mut warnings = vec![];
    let mut errors = vec![];
    assert_eq!(item.as_rule(), Rule::call_item);
    let item = item.into_inner().next().expect("guaranteed by grammar");
    let exp = match item.as_rule() {
        Rule::ident => Expression::VariableExpression {
            name: check!(
                ident::parse_from_pair(item.clone(), config),
                return err(warnings, errors),
                warnings,
                errors
            ),
            span: Span {
                span: item.as_span(),
                path,
            },
        },
        Rule::expr => {
            errors.push(CompileError::InvalidExpressionOnLhs {
                span: Span {
                    span: item.as_span(),
                    path,
                },
            });
            return err(warnings, errors);
        }
        a => unreachable!("{:?}", a),
    };
    ok(exp, warnings, errors)
}

fn parse_simple_variable_reassignment(
    pair: Pair<Rule>,
    config: Option<&BuildConfig>,
    path: Option<std::sync::Arc<std::path::PathBuf>>,
) -> CompileResult<(Expression, Expression)> {
    let mut warnings = vec![];
    let mut errors = vec![];
    let mut iter = pair.into_inner();
    let name = check!(
        Expression::parse_from_pair(iter.next().unwrap(), config),
        return err(warnings, errors),
        warnings,
        errors
    );
    let body = iter.next().unwrap();
    let body = check!(
        Expression::parse_from_pair(body.clone(), config),
        Expression::Tuple {
            fields: vec![],
            span: Span {
                span: body.as_span(),
                path
            }
        },
        warnings,
        errors
    );

    ok((name, body), warnings, errors)
}
fn parse_subfield_reassignment(
    pair: Pair<Rule>,
    config: Option<&BuildConfig>,
    path: Option<std::sync::Arc<std::path::PathBuf>>,
) -> CompileResult<(Expression, Expression)> {
    let mut warnings = vec![];
    let mut errors = vec![];
    let mut iter = pair.into_inner();
    let lhs = iter.next().expect("guaranteed by grammar");
    let rhs = iter.next().expect("guaranteed by grammar");
    let rhs_span = Span {
        span: rhs.as_span(),
        path: path.clone(),
    };
    let body = check!(
        Expression::parse_from_pair(rhs, config),
        Expression::Tuple {
            fields: vec![],
            span: rhs_span
        },
        warnings,
        errors
    );

    let inner = lhs.into_inner().next().expect("guaranteed by grammar");
    assert_eq!(inner.as_rule(), Rule::subfield_path);

    // treat parent as one expr, final name as the field to be accessed
    // if there are multiple fields, this is a nested expression
    // i.e. `a.b.c` is a lookup of field `c` on `a.b` which is a lookup
    // of field `b` on `a`
    // the first thing is either an exp or a var, everything subsequent must be
    // a field
    let mut name_parts = inner.into_inner();
    let mut expr = check!(
        parse_subfield_path_ensure_only_var(
            name_parts.next().expect("guaranteed by grammar"),
            config
        ),
        return err(warnings, errors),
        warnings,
        errors
    );

    for name_part in name_parts {
        expr = Expression::SubfieldExpression {
            prefix: Box::new(expr.clone()),
            span: Span {
                span: name_part.as_span(),
                path: path.clone(),
            },
            field_to_access: check!(
                ident::parse_from_pair(name_part, config),
                continue,
                warnings,
                errors
            ),
        }
    }

    ok((expr, body), warnings, errors)
}
