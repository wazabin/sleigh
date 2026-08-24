mod ids;
mod ops;
mod types;

pub(crate) use ids::LocalVarInterner;
pub use ids::{Builtin, LocalVarId};
pub use ops::{BinaryOperator, UnaryOperator};
pub use types::{Binop, Load, Range, RangeParam, SpaceRef, Unop};

use crate::{
    builder::SymbolId,
    objects::{field::FieldId, table::TableId},
    pmacro::PMacroId,
    runtime::CompiledSpec,
    token::BitRangeFieldId,
};
use pcode_types::{PCodeOpId, RegisterId};
use serde::{Deserialize, Serialize};

/// A named thing a p-code expression refers to.
///
/// Everything but [`Ident::Global`] is fully resolved: by the time an
/// instruction's AST reaches a consumer, a name has become an index into the
/// compiled specification. Look identifiers up through
/// [`CompiledSpec`](crate::CompiledSpec) — `registers()` for a
/// [`RegisterId`](pcode_types::RegisterId), `bitrange_info` for a bit-range
/// field, and so on.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Ident {
    /// A temporary declared inside a constructor or macro body — `local x:4`,
    /// or an assignment to a name that was never declared. Unique within one
    /// decoded instruction; see [`LocalVarId`].
    Named(LocalVarId),

    /// A machine register, as named by `define register`.
    Register(RegisterId),

    /// A named sub-range of a register, as named by `define bitrange`.
    /// [`CompiledSpec::bitrange_info`](crate::CompiledSpec::bitrange_info)
    /// gives the parent register and the byte window.
    BitRange(BitRangeFieldId),

    /// A token, context or global field. In an emitted instruction this is
    /// normally already folded to a constant; one surviving here means the
    /// decode could not supply a value for it.
    Field(FieldId),

    /// A sub-table operand. One surviving in an emitted instruction means its
    /// sub-constructor exported nothing.
    Table(TableId),
    /// An identifier absent from the symbol table during Phase 2 parsing.
    /// Resolved to the appropriate variant by the Phase 3 resolve pass.
    Global(Box<str>),
}

/// The shape of a p-code expression node.
///
/// `S` is the span type: `(usize, usize)` at parse time, `()` in the stored/runtime form.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExpressionTy<S = ()> {
    /// An integer literal.
    SizedInt {
        /// The value, zero-extended into a `u64`.
        value: u64,
        /// Explicit width in bytes from a `:n` suffix, or `None` when the
        /// literal takes its width from the context it appears in.
        size: Option<usize>,
    },

    /// `x(n)` — drop the low `n` bytes of `x`, keeping the high end.
    SubPieceMsb {
        /// The value being truncated.
        src: Box<Expression<S>>,
        /// How many bytes to drop from the bottom.
        count: usize,
    },

    /// `x:n` — keep the low `n` bytes of `x`.
    SubPieceLsb {
        /// The value being truncated.
        src: Box<Expression<S>>,
        /// How many low bytes to keep.
        count: usize,
    },

    /// `*[space]:n ptr` — a memory read.
    Load(Load<S>),

    /// `x[start, size]` — a bit range of a value.
    Range(Range<S>),

    /// A call to one of SLEIGH's built-in functions.
    FunctionCall {
        /// Which builtin.
        builtin: Builtin,
        /// Its arguments, in source order.
        args: Vec<Expression<S>>,
    },

    /// A call to a `define pcodeop` — an operation the specification declares
    /// but does not define, so a consumer must give it meaning. The name is
    /// the `id`-th entry of
    /// [`CompiledSpec::pcode_ops`](crate::CompiledSpec::pcode_ops).
    PcodeOp {
        /// Index into the specification's user-defined operation list.
        id: PCodeOpId,
        /// Its arguments, in source order.
        args: Vec<Expression<S>>,
    },

    /// A call to a `macro`. Expanded away before a consumer sees the AST;
    /// one surviving is a bug in this crate.
    MacroCall {
        /// The macro being called.
        id: PMacroId,
        /// Its arguments, in source order.
        args: Vec<Expression<S>>,
    },

    /// A call to a name the symbol table did not hold — necessarily a macro
    /// parameter, substituted when the macro is inlined. A consumer does not
    /// see this variant.
    DeferredCall {
        /// The name being called.
        name: Box<str>,
        /// Its arguments, in source order.
        args: Vec<Expression<S>>,
    },

    /// A reference to a named thing.
    Ident(Ident),

    /// A prefix operator applied to one operand.
    Unop(Unop<S>),

    /// An infix operator applied to two operands.
    Binop(Binop<S>),
}

