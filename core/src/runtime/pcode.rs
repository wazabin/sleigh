mod collect;
mod expand;

use crate::{
    instance::ConstructorInstance,
    objects::table::TableId,
    pmacro::expression::{BinaryOperator, Expression, ExpressionTy, Load, SpaceRef, UnaryOperator},
    semantics::{EmitError, PcodeAst},
    spec::Spec,
};
use pcode_types::{SPACE_CONST, SpaceId};

pub(super) fn pcode_ast_for_instance(
    spec: &Spec,
    instance: &ConstructorInstance,
) -> Result<PcodeAst, EmitError> {
    let mut expander = expand::PcodeExpander::new(spec, instance);
    expander.emit_instance(instance, &Default::default())?;
    Ok(PcodeAst {
        statements: expander.stmts,
    })
}

/// The address a sub-table operand exports, when it is a compile-time constant.
///
/// `globalset(SomeTable, var)` commits `var` at whatever address `SomeTable`
/// exports. In the corpus those constructors always compute a relocation into a
/// disassembly-action global and then `export *:4 reloc`, which folds to a
/// literal once the operand is decoded. Returns `None` when the export cannot
/// be expanded or does not fold — the caller reports that as a typed decode
/// error rather than guessing an address.
pub(super) fn exported_address(
    spec: &Spec,
    instance: &ConstructorInstance,
    table_id: TableId,
) -> Option<u64> {
    // Built on `instance` so the child sees the same `inst_next` a disassembly
    // action would: effect collection runs before delay slots are decoded, and
    // a `globalset` address is an action-side value either way.
    let mut expander = expand::PcodeExpander::new(spec, instance);
    let export = expander
        .emit_child_scoped(instance, table_id, &Default::default())
        .ok()??;
    const_fold(&export.into_direct_target_expr())
}

/// Folds an expanded p-code expression to a literal, if it is one.
fn const_fold(expr: &Expression) -> Option<u64> {
    match &expr.ty {
        &ExpressionTy::SizedInt { value, .. } => Some(value),

        ExpressionTy::Unop(unop) => {
            let value = const_fold(&unop.e)?;
            match unop.op {
                UnaryOperator::Minus => Some(value.wrapping_neg()),
                UnaryOperator::BitwiseNot => Some(!value),
                _ => None,
            }
        }

        ExpressionTy::Binop(binop) => {
            let lhs = const_fold(&binop.lhs)?;
            let rhs = const_fold(&binop.rhs)?;
            // Shift distances come from decoded bytes, so they are clamped the
            // same way disassembly-action evaluation clamps them.
            let shift = || u32::try_from(rhs).ok().filter(|&s| s < u64::BITS);
            match binop.op {
                BinaryOperator::Add => Some(lhs.wrapping_add(rhs)),
                BinaryOperator::Sub => Some(lhs.wrapping_sub(rhs)),
                BinaryOperator::Mul => Some(lhs.wrapping_mul(rhs)),
                BinaryOperator::Div => lhs.checked_div(rhs),
                BinaryOperator::BitwiseAnd => Some(lhs & rhs),
                BinaryOperator::BitwiseOr => Some(lhs | rhs),
                BinaryOperator::BitwiseXor => Some(lhs ^ rhs),
                BinaryOperator::LeftShift => shift().map(|s| lhs.wrapping_shl(s)),
                BinaryOperator::RightShift => shift().map(|s| lhs.wrapping_shr(s)),
                _ => None,
            }
        }

        _ => None,
    }
}

/// A runtime p-code value: either an expression or a memory address.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum RuntimeValue {
    Expr(Expression),
    Address {
        ptr: Expression,
        space: SpaceId,
        size: usize,
    },
}

impl RuntimeValue {
    /// Convert to a load expression (or the ptr directly if constant space).
    pub(crate) fn into_expr(self) -> Expression {
        match self {
            Self::Expr(expr) => expr,
            // A load from the constant space is the pointer value itself, but
            // the declared size still applies: `*[const]:4 imm` is `imm` as a
            // four-byte value, whatever width `imm` was decoded at. Dropping it
            // here made the exported operand carry the field's own width.
            Self::Address { ptr, space, size } if space == SPACE_CONST => {
                let ty = match ptr.ty {
                    // A literal carries its width in the node as well, so
                    // widening the expression alone would leave the two
                    // disagreeing.
                    ExpressionTy::SizedInt { value, .. } => ExpressionTy::SizedInt {
                        value,
                        size: Some(size),
                    },
                    other => other,
                };
                Expression {
                    ty,
                    size: Some(size),
                    span: ptr.span,
                }
            }
            Self::Address { ptr, space, size } => Expression {
                ty: ExpressionTy::Load(Load {
                    space: Some(SpaceRef::Resolved(space)),
                    size: Some(size),
                    ptr: Box::new(ptr),
                }),
                size: Some(size),
                span: (),
            },
        }
    }

    /// Return the pointer directly, ignoring space/size (for branch targets).
    pub(crate) fn into_direct_target_expr(self) -> Expression {
        match self {
            Self::Expr(expr) => expr,
            Self::Address { ptr, .. } => ptr,
        }
    }
}

/// Resolve a concrete field value from a constructor instance.
pub(super) fn make_int_expr(value: u64, size: usize) -> Expression {
    Expression {
        ty: ExpressionTy::SizedInt {
            value,
            size: Some(size),
        },
        size: Some(size),
        span: (),
    }
}
