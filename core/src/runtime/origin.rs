//! Whether each constant of an instruction's p-code is absolute or an offset
//! from the instruction's own address.
//!
//! SLEIGH folds address arithmetic into constants while it decodes: x86-64's
//! RIP-relative operand is `addr = inst_next + simm32`, a relative branch
//! `reloc = inst_next + simm8`, and by the time p-code is emitted both are
//! plain literals. A consumer that relocates code, or reasons about
//! position-independent code, needs to tell such a literal from one that
//! really is an absolute address. This module recovers that distinction.
//!
//! # How
//!
//! Two steps, neither of which decodes the instruction again.
//!
//! 1. **Abstract interpretation of the decode.** Every disassembly-time value
//!    is classified in the [`Relativity`] domain by replaying, over the
//!    decoded instance, the disassembly actions in exactly the order
//!    [`Walker::resolve_globals`] evaluated them: `inst_start` and `inst_next`
//!    are relative, token fields and the caller's context are numbers, and
//!    [`Expr::relativity`] carries the classification through each action.
//!    Context that the instruction's own actions assign from an address is
//!    tracked too, as `Mixed`: a context field is a bit field, so storing an
//!    address in one truncates it.
//!
//! 2. **A tracer emission.** The p-code lowering never folds: every constant
//!    varnode it emits is either one leaf of the resolved semantics copied
//!    verbatim — an operand, a global, `inst_start`, a literal — or one the
//!    lowering itself synthesises (a space id, a mask, a shift count), which
//!    depends on no leaf's value. So the instance is cloned, every
//!    instruction-relative source in it displaced by [`RELATIVE_SHIFT`] and
//!    every mixed one by [`MIXED_SHIFT`], and its p-code emitted again. Each
//!    constant then moved by exactly the displacement of the source it came
//!    from: by nothing if absolute.
//!
//! Soundness rests on the emission's *shape* not depending on any value it
//! carries. That is checked, not assumed: if the tracer's p-code differs from
//! the instruction's in anything but constant values — or fails to lower —
//! every constant is reported [`ConstantOrigin::Mixed`]. A constant that
//! moved by anything other than the two displacements is `Mixed` as well. No
//! path reports an address-dependent constant as absolute.
//!
//! [`Walker::resolve_globals`]: crate::runtime::walker::Walker::resolve_globals
//! [`Expr::relativity`]: crate::action::Expr::relativity

use pcode_types::{InstructionPcode, Opcode, PcodeOp, Varnode, lower_instruction};

use crate::{
    action::{Action, Relativity},
    bitrange::BitRange,
    instance::{ConstructorInstance, OperandValue},
    objects::field::{FIELD_INST_NEXT, FIELD_INST_START, FieldParent},
    runtime::{InstructionPcodeContext, pcode::pcode_ast_for_instance},
    spec::Spec,
};

/// Where one constant of an instruction's flat p-code came from.
///
/// Reported per input varnode by [`ConstantOrigins`]; see
/// [`Instruction::constant_origins`](crate::Instruction::constant_origins).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ConstantOrigin {
    /// A value that does not depend on where the instruction sits: an
    /// immediate, an absolute address, a mask or a space id.
    Absolute(u64),
    /// The instruction's address plus `offset_from_inst_start`, wrapping:
    /// `lea rax, [rip+0x10]`, a relative branch target, a pushed return
    /// address. The constant is
    /// `instruction.address().wrapping_add_signed(offset_from_inst_start)`.
    InstRelative {
        /// The constant minus the instruction's address.
        offset_from_inst_start: i64,
    },
    /// A value that depends on the instruction's address, but not as a fixed
    /// offset from it — AArch64's `adrp` page, `inst_next & 0xffffffff` —
    /// or whose origin could not be established. Holds the constant. Never
    /// safe to treat as absolute.
    Mixed(u64),
}

impl ConstantOrigin {
    /// Whether this constant depends on the instruction's address at all.
    pub fn depends_on_address(self) -> bool {
        !matches!(self, Self::Absolute(_))
    }
}

/// The [`ConstantOrigin`] of every constant in an instruction's flat p-code:
/// a side table parallel to [`InstructionPcode::ops`].
///
/// Position `(op, input)` is the `input`-th input of the `op`-th operation,
/// in the order [`Instruction::pcode_ops`](crate::Instruction::pcode_ops)
/// returns them — which is also the order
/// [`Instruction::pcode_ops_streamed`](crate::Instruction::pcode_ops_streamed)
/// reports them, counting each `op` and `branch_label` call. A position has
/// an origin when its varnode is a constant: one in the constant space, or
/// the address operand of a direct `BRANCH`, `CBRANCH` or `CALL`, which raw
/// p-code writes in the code space. Every other varnode has none.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ConstantOrigins {
    ops: Vec<Box<[Option<ConstantOrigin>]>>,
}

