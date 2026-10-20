//! [`Instruction::pcode_ops_with_origins`] on every way an instruction's
//! address reaches a constant: a disassembly-action global exported through
//! an address or a constant, a branch target, `inst_start`/`inst_next`/
//! `inst_next2` read in the semantic section, context an action assigns
//! from the address, and a delay-slot instruction's own address.

use crate::{
    CompiledSpec, Compiler, ConstantOrigin, Decoder, Instruction, Opcode, PcodeOp, PcodeSink,
    SourceDb, Varnode,
};

const SPEC: &str = r#"
define endian=little;
define space ram type=ram_space size=4 default;
define space register type=register_space size=4;
define register offset=0 size=4 [ r0 r1 ];
define register offset=0x100 size=4 [ ctxreg ];
define context ctxreg ctxf=(0,7);

define token instr(16) op=(0,7) imm=(8,15) simm=(8,15) signed;

# `inst_next + simm`, read through memory: x86-64's RIP-relative operand.
rel: reloc is simm [ reloc = inst_next + simm; ] { export *[ram]:4 reloc; }
# `(inst_start & ~0xfff) + (imm << 12)`: AArch64's `adrp` page.
page: base is imm [ base = (inst_start & 0xfffff000) + (imm << 12); ] { export *[const]:4 base; }
# The difference of two addresses is a number.
len: d is imm [ d = inst_next - inst_start; ] { export *[const]:4 d; }
# Reads context, as whatever the parent's action left it.
sub: ctxf is ctxf { r1 = ctxf; }

:ld rel     is op=1 & rel    { r0 = rel; }
:jmp rel    is op=2 & rel    { goto rel; }
:call rel   is op=3 & rel    { *:4 r1 = inst_next; call rel; }
:adrp page  is op=4 & page   { r0 = page; }
:len len    is op=5 & len    { r0 = len; }
:mov imm    is op=6 & imm    { r0 = imm; }
:setctx sub is op=7 & sub [ ctxf = inst_start; ] { build sub; }
:getctx sub is op=8 & sub    { build sub; }
:ds         is op=9          { delayslot(2); }
:skip       is op=10         { if (r0 == 0) goto inst_next2; r0 = inst_start; }
:same       is op=11 & imm   { r0 = 0x1002; r1 = inst_next; }
"#;

fn spec() -> &'static CompiledSpec {
    use std::sync::OnceLock;
    static SPEC_CELL: OnceLock<CompiledSpec> = OnceLock::new();
    SPEC_CELL.get_or_init(|| {
        let mut sources = SourceDb::new();
        let root = sources.add_file("origins.sla", SPEC);
        Compiler::new(&mut sources)
            .compile(root)
            .unwrap_or_else(|error| panic!("origins.sla compiles: {error:?}"))
    })
}

fn decode(bytes: &[u8]) -> Instruction<'static, '_> {
    let spec = spec();
    Decoder::new(spec)
        .decode_one(0x1000, bytes, &spec.new_context())
        .expect("decodes")
}

/// Every constant of the instruction as `(opcode, value, origin)`.
fn constants(bytes: &[u8]) -> Vec<(Opcode, u64, ConstantOrigin)> {
    let instruction = decode(bytes);
    let (pcode, origins) = instruction.pcode_ops_with_origins().expect("lowers");
    assert_eq!(origins.len(), pcode.ops.len());
    assert_eq!(origins, instruction.constant_origins().expect("lowers"));
    origins
        .iter()
        .map(|(op, input, origin)| {
            let op_ = &pcode.ops[op];
            (op_.opcode, op_.inputs[input].offset, origin)
        })
        .collect()
}

fn rel(offset_from_inst_start: i64) -> ConstantOrigin {
    ConstantOrigin::InstRelative {
        offset_from_inst_start,
    }
}

#[test]
fn a_rip_relative_load_address_is_relative() {
    // 0x1002 + 0x10. The load's first input is the space id: absolute.
    let constants = constants(&[1, 0x10]);
    assert_eq!(constants.len(), 2, "{constants:?}");
    assert_eq!(constants[0].0, Opcode::Load);
    assert!(matches!(constants[0].2, ConstantOrigin::Absolute(_)));
    assert_eq!(constants[1], (Opcode::Load, 0x1012, rel(0x12)));

    // A negative displacement: 0x1002 - 2.
    assert_eq!(constants_of_op(&[1, 0xfe], Opcode::Load)[1], rel(0));
}