impl ExpressionTy {
    pub(crate) fn pretty_print(&self, spec: &CompiledSpec) -> String {
        match self {
            ExpressionTy::SizedInt { value, size } => match size {
                Some(size) => format!("{value}:{size}"),
                None => value.to_string(),
            },
            ExpressionTy::SubPieceMsb { src, count } => {
                format!("subpiece_msb({}, {})", src.pretty_print(spec), count)
            }
            ExpressionTy::SubPieceLsb { src, count } => {
                format!("subpiece_lsb({}, {})", src.pretty_print(spec), count)
            }
            ExpressionTy::Load(load) => load.pretty_print(spec),
            ExpressionTy::Range(range) => range.pretty_print(spec),
            ExpressionTy::FunctionCall { builtin, args } => {
                format!("{}({})", builtin.as_str(), pretty_print_args(args, spec))
            }
            ExpressionTy::PcodeOp { id, args } => format!(
                "{}({})",
                spec.spec().pcode_ops[*id],
                pretty_print_args(args, spec)
            ),
            ExpressionTy::MacroCall { id, args } => format!(
                "{}({})",
                pretty_print_macro_name(spec, *id),
                pretty_print_args(args, spec)
            ),
            ExpressionTy::DeferredCall { name, args } => {
                format!("{}({})", name, pretty_print_args(args, spec))
            }
            ExpressionTy::Ident(ident) => pretty_print_ident(spec, ident),
            ExpressionTy::Unop(unop) => unop.pretty_print(spec),
            ExpressionTy::Binop(binop) => binop.pretty_print(spec),
        }
    }
}

impl<S> ExpressionTy<S> {
    /// Discards source spans.
    pub fn strip_span(self) -> ExpressionTy<()> {
        match self {
            ExpressionTy::SizedInt { value, size } => ExpressionTy::SizedInt { value, size },
            ExpressionTy::SubPieceMsb { src, count } => ExpressionTy::SubPieceMsb {
                src: Box::new(src.strip_span()),
                count,
            },
            ExpressionTy::SubPieceLsb { src, count } => ExpressionTy::SubPieceLsb {
                src: Box::new(src.strip_span()),
                count,
            },
            ExpressionTy::Load(load) => ExpressionTy::Load(load.strip_span()),
            ExpressionTy::Range(range) => ExpressionTy::Range(range.strip_span()),
            ExpressionTy::FunctionCall { builtin, args } => ExpressionTy::FunctionCall {
                builtin,
                args: args.into_iter().map(Expression::strip_span).collect(),
            },
            ExpressionTy::PcodeOp { id, args } => ExpressionTy::PcodeOp {
                id,
                args: args.into_iter().map(Expression::strip_span).collect(),
            },
            ExpressionTy::MacroCall { id, args } => ExpressionTy::MacroCall {
                id,
                args: args.into_iter().map(Expression::strip_span).collect(),
            },
            ExpressionTy::DeferredCall { name, args } => ExpressionTy::DeferredCall {
                name,
                args: args.into_iter().map(Expression::strip_span).collect(),
            },
            ExpressionTy::Ident(ident) => ExpressionTy::Ident(ident),
            ExpressionTy::Unop(unop) => ExpressionTy::Unop(unop.strip_span()),
            ExpressionTy::Binop(binop) => ExpressionTy::Binop(binop.strip_span()),
        }
    }
}

/// A p-code expression: a node kind, plus the width its value has.
///
/// `S` is the span type. It is `(usize, usize)` — a byte range into the
/// preprocessed source — while the compiler is lowering, and `()` in the form
/// a consumer receives, which is why [`PcodeExpr`](crate::PcodeExpr) defaults
/// it away.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Expression<S = ()> {
    /// What kind of expression this is, and its operands.
    pub ty: ExpressionTy<S>,

    /// Width of the value in bytes.
    ///
    /// `None` means the width was not written in the source and could not be
    /// inferred — a literal in a position that does not pin one down, most
    /// often. A consumer that needs a width must supply one from context
    /// rather than assume.
    pub size: Option<usize>,

    /// Where this node came from in the preprocessed source, or `()` once the
    /// compiler is done with it.
    pub span: S,
}

