use crate::build_config::BuildConfig;
use crate::error::*;
use crate::parser::Rule;
use crate::span::Span;
use crate::{Ident, TypeInfo};
use pest::iterators::Pair;
use std::collections::HashMap;

use super::Expression;
use crate::type_engine::IntegerBits;

#[derive(Debug, Clone)]
pub struct AsmExpression {
    pub(crate) registers: Vec<AsmRegisterDeclaration>,
    pub(crate) body: Vec<AsmOp>,
    pub(crate) returns: Option<(AsmRegister, Span)>,
    pub(crate) return_type: TypeInfo,
    pub(crate) whole_block_span: Span,
}

impl AsmExpression {
    pub(crate) fn parse_from_pair<'sc>(
        pair: Pair<'sc, Rule>,
        config: Option<&BuildConfig>,
        docstrings: &mut HashMap<String, String>,
    ) -> CompileResult< Self> {
        let path = config.map(|c| c.path());
        let whole_block_span = Span {
            span: pair.as_span(),
            path: path.clone(),
        };
        let mut warnings = Vec::new();
        let mut errors = Vec::new();
        let mut iter = pair.into_inner();
        let _asm_keyword = iter.next();
        let asm_registers = iter.next().unwrap();
        let asm_registers = check!(
            AsmRegisterDeclaration::parse_from_pair(asm_registers, config, docstrings),
            return err(warnings, errors),
            warnings,
            errors
        );
        let mut asm_op_buf = Vec::new();
        let mut implicit_op_return = None;
        let mut implicit_op_type = None;
        while let Some(pair) = iter.next() {
            match pair.as_rule() {
                Rule::asm_op => {
                    let op = check!(
                        AsmOp::parse_from_pair(pair, config),
                        continue,
                        warnings,
                        errors
                    );
                    asm_op_buf.push(op);
                }
                Rule::asm_register => {
                    implicit_op_return = Some((
                        check!(
                            AsmRegister::parse_from_pair(pair.clone()),
                            continue,
                            warnings,
                            errors
                        ),
                        Span {
                            span: pair.as_span(),
                            path: path.clone(),
                        },
                    ));
                }
                Rule::type_name => {
                    implicit_op_type = Some(check!(
                        TypeInfo::parse_from_pair(pair, config),
                        continue,
                        warnings,
                        errors
                    ));
                }
                a => unreachable!("{:?}", a),
            }
        }
        let return_type = implicit_op_type.unwrap_or(if implicit_op_return.is_some() {
            TypeInfo::UnsignedInteger(IntegerBits::SixtyFour)
        } else {
            TypeInfo::Unit
        });

        ok(
            AsmExpression {
                registers: asm_registers,
                body: asm_op_buf,
                returns: implicit_op_return,
                return_type,
                whole_block_span,
            },
            warnings,
            errors,
        )
    }
}

#[derive(Debug, Clone)]
pub(crate) struct AsmOp {
    pub(crate) op_name: Ident,
    pub(crate) op_args: Vec<Ident>,
    pub(crate) span: Span,
    pub(crate) immediate: Option<Ident>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct AsmRegister {
    pub(crate) name: String,
}

impl AsmRegister {
    fn parse_from_pair<'sc>(pair: Pair<'sc, Rule>) -> CompileResult< Self> {
        ok(
            AsmRegister {
                name: pair.as_str().to_string(),
            },
            vec![],
            vec![],
        )
    }
}

impl Into<String> for AsmRegister {
    fn into(self) -> String {
        self.name.clone()
    }
}

impl AsmOp {
    fn parse_from_pair<'sc>(
        pair: Pair<'sc, Rule>,
        config: Option<&BuildConfig>,
    ) -> CompileResult< Self> {
        let path = config.map(|c| c.path());
        let mut warnings = Vec::new();
        let mut errors = Vec::new();
        let span = Span {
            span: pair.as_span(),
            path: path.clone(),
        };
        let mut iter = pair.into_inner();
        let opcode = check!(
            Ident::parse_from_pair(iter.next().unwrap(), config),
            return err(warnings, errors),
            warnings,
            errors
        );
        let mut args = vec![];
        let mut immediate_value = None;
        while let Some(pair) = iter.next() {
            match pair.as_rule() {
                Rule::asm_register => {
                    args.push(Ident {
                        primary_name: pair.as_str(),
                        span: Span {
                            span: pair.as_span(),
                            path: path.clone(),
                        },
                    });
                }
                Rule::asm_immediate => {
                    immediate_value = Some(Ident {
                        primary_name: pair.as_str().trim(),
                        span: Span {
                            span: pair.as_span(),
                            path: path.clone(),
                        },
                    });
                }
                _ => unreachable!(),
            }
        }
        ok(
            AsmOp {
                span,
                op_name: opcode,
                op_args: args,
                immediate: immediate_value,
            },
            warnings,
            errors,
        )
    }
}

#[derive(Debug, Clone)]
pub(crate) struct AsmRegisterDeclaration {
    pub(crate) name: Span,
    pub(crate) initializer: Option<Expression>,
    pub(crate) name_span: Span,
}

impl AsmRegisterDeclaration {
    fn parse_from_pair<'sc>(
        pair: Pair<'sc, Rule>,
        config: Option<&BuildConfig>,
        docstrings: &mut HashMap<String, String>,
    ) -> CompileResult< Vec<Self>> {
        let mut iter = pair.into_inner();
        let mut warnings = Vec::new();
        let mut errors = Vec::new();
        let mut reg_buf: Vec<AsmRegisterDeclaration> = Vec::new();
        while let Some(pair) = iter.next() {
            assert_eq!(pair.as_rule(), Rule::asm_register_declaration);
            let mut iter = pair.into_inner();
            let reg_name = iter.next().unwrap();
            // if there is still anything in the iterator, then it is a variable expression to be
            // assigned to that register
            let initializer = if let Some(pair) = iter.next() {
                Some(check!(
                    Expression::parse_from_pair(pair, config, docstrings),
                    return err(warnings, errors),
                    warnings,
                    errors
                ))
            } else {
                None
            };
            reg_buf.push(AsmRegisterDeclaration {
                name: reg_name.as_str(),
                name_span: Span {
                    span: reg_name.as_span(),
                    path: config.map(|c| c.path()),
                },
                initializer,
            })
        }

        ok(reg_buf, warnings, errors)
    }
}
