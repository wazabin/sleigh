// These types are part of the source AST that `unstable-syntax` exposes; without
// that feature nothing outside the crate can reach them.
#![cfg_attr(not(feature = "unstable-syntax"), allow(unreachable_pub))]

use crate::objects::{field::FieldId, table::TableId};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum Expr {
    Binary {
        op: BinOp,
        lhs: Box<Expr>,
        rhs: Box<Expr>,
    },
    Unary {
        op: UnOp,
        expr: Box<Expr>,
    },
    Atom(Atom),
}

/// An infix operator in a disassembly-action expression.
///
/// Actions compute with whole field values at decode time, so — unlike p-code
/// — there is one arithmetic per operator and no size or signedness to choose.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BinOp {
    /// `|` — bitwise or. Also spelled `$or`.
    Or,
    /// `^` — bitwise exclusive-or. Also spelled `$xor`.
    Xor,
    /// `&` — bitwise and. Also spelled `$and`.
    And,
    /// `<<` — left shift.
    Shl,
    /// `>>` — right shift.
    Shr,
    /// `+` — addition, wrapping.
    Add,
    /// `-` — subtraction, wrapping.
    Sub,
    /// `*` — multiplication, wrapping.
    Mul,
    /// `/` — division. A zero divisor fails the decode rather than trapping.
    Div,
}

/// A prefix operator in a disassembly-action expression.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum UnOp {
    /// `-` — negation, wrapping.
    Neg,
    /// `~` — bitwise complement.
    Not,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum Atom {
    /// An identifier
    Ident(FieldId),

    Int(i64),
}

impl Expr {
    /// Evaluate an expression using a fallible field evaluator.
    ///
    /// Operands come from instruction bytes, so every operation here is
    /// attacker-controlled and must not panic. Returns `None` — which the
    /// decoder reports as [`DecodeError::InvalidAction`] — when:
    ///
    /// - the evaluator returns `None` for any field;
    /// - a division has a zero divisor, or is `i64::MIN / -1`;
    /// - a shift distance is negative or at least 64.
    ///
    /// Addition, subtraction, multiplication and negation wrap on overflow
    /// rather than failing, matching the two's-complement arithmetic a
    /// disassembly action expects when it computes an address.
    ///
    /// [`DecodeError::InvalidAction`]: crate::DecodeError::InvalidAction
    pub(crate) fn eval_fallible<F>(&self, field_evaluator: &F) -> Option<i64>
    where
        F: Fn(FieldId) -> Option<i64>,
    {
        match self {
            &Expr::Atom(Atom::Int(v)) => Some(v),

            &Expr::Atom(Atom::Ident(id)) => field_evaluator(id),

            Expr::Unary { op, expr } => {
                let v = expr.eval_fallible(field_evaluator)?;
                Some(match op {
                    UnOp::Neg => v.wrapping_neg(),
                    UnOp::Not => !v,
                })
            }

            Expr::Binary { op, lhs, rhs } => {
                let l = lhs.eval_fallible(field_evaluator)?;
                let r = rhs.eval_fallible(field_evaluator)?;
                // A shift distance is only meaningful in 0..64; anything else
                // is a malformed action rather than a value to mask down.
                let shift = || u32::try_from(r).ok().filter(|&s| s < i64::BITS);
                match op {
                    BinOp::Add => Some(l.wrapping_add(r)),
                    BinOp::Sub => Some(l.wrapping_sub(r)),
                    BinOp::Mul => Some(l.wrapping_mul(r)),
                    BinOp::Div => l.checked_div(r),
                    BinOp::And => Some(l & r),
                    BinOp::Or => Some(l | r),
                    BinOp::Xor => Some(l ^ r),
                    BinOp::Shl => shift().map(|s| l.wrapping_shl(s)),
                    BinOp::Shr => shift().map(|s| l.wrapping_shr(s)),
                }
            }
        }
    }

    /// Evaluates the expression in the relativity domain: whether its value
    /// is a plain number, the instruction's address plus a constant, or
    /// neither. `atom` classifies each field the way [`eval_fallible`]'s
    /// evaluator would read it.
    ///
    /// This is the abstract counterpart of [`eval_fallible`], over the same
    /// tree and in the same order; see [`Relativity`] for the rules.
    ///
    /// [`eval_fallible`]: Self::eval_fallible
    pub(crate) fn relativity<F>(&self, atom: &F) -> Relativity
    where
        F: Fn(FieldId) -> Relativity,
    {
        match self {
            Expr::Atom(Atom::Int(_)) => Relativity::Number,
            &Expr::Atom(Atom::Ident(id)) => atom(id),
            Expr::Unary { op, expr } => Relativity::unary(*op, expr.relativity(atom)),
            Expr::Binary { op, lhs, rhs } => {
                Relativity::binary(*op, lhs.relativity(atom), rhs.relativity(atom))
            }
        }
    }

