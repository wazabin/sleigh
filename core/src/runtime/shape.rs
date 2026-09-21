//! The shape of a decoded instruction: which of its bits chose the
//! constructors, and which are plain operand values.
//!
//! A decode reads the instruction's bits for two purposes. Some decide what
//! the instruction *is*: the bits the decision trees dispatch on, the bits
//! the tried constructors' patterns test, and the bits of fields whose value
//! picks a register, an attached value or a context assignment. The rest are
//! read only to become integers in the p-code — immediates, displacements,
//! branch offsets. Two encodings that agree on the first kind decode to the
//! same constructors, in the same order, with the same operand layout; they
//! differ only in the values of the second kind.
//!
//! [`Decoder::decode_one_shaped`](super::Decoder::decode_one_shaped) records
//! that split as a [`Shape`]: a bit mask over the instruction's bytes (set
//! where the bit was decisive), the patterns of the more specific
//! candidates the decode passed over — an encoding matching one of them
//! decodes differently, whatever its masked bits say — and the list of
//! parameter fields. A caller memoizing per-encoding work can key it on the
//! masked bytes, check the exclusions, and parameterize it on the fields,
//! and serve every encoding of the shape from one entry.

use std::cell::{Cell, RefCell};

use crate::bitrange::BitRange;

use super::walker::{extract_bytes, signed};

/// One parameter field of a [`Shape`]: a run of instruction bits read only
/// for its integer value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ParamField {
    /// The field's lowest bit, LSB-numbered from the instruction's first
    /// byte: bit `n` is bit `n % 8` of byte `n / 8`.
    pub bit: u32,
    /// The field's width in bits.
    pub width: u8,
    /// Whether the field's value is sign-extended.
    pub signed: bool,
}

impl ParamField {
    fn range(self) -> BitRange {
        let start = self.bit as usize;
        BitRange::new(start, start + usize::from(self.width) - 1)
    }

    /// The field's value in `bytes`, an instruction of the shape.
    pub fn value(self, bytes: &[u8]) -> i64 {
        let range = self.range();
        let raw = extract_bytes(bytes, &range);
        if self.signed {
            signed(raw, &range)
        } else {
            raw as i64
        }
    }
}

/// A candidate pattern the decode passed over: it would have been taken had
/// it matched, so no encoding of the shape matches it.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Exclusion {
    /// The instruction byte the pattern starts at.
    pub offset: usize,
    /// Per-byte masks from `offset`.
    pub mask: Box<[u8]>,
    /// What the masked bits would have to equal.
    pub value: Box<[u8]>,
}

impl Exclusion {
    /// Whether `bytes`, an instruction, matches the pattern.
    pub fn matches(&self, bytes: &[u8]) -> bool {
        bytes
            .get(self.offset..self.offset + self.mask.len())
            .is_some_and(|bytes| {
                bytes
                    .iter()
                    .zip(self.mask.iter().zip(self.value.iter()))
                    .all(|(byte, (mask, value))| byte & mask == *value)
            })
    }
}

/// Which bits of an instruction chose its constructors, which patterns it
/// escaped, and which fields are its parameters. See the [module
/// documentation](self).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Shape {
    len: usize,
    mask: Box<[u8]>,
    exclusions: Box<[Exclusion]>,
    params: Box<[ParamField]>,
    overrun: bool,
}

impl Shape {
    /// The instruction's length in bytes.
    pub fn len(&self) -> usize {
        self.len
    }

    /// Whether the instruction is empty.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// One byte per instruction byte; a set bit chose the constructors.
    pub fn mask(&self) -> &[u8] {
        &self.mask
    }

    /// The patterns of more specific candidates the decode passed over, in
    /// the order it tried them. No encoding of the shape matches any. A
    /// pattern may reach past the instruction's own bytes — the decoder
    /// tested it against what followed — so they are tested against the
    /// stream, not the instruction.
    pub fn exclusions(&self) -> &[Exclusion] {
        &self.exclusions
    }

    /// The parameter fields, in the order the decoder read them. A field
    /// may overlap the mask — a decision tree can dispatch on one bit of an
    /// immediate — and those bits are then the same in every instruction of
    /// the shape.
    pub fn params(&self) -> &[ParamField] {
        &self.params
    }

    /// Whether `bytes`, a stream whose first [`len`](Self::len) bytes agree
    /// with an instruction of this shape on the mask, start an instruction
    /// of this shape: they match no exclusion. A stream too short for an
    /// exclusion to be tested does not match it, as it would not for the
    /// decoder.
    pub fn admits(&self, bytes: &[u8]) -> bool {
        bytes.len() >= self.len && !self.exclusions.iter().any(|e| e.matches(bytes))
    }