impl ConstantOrigins {
    /// The origin of input `input` of operation `op`, or `None` when that
    /// varnode is not a constant (or the position does not exist).
    pub fn get(&self, op: usize, input: usize) -> Option<ConstantOrigin> {
        self.ops.get(op)?.get(input).copied().flatten()
    }

    /// The origins of operation `op`'s inputs, one per input. Empty for an
    /// operation that does not exist.
    pub fn op(&self, op: usize) -> &[Option<ConstantOrigin>] {
        self.ops.get(op).map_or(&[], |inputs| inputs)
    }

    /// Number of operations covered — that of the p-code it describes.
    pub fn len(&self) -> usize {
        self.ops.len()
    }

    /// Whether the p-code it describes has no operations.
    pub fn is_empty(&self) -> bool {
        self.ops.is_empty()
    }

    /// Every constant as `(op, input, origin)`, in emission order.
    pub fn iter(&self) -> impl Iterator<Item = (usize, usize, ConstantOrigin)> + '_ {
        self.ops.iter().enumerate().flat_map(|(op, inputs)| {
            inputs
                .iter()
                .enumerate()
                .filter_map(move |(input, origin)| origin.map(|origin| (op, input, origin)))
        })
    }
}

/// How far the tracer displaces an instruction-relative source. Odd, and
/// with every byte set, so a displacement survives in any width.
const RELATIVE_SHIFT: u64 = 0x1357_9bdf_2468_ace1;

/// How far the tracer displaces a mixed source; distinct from
/// [`RELATIVE_SHIFT`] in every byte.
const MIXED_SHIFT: u64 = 0x7a5b_3c1d_6e4f_9283;

/// Whether `op`'s input `index` holds a constant.
fn is_constant_position(op: &PcodeOp, index: usize) -> bool {
    op.inputs[index].is_constant()
        || (index == 0 && matches!(op.opcode, Opcode::Branch | Opcode::CBranch | Opcode::Call))
}

/// The origins of `pcode`, the flat p-code of `instance`.
pub(super) fn constant_origins(
    spec: &Spec,
    instance: &ConstructorInstance,
    pcode: &InstructionPcode,
) -> ConstantOrigins {
    let mut tracer = instance.clone();
    displace_instruction(spec, &mut tracer);
    let traced = pcode_ast_for_instance(spec, &tracer)
        .ok()
        .and_then(|ast| lower_instruction(&ast, &InstructionPcodeContext::new(spec)).ok());
    classify(
        instance.inst_start,
        &pcode.ops,
        traced.as_ref().map(|p| &*p.ops),
    )
}

/// Compares the instruction's p-code with the tracer's, constant by constant.
fn classify(inst_start: u64, ops: &[PcodeOp], traced: Option<&[PcodeOp]>) -> ConstantOrigins {
    let traced = traced.filter(|traced| same_shape(ops, traced));
    let ops = ops
        .iter()
        .enumerate()
        .map(|(op_index, op)| {
            (0..op.inputs.len())
                .map(|index| {
                    is_constant_position(op, index).then(|| {
                        let value = op.inputs[index].offset;
                        let Some(traced) = traced else {
                            return ConstantOrigin::Mixed(value);
                        };
                        let moved = traced[op_index].inputs[index].offset.wrapping_sub(value);
                        match moved {
                            0 => ConstantOrigin::Absolute(value),
                            RELATIVE_SHIFT => ConstantOrigin::InstRelative {
                                offset_from_inst_start: value.wrapping_sub(inst_start) as i64,
                            },
                            _ => ConstantOrigin::Mixed(value),
                        }
                    })
                })
                .collect()
        })
        .collect();
    ConstantOrigins { ops }
}

/// Whether two emissions agree in everything but their constants' values.
fn same_shape(ops: &[PcodeOp], traced: &[PcodeOp]) -> bool {
    let same_varnode = |a: &Varnode, b: &Varnode| a.space == b.space && a.size == b.size;
    ops.len() == traced.len()
        && ops.iter().zip(traced).all(|(op, traced)| {
            op.opcode == traced.opcode
                && op.output == traced.output
                && op.inputs.len() == traced.inputs.len()
                && op.inputs.iter().zip(&traced.inputs).enumerate().all(
                    |(index, (input, traced))| {
                        if is_constant_position(op, index) {
                            same_varnode(input, traced)
                        } else {
                            input == traced
                        }
                    },
                )
        })
}