impl<S: std::fmt::Debug> std::fmt::Debug for Expression<S> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(size) = self.size {
            write!(f, "{:?} (size: {:?})", self.ty, size)
        } else {
            self.ty.fmt(f)
        }
    }
}

impl<S> Expression<S> {
    /// Discards source spans, giving the form a consumer receives.
    pub fn strip_span(self) -> Expression<()> {
        Expression {
            ty: self.ty.strip_span(),
            size: self.size,
            span: (),
        }
    }
}

impl Expression {
    /// Renders this expression in a SLEIGH-like syntax, resolving identifiers
    /// against `spec`. For diagnostics and tests; not a stable format.
    pub fn pretty_print(&self, spec: &CompiledSpec) -> String {
        self.ty.pretty_print(spec)
    }
}

impl Expression<(usize, usize)> {
    pub(crate) fn new_int(value: u64, size: Option<usize>, span: (usize, usize)) -> Self {
        Self {
            ty: ExpressionTy::SizedInt { value, size },
            size,
            span,
        }
    }
}

fn pretty_print_args(args: &[Expression], spec: &CompiledSpec) -> String {
    args.iter()
        .map(|arg| arg.pretty_print(spec))
        .collect::<Vec<_>>()
        .join(", ")
}

pub(crate) fn pretty_print_ident(spec: &CompiledSpec, ident: &Ident) -> String {
    match ident {
        Ident::Named(id) => format!("v{}", id.0),
        Ident::Register(id) => spec.spec().registers[*id].name.to_string(),
        Ident::BitRange(id) => spec.spec().bitranges[*id].name.to_string(),
        Ident::Field(id) => spec.spec().fields[*id].name.to_string(),
        Ident::Table(id) => format!("table{}", usize::from(*id)),
        Ident::Global(name) => format!("?{name}"),
    }
}

fn pretty_print_macro_name(spec: &CompiledSpec, id: PMacroId) -> String {
    spec.spec()
        .symbols
        .iter()
        .find_map(|(name, &symbol)| match symbol {
            SymbolId::Macro(candidate) if candidate == id => Some(name.to_string()),
            _ => None,
        })
        .unwrap_or_else(|| "<unknown-macro>".to_string())
}

pub(crate) fn infer_expr_size(spec: &crate::spec::Spec, expr: &mut Expression) -> Option<usize> {
    if expr.size.is_some() {
        return expr.size;
    }
    let size = match &mut expr.ty {
        ExpressionTy::SizedInt { size, .. } => *size,
        ExpressionTy::Ident(ident) => ident_size(spec, ident),
        ExpressionTy::Load(load) => load.size,
        ExpressionTy::SubPieceMsb { src, count } => {
            infer_expr_size(spec, src).map(|size| size.saturating_sub(*count))
        }
        ExpressionTy::SubPieceLsb { count, .. } => Some(*count),
        ExpressionTy::Range(_) => None,
        ExpressionTy::FunctionCall { builtin, args } => match builtin {
            Builtin::Carry | Builtin::Scarry | Builtin::Sborrow | Builtin::Nan => Some(1),
            Builtin::Abs | Builtin::Sqrt | Builtin::Floor | Builtin::Ceil | Builtin::Round => {
                args.first_mut().and_then(|arg| infer_expr_size(spec, arg))
            }
            _ => None,
        },
        ExpressionTy::PcodeOp { .. } => None,
        ExpressionTy::MacroCall { .. } | ExpressionTy::DeferredCall { .. } => None,
        ExpressionTy::Unop(unop) => match unop.op {
            UnaryOperator::LogicalNot => Some(1),
            UnaryOperator::AddressOf(size) => size,
            _ => infer_expr_size(spec, &mut unop.e),
        },
        ExpressionTy::Binop(binop) => {
            if binop.op.is_comparison() {
                Some(1)
            } else {
                infer_expr_size(spec, &mut binop.lhs)
            }
        }
    };
    expr.size = size;
    size
}

pub(crate) fn ident_size(spec: &crate::spec::Spec, ident: &Ident) -> Option<usize> {
    match ident {
        Ident::Register(id) => Some(spec.registers[*id].size),
        Ident::BitRange(id) => Some(spec.bitranges[*id].size().div_ceil(8)),
        Ident::Named(_) | Ident::Field(_) | Ident::Table(_) | Ident::Global(_) => None,
    }
}