    fn fields(&self, set: &mut HashSet<FieldId>) {
        match self {
            Expr::Binary { lhs, rhs, .. } => {
                lhs.fields(set);
                rhs.fields(set);
            }

            Expr::Unary { expr, .. } => {
                expr.fields(set);
            }

            &Expr::Atom(Atom::Ident(id)) => {
                set.insert(id);
            }

            Expr::Atom(_) => (),
        }
    }
}

/// What a disassembly-time value is relative to.
///
/// The abstract domain [`Expr::relativity`] evaluates alongside the concrete
/// value: `Number` is a value that does not depend on where the instruction
/// sits, `InstRelative` one that equals the instruction's address plus a
/// constant (`inst_start`, `inst_next`, `inst_next + simm32`), and `Mixed` is
/// everything else that involves the address — `inst_start & ~0xfff`, the sum
/// of two addresses. The offset of an `InstRelative` value is not carried:
/// the concrete value already determines it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Relativity {
    /// Independent of the instruction's address.
    Number,
    /// The instruction's address plus a constant.
    InstRelative,
    /// Depends on the instruction's address, but not as a fixed offset from
    /// it. Never to be read as absolute.
    Mixed,
}

impl Relativity {
    /// `relative ± number` stays relative and `relative − relative` is a
    /// number; any other operator that sees an address — multiplication,
    /// masking, shifts, the sum of two addresses — loses the offset.
    pub(crate) fn binary(op: BinOp, lhs: Self, rhs: Self) -> Self {
        use Relativity::{InstRelative, Mixed, Number};
        match (op, lhs, rhs) {
            (_, Number, Number) => Number,
            (BinOp::Add, InstRelative, Number) | (BinOp::Add, Number, InstRelative) => InstRelative,
            (BinOp::Sub, InstRelative, Number) => InstRelative,
            (BinOp::Sub, InstRelative, InstRelative) => Number,
            _ => Mixed,
        }
    }

    /// Negating or complementing an address is no longer an offset from it.
    pub(crate) fn unary(_: UnOp, value: Self) -> Self {
        match value {
            Relativity::Number => Relativity::Number,
            Relativity::InstRelative | Relativity::Mixed => Relativity::Mixed,
        }
    }
}

/// Where a `globalset` commits its value.
///
/// SLEIGH allows either an expression over fields — overwhelmingly
/// `inst_next` — or the bare name of a sub-table operand, whose exported
/// address is only known once that operand has been decoded.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum GlobalSetAddr {
    /// An action expression evaluated against the current decode.
    Expr(Expr),

    /// A sub-table operand of the same constructor; the address is the value
    /// that operand's constructor exports.
    Table(TableId),
}

/// One statement of a disassembly-action block, in source order.
///
/// SLEIGH evaluates an action block sequentially, so `Assign` and `GlobalSet`
/// share one ordered list: `globalset` commits the value the named context
/// variable holds *at that point*, which a later `Assign` may then change
/// again without affecting what was committed.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum Action {
    /// `field = expr;`
    Assign {
        /// The [`FieldId`] of the field getting assigned
        field_id: FieldId,

        /// The expression for the assignment
        expr: Expr,
    },

    /// `globalset(addr, field);`
    GlobalSet {
        /// Where the committed value takes effect.
        addr: GlobalSetAddr,

        /// The context field whose current value is committed.
        field_id: FieldId,
    },
}

impl Action {
    /// The field this action assigns, if any.
    ///
    /// `globalset` has none: it commits the value its target already holds.
    pub(crate) fn written_field(&self) -> Option<FieldId> {
        match self {
            Action::Assign { field_id, .. } => Some(*field_id),
            Action::GlobalSet { .. } => None,
        }
    }