/// Displaces every address-dependent source of a decoded instruction, its
/// delay-slot instructions included, by its [`Relativity`].
fn displace_instruction(spec: &Spec, instruction: &mut ConstructorInstance) {
    let mut tainted = Vec::new();
    taint_context(spec, instruction, &mut tainted);
    for delayed in &instruction.delay_slots {
        taint_context(spec, delayed, &mut tainted);
    }

    displace(spec, instruction, &tainted);
    if let Some(next2) = &mut instruction.inst_next2 {
        *next2 = next2.wrapping_add(RELATIVE_SHIFT);
    }
    // A delayed instruction sits a fixed distance past this one, so its
    // `inst_start` is relative to this instruction's too.
    for delayed in &mut instruction.delay_slots {
        displace(spec, delayed, &tainted);
    }
}

fn shift(relativity: Relativity) -> u64 {
    match relativity {
        Relativity::Number => 0,
        Relativity::InstRelative => RELATIVE_SHIFT,
        Relativity::Mixed => MIXED_SHIFT,
    }
}

/// Context bits, as inclusive `(start, end)` bit positions.
type Bits = (usize, usize);

fn overlaps(range: &BitRange, tainted: &[Bits]) -> bool {
    tainted
        .iter()
        .any(|&(start, end)| start <= range.end() && range.start() <= end)
}

/// Collects the context bits this instruction's own actions write from an
/// address-dependent value.
///
/// While constructors are being matched, a context assignment sees the
/// globals [`Walker::create_globals`] gives it — `inst_start`, and zero for
/// everything else — and the context its ancestors' actions left. Tracking
/// which ancestor wrote what is not worth the precision: a write anywhere in
/// the instruction taints the written bits everywhere, and the pass repeats
/// until no new bits are tainted, since one tainted field may feed another.
///
/// [`Walker::create_globals`]: crate::runtime::walker::Walker
fn taint_context(spec: &Spec, instance: &ConstructorInstance, tainted: &mut Vec<Bits>) {
    fn pass(spec: &Spec, instance: &ConstructorInstance, tainted: &mut Vec<Bits>) -> bool {
        let constructor = instance.constructor(spec);
        let mut grew = false;
        for action in &constructor.actions {
            let Action::Assign { field_id, expr } = action else {
                continue;
            };
            let field = &spec.fields[*field_id];
            if field.parent != FieldParent::Context {
                continue;
            }
            let relativity = expr.relativity(&|id| {
                let field = &spec.fields[id];
                match field.parent {
                    FieldParent::Token(_) => Relativity::Number,
                    FieldParent::Context if overlaps(&field.range, tainted) => Relativity::Mixed,
                    FieldParent::Context => Relativity::Number,
                    FieldParent::Global if id == FIELD_INST_START => Relativity::InstRelative,
                    FieldParent::Global => Relativity::Number,
                }
            });
            // A context field is a bit field: whatever address went in was
            // truncated to it.
            let bits = (field.range.start(), field.range.end());
            if relativity != Relativity::Number && !tainted.contains(&bits) {
                tainted.push(bits);
                grew = true;
            }
        }
        for operand in &instance.operand_values {
            if let OperandValue::Constructor(child) = operand {
                grew |= pass(spec, child, tainted);
            }
        }
        grew
    }

    while pass(spec, instance, tainted) {}
}