fn constants_of_op(bytes: &[u8], opcode: Opcode) -> Vec<ConstantOrigin> {
    constants(bytes)
        .into_iter()
        .filter(|(op, ..)| *op == opcode)
        .map(|(.., origin)| origin)
        .collect()
}

#[test]
fn relative_branch_and_call_targets_are_relative() {
    assert_eq!(constants(&[2, 0x20]), [(Opcode::Branch, 0x1022, rel(0x22))]);

    let call = constants(&[3, 0x20]);
    // The pushed return address is `inst_next`, read in the semantic section.
    assert!(
        call.contains(&(Opcode::Copy, 0x1002, rel(2)))
            || call.contains(&(Opcode::Store, 0x1002, rel(2))),
        "{call:?}"
    );
    assert!(
        call.contains(&(Opcode::Call, 0x1022, rel(0x22))),
        "{call:?}"
    );
}

#[test]
fn a_masked_address_is_mixed_never_absolute() {
    // (0x1000 & 0xfffff000) + (3 << 12).
    assert_eq!(
        constants(&[4, 3]),
        [(Opcode::Copy, 0x4000, ConstantOrigin::Mixed(0x4000))]
    );
}

#[test]
fn the_difference_of_two_addresses_is_absolute() {
    assert_eq!(
        constants(&[5, 0]),
        [(Opcode::Copy, 2, ConstantOrigin::Absolute(2))]
    );
}

#[test]
fn an_immediate_is_absolute() {
    assert_eq!(
        constants(&[6, 0x34]),
        [(Opcode::Copy, 0x34, ConstantOrigin::Absolute(0x34))]
    );
}

#[test]
fn an_immediate_equal_to_an_address_is_still_absolute() {
    // `0x1002` and `inst_next` are the same number; only one is relative.
    assert_eq!(
        constants(&[11, 0]),
        [
            (Opcode::Copy, 0x1002, ConstantOrigin::Absolute(0x1002)),
            (Opcode::Copy, 0x1002, rel(2)),
        ]
    );
}

#[test]
fn context_an_action_assigns_from_the_address_is_mixed() {
    // `ctxf = inst_start` truncates 0x1000 to its low byte.
    assert_eq!(
        constants(&[7, 0]),
        [(Opcode::Copy, 0, ConstantOrigin::Mixed(0))]
    );
    // The caller's context is plain state.
    assert_eq!(
        constants(&[8, 0]),
        [(Opcode::Copy, 0, ConstantOrigin::Absolute(0))]
    );
}

#[test]
fn a_delay_slot_instruction_is_relative_to_the_one_delaying_it() {
    // `ds` at 0x1000, then `ld` at 0x1002 loading from 0x1004 + 0x10.
    assert_eq!(
        constants_of_op(&[9, 0, 1, 0x10], Opcode::Load)[1],
        rel(0x14)
    );
}

#[test]
fn inst_next2_is_relative() {
    let constants = constants(&[10, 0, 6, 0]);
    assert!(
        constants.contains(&(Opcode::CBranch, 0x1004, rel(4))),
        "{constants:?}"
    );
    assert!(
        constants.contains(&(Opcode::Copy, 0x1000, rel(0))),
        "{constants:?}"
    );
}

/// Counts what the streamed lowering hands its sink.
#[derive(Default)]
struct Count(usize);

impl PcodeSink for Count {
    fn op(&mut self, _: Opcode, _: Option<Varnode>, _: &[Varnode]) {
        self.0 += 1;
    }
    fn label(&mut self, _: crate::LabelId) {}
    fn branch_label(&mut self, _: Opcode, _: crate::LabelId, _: Option<Varnode>) {
        self.0 += 1;
    }
}

#[test]
fn origins_are_indexed_like_the_streamed_operations() {
    for bytes in [
        [1u8, 0x10, 0, 0],
        [3, 0x20, 0, 0],
        [10, 0, 6, 0],
        [9, 0, 1, 0x10],
    ] {
        let instruction = decode(&bytes);
        let streamed = instruction
            .pcode_ops_streamed(|_| Count::default())
            .expect("streams");
        let (pcode, origins) = instruction.pcode_ops_with_origins().expect("lowers");
        assert_eq!(streamed.0, origins.len());
        for (index, op) in pcode.ops.iter().enumerate() {
            let op: &PcodeOp = op;
            assert_eq!(origins.op(index).len(), op.inputs.len());
            for (input, varnode) in op.inputs.iter().enumerate() {
                if varnode.is_constant() {
                    assert!(origins.get(index, input).is_some());
                }
            }
        }
    }
}