    /// Retrieves the fields used in this action
    pub(crate) fn fields(&self) -> HashSet<FieldId> {
        let mut set = HashSet::new();

        match self {
            Action::Assign { field_id, expr } => {
                set.insert(*field_id);
                expr.fields(&mut set);
            }

            Action::GlobalSet { addr, field_id } => {
                set.insert(*field_id);
                if let GlobalSetAddr::Expr(expr) = addr {
                    expr.fields(&mut set);
                }
            }
        }

        set
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn f() -> FieldId {
        FieldId::from(0usize)
    }

    /// Evaluates `<field> <op> <k>`, with the field standing in for a value
    /// read out of the instruction bytes.
    fn eval(op: BinOp, field: i64, k: i64) -> Option<i64> {
        Expr::Binary {
            op,
            lhs: Box::new(Expr::Atom(Atom::Ident(f()))),
            rhs: Box::new(Expr::Atom(Atom::Int(k))),
        }
        .eval_fallible(&|_| Some(field))
    }

    fn eval_unary(op: UnOp, field: i64) -> Option<i64> {
        Expr::Unary {
            op,
            expr: Box::new(Expr::Atom(Atom::Ident(f()))),
        }
        .eval_fallible(&|_| Some(field))
    }

    #[test]
    fn division_by_a_zero_field_fails_instead_of_panicking() {
        assert_eq!(eval(BinOp::Div, 10, 2), Some(5));
        assert_eq!(eval(BinOp::Div, 10, 0), None);
        // i64::MIN / -1 overflows the signed range.
        assert_eq!(eval(BinOp::Div, i64::MIN, -1), None);
    }

    #[test]
    fn out_of_range_shift_distances_fail_instead_of_panicking() {
        assert_eq!(eval(BinOp::Shl, 1, 3), Some(8));
        assert_eq!(eval(BinOp::Shr, -8, 1), Some(-4));

        // A distance of exactly the word width is the boundary case: masking it
        // down to zero would silently yield `l` unchanged.
        assert_eq!(eval(BinOp::Shl, 1, 64), None);
        assert_eq!(eval(BinOp::Shr, 1, 64), None);
        assert_eq!(eval(BinOp::Shl, 1, 1_000), None);
        assert_eq!(eval(BinOp::Shl, 1, -1), None);
        assert_eq!(eval(BinOp::Shr, 1, -1), None);
    }

    /// Address arithmetic in a disassembly action is two's-complement, so
    /// overflow wraps rather than failing the decode.
    #[test]
    fn additive_overflow_wraps_rather_than_failing() {
        assert_eq!(eval(BinOp::Add, i64::MAX, 1), Some(i64::MIN));
        assert_eq!(eval(BinOp::Sub, i64::MIN, 1), Some(i64::MAX));
        assert_eq!(eval(BinOp::Mul, i64::MAX, 2), Some(-2));
        assert_eq!(eval_unary(UnOp::Neg, i64::MIN), Some(i64::MIN));
    }

    /// `<lhs> <op> <rhs>` in the relativity domain.
    fn rel(op: BinOp, lhs: Relativity, rhs: Relativity) -> Relativity {
        Relativity::binary(op, lhs, rhs)
    }

    #[test]
    fn relativity_follows_the_offset_rules() {
        use Relativity::{InstRelative as R, Mixed as M, Number as N};
        let all = [
            BinOp::Or,
            BinOp::Xor,
            BinOp::And,
            BinOp::Shl,
            BinOp::Shr,
            BinOp::Add,
            BinOp::Sub,
            BinOp::Mul,
            BinOp::Div,
        ];
        for op in all {
            assert_eq!(rel(op, N, N), N, "{op:?}");
            assert_eq!(rel(op, M, N), M, "{op:?}");
            assert_eq!(rel(op, N, M), M, "{op:?}");
            assert_eq!(rel(op, M, M), M, "{op:?}");
            assert_eq!(rel(op, R, M), M, "{op:?}");
        }
        assert_eq!(rel(BinOp::Add, R, N), R);
        assert_eq!(rel(BinOp::Add, N, R), R);
        assert_eq!(rel(BinOp::Sub, R, N), R);
        assert_eq!(rel(BinOp::Sub, R, R), N);
        // A sum of two addresses, and a number minus an address, are not
        // offsets from one.
        assert_eq!(rel(BinOp::Add, R, R), M);
        assert_eq!(rel(BinOp::Sub, N, R), M);
        for op in [
            BinOp::Or,
            BinOp::Xor,
            BinOp::And,
            BinOp::Shl,
            BinOp::Shr,
            BinOp::Mul,
            BinOp::Div,
        ] {
            assert_eq!(rel(op, R, N), M, "{op:?}");
            assert_eq!(rel(op, N, R), M, "{op:?}");
        }
        assert_eq!(Relativity::unary(UnOp::Neg, R), M);
        assert_eq!(Relativity::unary(UnOp::Not, R), M);
        assert_eq!(Relativity::unary(UnOp::Not, N), N);
    }

    #[test]
    fn relativity_of_an_expression_reads_its_atoms() {
        // `(inst_start & ~0xfff) + imm`: AArch64's `adrp`.
        let page = Expr::Binary {
            op: BinOp::Add,
            lhs: Box::new(Expr::Binary {
                op: BinOp::And,
                lhs: Box::new(Expr::Atom(Atom::Ident(f()))),
                rhs: Box::new(Expr::Unary {
                    op: UnOp::Not,
                    expr: Box::new(Expr::Atom(Atom::Int(0xfff))),
                }),
            }),
            rhs: Box::new(Expr::Atom(Atom::Int(0x1000))),
        };
        assert_eq!(
            page.relativity(&|_| Relativity::InstRelative),
            Relativity::Mixed
        );
        assert_eq!(page.relativity(&|_| Relativity::Number), Relativity::Number);

        // `inst_next + simm32`: x86-64's RIP-relative operand.
        let rip = Expr::Binary {
            op: BinOp::Add,
            lhs: Box::new(Expr::Atom(Atom::Ident(f()))),
            rhs: Box::new(Expr::Atom(Atom::Int(0x10))),
        };
        assert_eq!(
            rip.relativity(&|_| Relativity::InstRelative),
            Relativity::InstRelative
        );
    }

    #[test]
    fn a_field_the_evaluator_cannot_resolve_fails() {
        let expr = Expr::Atom(Atom::Ident(f()));
        assert_eq!(expr.eval_fallible(&|_| None), None);
    }
}
