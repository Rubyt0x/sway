use crate::build_config::BuildConfig;
use crate::control_flow_analysis::ControlFlowGraph;
use crate::error::*;
use crate::semantic_analysis::ast_node::*;
use crate::type_engine::TypeId;
use crate::types::ResolvedType;

/// Given an enum declaration and the instantiation expression/type arguments, construct a valid
/// [TypedExpression].
pub(crate) fn instantiate_enum(
    enum_decl: TypedEnumDeclaration,
    enum_field_name: Ident,
    args: Vec<Expression>,
    type_arguments: Vec<TypeId>,
    namespace: &mut Namespace,
    self_type: TypeId,
    build_config: &BuildConfig,
    dead_code_graph: &mut ControlFlowGraph,
) -> CompileResult< TypedExpression> {
    let mut warnings = vec![];
    let mut errors = vec![];
    let enum_decl = check!(
        enum_decl.resolve_generic_types(type_arguments),
        return err(warnings, errors),
        warnings,
        errors
    );
    let (enum_field_type, tag, variant_name) = match enum_decl
        .variants
        .iter()
        .find(|x| x.name.as_str() == enum_field_name.as_str())
    {
        Some(o) => (o.r#type.clone(), o.tag, o.name.clone()),
        None => {
            errors.push(CompileError::UnknownEnumVariant {
                enum_name: enum_decl.name.as_str(),
                variant_name: enum_field_name.as_str(),
                span: enum_field_name.clone().span,
            });
            return err(warnings, errors);
        }
    };

    // If there is an instantiator, it must match up with the type. If there is not an
    // instantiator, then the type of the enum is necessarily the unit type.

    match (&args[..], namespace.look_up_type_id(enum_field_type)) {
        ([], ResolvedType::Unit) => ok(
            TypedExpression {
                return_type: enum_decl.as_type(namespace),
                expression: TypedExpressionVariant::EnumInstantiation {
                    tag,
                    contents: None,
                    enum_decl,
                    variant_name,
                },
                is_constant: IsConstant::No,
                span: enum_field_name.span.clone(),
            },
            warnings,
            errors,
        ),
        ([single_expr], r#type) => {
            let typed_expr = check!(
                TypedExpression::type_check(
                    single_expr.clone(),
                    namespace,
                    Some(enum_field_type),
                    "Enum instantiator must match its declared variant type.",
                    self_type,
                    build_config,
                    dead_code_graph,
                ),
                return err(warnings, errors),
                warnings,
                errors
            );

            // we now know that the instantiator type matches the declared type, via the above tpe
            // check

            ok(
                TypedExpression {
                    return_type: enum_decl.as_type(namespace),
                    expression: TypedExpressionVariant::EnumInstantiation {
                        tag,
                        contents: Some(Box::new(typed_expr)),
                        enum_decl,
                        variant_name,
                    },
                    is_constant: IsConstant::No,
                    span: enum_field_name.span.clone(),
                },
                warnings,
                errors,
            )
        }
        ([], _) => {
            errors.push(CompileError::MissingEnumInstantiator {
                span: enum_field_name.span.clone(),
            });
            return err(warnings, errors);
        }
        (_too_many_expressions, ResolvedType::Unit) => {
            errors.push(CompileError::UnnecessaryEnumInstantiator {
                span: enum_field_name.span.clone(),
            });
            return err(warnings, errors);
        }
        (_too_many_expressions, ty) => {
            errors.push(CompileError::MoreThanOneEnumInstantiator {
                span: enum_field_name.span.clone(),
                ty: ty.friendly_type_str(),
            });
            return err(warnings, errors);
        }
    }
}
