use super::*;

use crate::{parse_tree::AsmOp, semantic_analysis::ast_node::*, Ident};
use sway_types::state::StateIndex;

#[derive(Clone, Debug)]
pub(crate) struct ContractCallMetadata {
    pub(crate) func_selector: [u8; 4],
    pub(crate) contract_address: Box<TypedExpression>,
}

#[derive(Clone, Debug)]
pub(crate) enum TypedExpressionVariant {
    Literal(Literal),
    FunctionApplication {
        name: CallPath,
        arguments: Vec<(Ident, TypedExpression)>,
        function_body: TypedCodeBlock,
        /// If this is `Some(val)` then `val` is the metadata. If this is `None`, then
        /// there is no selector.
        selector: Option<ContractCallMetadata>,
    },
    LazyOperator {
        op: LazyOp,
        lhs: Box<TypedExpression>,
        rhs: Box<TypedExpression>,
    },
    VariableExpression {
        name: Ident,
    },
    Tuple {
        fields: Vec<TypedExpression>,
    },
    Array {
        contents: Vec<TypedExpression>,
    },
    ArrayIndex {
        prefix: Box<TypedExpression>,
        index: Box<TypedExpression>,
    },
    StructExpression {
        struct_name: Ident,
        fields: Vec<TypedStructExpressionField>,
    },
    CodeBlock(TypedCodeBlock),
    // a flag that this value will later be provided as a parameter, but is currently unknown
    FunctionParameter,
    IfExp {
        condition: Box<TypedExpression>,
        then: Box<TypedExpression>,
        r#else: Option<Box<TypedExpression>>,
    },
    AsmExpression {
        registers: Vec<TypedAsmRegisterDeclaration>,
        body: Vec<AsmOp>,
        returns: Option<(AsmRegister, Span)>,
        whole_block_span: Span,
    },
    // like a variable expression but it has multiple parts,
    // like looking up a field in a struct
    StructFieldAccess {
        prefix: Box<TypedExpression>,
        field_to_access: OwnedTypedStructField,
        resolved_type_of_parent: TypeId,
        field_to_access_span: Span,
    },
    EnumArgAccess {
        prefix: Box<TypedExpression>,
        //variant_to_access: TypedEnumVariant,
        arg_num_to_access: usize,
        resolved_type_of_parent: TypeId,
    },
    TupleElemAccess {
        prefix: Box<TypedExpression>,
        elem_to_access_num: usize,
        resolved_type_of_parent: TypeId,
        elem_to_access_span: Span,
    },
    EnumInstantiation {
        /// for printing
        enum_decl: TypedEnumDeclaration,
        /// for printing
        variant_name: Ident,
        tag: usize,
        contents: Option<Box<TypedExpression>>,
    },
    AbiCast {
        abi_name: CallPath,
        address: Box<TypedExpression>,
        #[allow(dead_code)]
        // this span may be used for errors in the future, although it is not right now.
        span: Span,
    },
    StorageAccess(TypeCheckedStorageAccess),
    SizeOf {
        variant: SizeOfVariant,
    },
}

/// Whether a storage access is _writing_, i.e. _storing_; or _reading_, i.e. _loading_.
#[derive(Clone, Debug, Copy)]
pub enum StoreOrLoad {
    Store,
    Load,
}

#[derive(Clone, Debug)]
pub struct TypeCheckedStorageAccess {
    pub(crate) field_ix_and_name: Option<(StateIndex, Ident)>,
    pub(crate) store_or_load: StoreOrLoad,
}

impl TypeCheckedStorageAccess {
    pub fn mode(&self) -> StoreOrLoad {
        self.store_or_load
    }
    pub fn field_ix(&self) -> Option<&StateIndex> {
        self.field_ix_and_name.as_ref().map(|(x, _)| x)
    }
    pub fn field_name(&self) -> Option<&Ident> {
        self.field_ix_and_name.as_ref().map(|(_, x)| x)
    }
    pub fn new_store(field: StateIndex, name: Ident) -> Self {
        Self {
            field_ix_and_name: Some((field, name)),
            store_or_load: StoreOrLoad::Store,
        }
    }
    pub fn new_load(field: StateIndex, name: Ident) -> Self {
        Self {
            field_ix_and_name: Some((field, name)),
            store_or_load: StoreOrLoad::Load,
        }
    }