    /// Whether the decode depended on instructions past this one — a delay
    /// slot was filled, or `inst_next2` measured. The shape then does not
    /// describe what chose the constructors.
    pub fn overruns(&self) -> bool {
        self.overrun
    }

    /// `bytes` with every bit outside the mask cleared, into `out`, which
    /// must hold [`len`](Self::len) bytes.
    pub fn masked(&self, bytes: &[u8], out: &mut [u8]) {
        for ((out, byte), mask) in out.iter_mut().zip(bytes).zip(&self.mask) {
            *out = byte & mask;
        }
    }
}

/// Accumulates a [`Shape`] while a walker decodes.
#[derive(Default)]
pub(crate) struct ShapeRecorder {
    /// Bits chosen so far, growing as the walker reads further.
    mask: RefCell<Vec<u8>>,
    exclusions: RefCell<Vec<Exclusion>>,
    params: RefCell<Vec<ParamField>>,
    overrun: Cell<bool>,
}

impl ShapeRecorder {
    /// Marks the bits of `range`, numbered from the instruction's first byte.
    pub(crate) fn note_bits(&self, range: &BitRange) {
        let mut mask = self.mask.borrow_mut();
        let last = range.end() / 8;
        if mask.len() <= last {
            mask.resize(last + 1, 0);
        }
        for bit in range.iter() {
            mask[bit / 8] |= 1 << (bit % 8);
        }
    }

    /// Marks the bits of `bytes`, a per-byte mask starting at byte `offset`
    /// of the instruction.
    pub(crate) fn note_mask(&self, offset: usize, bytes: &[u8]) {
        let mut mask = self.mask.borrow_mut();
        if mask.len() < offset + bytes.len() {
            mask.resize(offset + bytes.len(), 0);
        }
        for (slot, byte) in mask[offset..].iter_mut().zip(bytes) {
            *slot |= byte;
        }
    }

    /// Records a candidate pattern at `offset` of the instruction that was
    /// not taken: it failed at `byte` of `mask`, or the bytes ran out
    /// before it could be tested. When the mask so far already fixes the
    /// bits it failed on, every encoding of the shape fails it the same way
    /// and nothing is kept.
    pub(crate) fn note_exclusion(
        &self,
        offset: usize,
        byte: Option<usize>,
        mask: &[u8],
        value: &[u8],
    ) {
        let fixed = byte.is_some_and(|byte| {
            self.mask
                .borrow()
                .get(offset + byte)
                .is_some_and(|fixed| fixed & mask[byte] == mask[byte])
        });
        if fixed {
            return;
        }
        let exclusion = Exclusion {
            offset,
            mask: mask.into(),
            value: value.into(),
        };
        let mut exclusions = self.exclusions.borrow_mut();
        if !exclusions.contains(&exclusion) {
            exclusions.push(exclusion);
        }
    }

    /// Records a field read for its value alone.
    pub(crate) fn note_param(&self, range: &BitRange, signed: bool) {
        let width = range.size();
        // A field wider than a `u8` counts or a `u64` holds is not a
        // parameter a caller can carry; keep it in the mask instead.
        if width == 0 || width > 64 {
            self.note_bits(range);
            return;
        }
        self.params.borrow_mut().push(ParamField {
            bit: range.start() as u32,
            width: width as u8,
            signed,
        });
    }

    /// Records that the decode depended on bytes past the instruction.
    pub(crate) fn note_overrun(&self) {
        self.overrun.set(true);
    }

    /// The shape of the instruction of `len` bytes the walker decoded.
    pub(crate) fn finish(self, len: usize) -> Shape {
        let mut mask = self.mask.into_inner();
        let mut overrun = self.overrun.get();
        if mask.len() > len {
            overrun |= mask[len..].iter().any(|byte| *byte != 0);
            mask.truncate(len);
        }
        mask.resize(len, 0);
        let mut params = self.params.into_inner();
        // A parameter past the end is a decode the walker abandoned; it
        // decides nothing.
        params.retain(|param| (param.bit as usize + usize::from(param.width)).div_ceil(8) <= len);
        let exclusions = self.exclusions.into_inner();
        Shape {
            len,
            mask: mask.into_boxed_slice(),
            exclusions: exclusions.into_boxed_slice(),
            params: params.into_boxed_slice(),
            overrun,
        }
    }
}