/// Classifies one constructor's globals the way
/// [`Walker::resolve_globals`](crate::runtime::walker::Walker::resolve_globals)
/// computed them, then displaces this constructor's sources in place.
///
/// Returns the classification of the globals, before displacement, for the
/// parent to inherit the ones it shares.
fn displace(spec: &Spec, instance: &mut ConstructorInstance, tainted: &[Bits]) -> Vec<Relativity> {
    let constructor = instance.constructor(spec);
    let mut globals = vec![Relativity::Number; instance.global_values.len()];

    for field in [FIELD_INST_START, FIELD_INST_NEXT] {
        if let Some(idx) = constructor.try_global_index(field) {
            globals[idx] = Relativity::InstRelative;
        }
    }

    for operand in &mut instance.operand_values {
        if let OperandValue::Constructor(child) = operand {
            let inherited = displace(spec, child, tainted);
            for (field_id, child_idx) in child.constructor(spec).global_pairs() {
                if let Some(idx) = constructor.try_global_index(field_id) {
                    globals[idx] = inherited[child_idx];
                }
            }
        }
    }

    for action in &constructor.actions {
        let Action::Assign { field_id, expr } = action else {
            continue;
        };
        if spec.fields[*field_id].parent != FieldParent::Global {
            continue;
        }
        let relativity = expr.relativity(&|id| match spec.fields[id].parent {
            FieldParent::Token(_) => Relativity::Number,
            // Global actions read the caller's context, which is plain state.
            FieldParent::Context => Relativity::Number,
            FieldParent::Global => constructor
                .try_global_index(id)
                .map_or(Relativity::Number, |idx| globals[idx]),
        });
        globals[constructor.global_index(*field_id)] = relativity;
    }

    instance.inst_start = instance.inst_start.wrapping_add(RELATIVE_SHIFT);
    instance.inst_next = instance.inst_next.wrapping_add(RELATIVE_SHIFT);
    for (value, relativity) in instance.global_values.iter_mut().zip(&globals) {
        *value = value.wrapping_add(shift(*relativity));
    }
    if !tainted.is_empty() {
        for (field_id, operand_id) in constructor.field_map.iter() {
            let field = &spec.fields[*field_id];
            if field.parent == FieldParent::Context
                && overlaps(&field.range, tainted)
                && let Some(OperandValue::Int(value)) =
                    instance.operand_values.get_mut(*operand_id as usize)
            {
                *value = value.wrapping_add(MIXED_SHIFT as i64);
            }
        }
    }

    globals
}

#[cfg(test)]
mod tests {
    use super::*;

    fn op(opcode: Opcode, inputs: Vec<Varnode>) -> PcodeOp {
        PcodeOp::new(opcode, None, inputs)
    }

    #[test]
    fn a_constant_is_classified_by_how_far_the_tracer_moved_it() {
        let ops = [op(
            Opcode::CallOther,
            vec![
                Varnode::constant(1, 4),
                Varnode::constant(0x1010, 8),
                Varnode::constant(0x2000, 8),
                Varnode::constant(0x3000, 8),
            ],
        )];
        let traced = [op(
            Opcode::CallOther,
            vec![
                Varnode::constant(1, 4),
                Varnode::constant(0x1010u64.wrapping_add(RELATIVE_SHIFT), 8),
                Varnode::constant(0x2000u64.wrapping_add(MIXED_SHIFT), 8),
                Varnode::constant(0x3001, 8),
            ],
        )];
        let origins = classify(0x1000, &ops, Some(&traced));
        assert_eq!(
            origins.op(0),
            [
                Some(ConstantOrigin::Absolute(1)),
                Some(ConstantOrigin::InstRelative {
                    offset_from_inst_start: 0x10
                }),
                Some(ConstantOrigin::Mixed(0x2000)),
                // Moved, but not by a displacement the tracer applies.
                Some(ConstantOrigin::Mixed(0x3000)),
            ]
        );
    }

    #[test]
    fn a_tracer_of_another_shape_makes_every_constant_mixed() {
        let register = Varnode::new(pcode_types::SpaceId::from(2usize), 0, 8);
        let ops = [op(Opcode::Copy, vec![Varnode::constant(5, 8)])];
        for traced in [
            vec![],
            vec![op(Opcode::IntZext, vec![Varnode::constant(5, 8)])],
            vec![op(Opcode::Copy, vec![Varnode::constant(5, 4)])],
            vec![op(Opcode::Copy, vec![register])],
        ] {
            let origins = classify(0x1000, &ops, Some(&traced));
            assert_eq!(origins.get(0, 0), Some(ConstantOrigin::Mixed(5)));
        }
        let origins = classify(0x1000, &ops, None);
        assert_eq!(origins.get(0, 0), Some(ConstantOrigin::Mixed(5)));
    }

    #[test]
    fn only_constants_have_an_origin() {
        let ram = pcode_types::SpaceId::from(1usize);
        let register = Varnode::new(pcode_types::SpaceId::from(2usize), 0, 8);
        let ops = [
            op(Opcode::Copy, vec![register]),
            op(Opcode::Branch, vec![Varnode::new(ram, 0x1000, 8)]),
            op(Opcode::BranchInd, vec![register]),
        ];
        let origins = classify(0x1000, &ops, Some(&ops));
        assert_eq!(origins.len(), 3);
        assert_eq!(origins.get(0, 0), None);
        assert_eq!(origins.get(1, 0), Some(ConstantOrigin::Absolute(0x1000)));
        assert_eq!(origins.get(2, 0), None);
        assert_eq!(origins.get(9, 0), None);
        assert!(origins.op(9).is_empty());
        assert_eq!(
            origins.iter().collect::<Vec<_>>(),
            [(1, 0, ConstantOrigin::Absolute(0x1000))]
        );
    }
}