    pub fn unit_access() -> Self {
        Self {
            field_ix_and_name: None,
            // this doesn't actually matter since `unit_access` does nothing,
            // so we go with `Load` as it implies less overhead
            store_or_load: StoreOrLoad::Load,
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) enum SizeOfVariant {
    Type(TypeId),
    Val(Box<TypedExpression>),
}

#[derive(Clone, Debug)]
pub(crate) struct TypedAsmRegisterDeclaration {
    pub(crate) initializer: Option<TypedExpression>,
    pub(crate) name: Ident,
}

impl TypedAsmRegisterDeclaration {
    pub(crate) fn copy_types(&mut self, type_mapping: &[(TypeParameter, TypeId)]) {
        if let Some(ref mut initializer) = self.initializer {
            initializer.copy_types(type_mapping)
        }
    }
}

impl TypedExpressionVariant {
    pub(crate) fn pretty_print(&self) -> String {
        match self {
            TypedExpressionVariant::Literal(lit) => format!(
                "literal {}",
                match lit {
                    Literal::U8(content) => content.to_string(),
                    Literal::U16(content) => content.to_string(),
                    Literal::U32(content) => content.to_string(),
                    Literal::U64(content) => content.to_string(),
                    Literal::Numeric(content) => content.to_string(),
                    Literal::String(content) => content.as_str().to_string(),
                    Literal::Boolean(content) => content.to_string(),
                    Literal::Byte(content) => content.to_string(),
                    Literal::B256(content) => content
                        .iter()
                        .map(|x| x.to_string())
                        .collect::<Vec<_>>()
                        .join(", "),
                }
            ),
            TypedExpressionVariant::FunctionApplication { name, .. } => {
                format!("\"{}\" fn entry", name.suffix.as_str())
            }
            TypedExpressionVariant::LazyOperator { op, .. } => match op {
                LazyOp::And => "&&".into(),
                LazyOp::Or => "||".into(),
            },
            TypedExpressionVariant::Tuple { fields } => {
                let fields = fields
                    .iter()
                    .map(|field| field.pretty_print())
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("tuple({})", fields)
            }
            TypedExpressionVariant::Array { .. } => "array".into(),
            TypedExpressionVariant::ArrayIndex { .. } => "[..]".into(),
            TypedExpressionVariant::StructExpression { struct_name, .. } => {
                format!("\"{}\" struct init", struct_name.as_str())
            }
            TypedExpressionVariant::CodeBlock(_) => "code block entry".into(),
            TypedExpressionVariant::FunctionParameter => "fn param access".into(),
            TypedExpressionVariant::IfExp { .. } => "if exp".into(),
            TypedExpressionVariant::AsmExpression { .. } => "inline asm".into(),
            TypedExpressionVariant::AbiCast { abi_name, .. } => {
                format!("abi cast {}", abi_name.suffix.as_str())
            }
            TypedExpressionVariant::StructFieldAccess {
                resolved_type_of_parent,
                field_to_access,
                ..
            } => {
                format!(
                    "\"{}.{}\" struct field access",
                    look_up_type_id(*resolved_type_of_parent).friendly_type_str(),
                    field_to_access.name
                )
            }
            TypedExpressionVariant::EnumArgAccess {
                resolved_type_of_parent,
                arg_num_to_access,
                ..
            } => {
                format!(
                    "\"{}.{}\" arg num access",
                    look_up_type_id(*resolved_type_of_parent).friendly_type_str(),
                    arg_num_to_access
                )
            }
            TypedExpressionVariant::TupleElemAccess {
                resolved_type_of_parent,
                elem_to_access_num,
                ..
            } => {
                format!(
                    "\"{}.{}\" tuple index",
                    look_up_type_id(*resolved_type_of_parent).friendly_type_str(),
                    elem_to_access_num
                )
            }
            TypedExpressionVariant::VariableExpression { name, .. } => {
                format!("\"{}\" variable exp", name.as_str())
            }
            TypedExpressionVariant::EnumInstantiation {
                tag,
                enum_decl,
                variant_name,
                ..
            } => {
                format!(
                    "{}::{} enum instantiation (tag: {})",
                    enum_decl.name.as_str(),
                    variant_name.as_str(),
                    tag
                )
            }
            TypedExpressionVariant::StorageAccess(access) => match access.field_name() {
                Some(name) => format!("storage field {} access", name.as_str()),
                None => "storage struct access".into(),
                },
            
            TypedExpressionVariant::SizeOf { variant } => match variant {
                SizeOfVariant::Val(exp) => format!("size_of_val({:?})", exp.pretty_print()),
                SizeOfVariant::Type(type_name) => {
                    format!("size_of({:?})", type_name.friendly_type_str())
                }
            },
        }
    }
    /// Makes a fresh copy of all type ids in this expression. Used when monomorphizing.
    pub(crate) fn copy_types(&mut self, type_mapping: &[(TypeParameter, TypeId)]) {
        use TypedExpressionVariant::*;
        match self {
            Literal(..) => (),
            FunctionApplication {
                arguments,
                function_body,
                ..
            } => {
                arguments
                    .iter_mut()
                    .for_each(|(_ident, expr)| expr.copy_types(type_mapping));
                function_body.copy_types(type_mapping);
            }
            LazyOperator { lhs, rhs, .. } => {
                (*lhs).copy_types(type_mapping);
                (*rhs).copy_types(type_mapping);
            }
            VariableExpression { .. } => (),
            Tuple { fields } => fields.iter_mut().for_each(|x| x.copy_types(type_mapping)),
            Array { contents } => contents.iter_mut().for_each(|x| x.copy_types(type_mapping)),
            ArrayIndex { prefix, index } => {
                (*prefix).copy_types(type_mapping);
                (*index).copy_types(type_mapping);
            }
            StructExpression { fields, .. } => {
                fields.iter_mut().for_each(|x| x.copy_types(type_mapping))
            }
            CodeBlock(block) => {
                block.copy_types(type_mapping);
            }
            FunctionParameter => (),
            IfExp {
                condition,
                then,
                r#else,
            } => {
                condition.copy_types(type_mapping);
                then.copy_types(type_mapping);
                if let Some(ref mut r#else) = r#else {
                    r#else.copy_types(type_mapping);
                }
            }
            AsmExpression {
                registers, //: Vec<TypedAsmRegisterDeclaration>,
                ..
            } => {
                registers
                    .iter_mut()
                    .for_each(|x| x.copy_types(type_mapping));
            }
            // like a variable expression but it has multiple parts,
            // like looking up a field in a struct
            StructFieldAccess {
                prefix,
                field_to_access,
                ref mut resolved_type_of_parent,
                ..
            } => {
                *resolved_type_of_parent = if let Some(matching_id) =
                    look_up_type_id(*resolved_type_of_parent).matches_type_parameter(type_mapping)
                {
                    insert_type(TypeInfo::Ref(matching_id))
                } else {
                    insert_type(look_up_type_id_raw(*resolved_type_of_parent))
                };

                field_to_access.copy_types(type_mapping);
                prefix.copy_types(type_mapping);
            }
            EnumArgAccess {
                prefix,
                ref mut resolved_type_of_parent,
                ..
            } => {
                *resolved_type_of_parent = if let Some(matching_id) =
                    look_up_type_id(*resolved_type_of_parent).matches_type_parameter(type_mapping)
                {
                    insert_type(TypeInfo::Ref(matching_id))
                } else {
                    insert_type(look_up_type_id_raw(*resolved_type_of_parent))
                };

                prefix.copy_types(type_mapping);
            }
            TupleElemAccess {
                prefix,
                ref mut resolved_type_of_parent,
                ..
            } => {
                *resolved_type_of_parent = if let Some(matching_id) =
                    look_up_type_id(*resolved_type_of_parent).matches_type_parameter(type_mapping)
                {
                    insert_type(TypeInfo::Ref(matching_id))
                } else {
                    insert_type(look_up_type_id_raw(*resolved_type_of_parent))
                };

                prefix.copy_types(type_mapping);
            }
            EnumInstantiation {
                enum_decl,
                contents,
                ..
            } => {
                enum_decl.copy_types(type_mapping);
                if let Some(ref mut contents) = contents {
                    contents.copy_types(type_mapping)
                };
            }
            AbiCast { address, .. } => address.copy_types(type_mapping),
            // storage is never generic and cannot be monomorphized
            StorageAccess { .. } => (),
            SizeOf { variant } => match variant {
                SizeOfVariant::Type(_) => (),
                SizeOfVariant::Val(exp) => exp.copy_types(type_mapping),
            },
        }
    }
}
