use crate::{
    asm_generation::{AsmNamespace, RegisterSequencer},
    asm_lang::Op,
    error::*,
    TypedFunctionDeclaration,
};

pub(crate) fn convert_fn_decl_to_asm<'sc>(
    _decl: &TypedFunctionDeclaration,
    _namespace: &mut AsmNamespace,
    _register_sequencer: &mut RegisterSequencer,
) -> CompileResult< Vec<Op<'sc>>> {
    // for now, we inline all functions as a shortcut.
    ok(vec![], vec![], vec![])
}
