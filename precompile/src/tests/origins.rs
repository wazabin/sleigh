//! Constant origins on real specifications: which constants of the lifted
//! p-code are instruction-relative, which are absolute, and which are mixed.

use sleigh::{CompiledSpec, ConstantOrigin, Decoder, Opcode};

const ADDRESS: u64 = 0x1000;

/// Every constant of the instruction at [`ADDRESS`] as
/// `(opcode, value, origin)`, after checking its disassembly.
fn constants(
    spec: &CompiledSpec,
    bytes: &[u8],
    display: &str,
) -> Vec<(Opcode, u64, ConstantOrigin)> {
    let instruction = Decoder::new(spec)
        .decode_one(ADDRESS, bytes, &spec.new_context())
        .expect("decodes");
    assert_eq!(instruction.to_string(), display);
    let (pcode, origins) = instruction.pcode_ops_with_origins().expect("lowers");
    origins
        .iter()
        .map(|(op, input, origin)| {
            (
                pcode.ops[op].opcode,
                pcode.ops[op].inputs[input].offset,
                origin,
            )
        })
        .collect()
}

fn rel(offset_from_inst_start: i64) -> ConstantOrigin {
    ConstantOrigin::InstRelative {
        offset_from_inst_start,
    }
}

/// The space-id constant every `LOAD` and `STORE` carries.
fn space_id(constant: &(Opcode, u64, ConstantOrigin)) -> bool {
    matches!(
        constant,
        (Opcode::Load | Opcode::Store, _, ConstantOrigin::Absolute(_))
    )
}

#[test]
fn x64_lea_rip_relative_is_relative() {
    // 0x1000 + 7 + 0x10.
    let constants = constants(
        crate::x64::spec(),
        b"\x48\x8d\x05\x10\x00\x00\x00",
        "LEA RAX,[4119]",
    );
    assert_eq!(constants, [(Opcode::Copy, 0x1017, rel(0x17))]);
}

#[test]
fn x64_mov_rip_relative_load_address_is_relative() {
    let constants = constants(
        crate::x64::spec(),
        b"\x48\x8b\x05\x10\x00\x00\x00",
        "MOV RAX,qword ptr [4119]",
    );
    assert_eq!(constants.len(), 2);
    assert!(space_id(&constants[0]));
    assert_eq!(constants[1], (Opcode::Load, 0x1017, rel(0x17)));
}

#[test]
fn x64_absolute_addresses_are_absolute() {
    // disp32 through a SIB with no base, and a 64-bit moffs.
    for (bytes, display) in [
        (
            &b"\x48\x8b\x04\x25\x00\x30\x40\x00"[..],
            "MOV RAX,qword ptr [4206592]",
        ),
        (
            &b"\x48\xa1\x00\x30\x40\x00\x00\x00\x00\x00"[..],
            "MOV RAX,[4206592]",
        ),
    ] {
        let constants = constants(crate::x64::spec(), bytes, display);
        assert_eq!(constants.len(), 2);
        assert!(space_id(&constants[0]));
        assert_eq!(
            constants[1],
            (Opcode::Load, 0x403000, ConstantOrigin::Absolute(0x403000))
        );
    }
}

#[test]
fn x64_relative_jmp_and_call_targets_are_relative() {
    let jmp = constants(crate::x64::spec(), b"\xe9\x10\x00\x00\x00", "JMP 4117");
    assert_eq!(jmp, [(Opcode::Branch, 0x1015, rel(0x15))]);

    let jz = constants(crate::x64::spec(), b"\x74\x10", "JZ 4114");
    assert_eq!(jz, [(Opcode::CBranch, 0x1012, rel(0x12))]);

    let call = constants(crate::x64::spec(), b"\xe8\x10\x00\x00\x00", "CALL 4117");
    // The pushed return address is `inst_next`; the stack adjustment is not
    // an address.
    assert!(call.contains(&(Opcode::Copy, 0x1005, rel(5))), "{call:?}");
    assert!(
        call.contains(&(Opcode::IntSub, 8, ConstantOrigin::Absolute(8))),
        "{call:?}"
    );
    assert!(
        call.contains(&(Opcode::Call, 0x1015, rel(0x15))),
        "{call:?}"
    );
}

#[test]
fn x64_immediates_are_absolute() {
    let constants = constants(crate::x64::spec(), b"\xb8\x34\x12\x00\x00", "MOV EAX,4660");
    assert_eq!(
        constants,
        [(Opcode::Copy, 0x1234, ConstantOrigin::Absolute(0x1234))]
    );
}

#[test]
fn x64_eip_relative_is_mixed() {
    // `addr32`: `(inst_next + simm32) & 0xffffffff` truncates the address.
    let constants = constants(
        crate::x64::spec(),
        b"\x67\x48\x8d\x05\x10\x00\x00\x00",
        "LEA RAX,[4120]",
    );
    assert_eq!(
        constants,
        [(Opcode::Copy, 0x1018, ConstantOrigin::Mixed(0x1018))]
    );
}

#[test]
fn x64_rep_loops_back_to_its_own_address() {
    let constants = constants(crate::x64::spec(), b"\xf3\xa4", "MOVSB.REP RDI,RSI");
    assert!(
        constants.contains(&(Opcode::CBranch, 0x1002, rel(2))),
        "{constants:?}"
    );
    assert!(
        constants.contains(&(Opcode::Branch, 0x1000, rel(0))),
        "{constants:?}"
    );
    assert!(
        constants
            .iter()
            .filter(|(op, ..)| !matches!(op, Opcode::Branch | Opcode::CBranch))
            .all(|(.., origin)| !origin.depends_on_address()),
        "{constants:?}"
    );
}

#[test]
fn aarch64_pc_relative_loads_are_relative() {
    let spec = crate::aarch64::spec();
    let adr = constants(spec, b"\x80\x00\x00\x10", "adr x0, 4112");
    assert_eq!(adr, [(Opcode::Copy, 0x1010, rel(0x10))]);

    let ldr = constants(spec, b"\x80\x00\x00\x58", "ldr x0, 4112");
    assert_eq!(ldr.len(), 2);
    assert!(space_id(&ldr[0]));
    assert_eq!(ldr[1], (Opcode::Load, 0x1010, rel(0x10)));

    let bl = constants(spec, b"\x04\x00\x00\x94", "bl 4112");
    assert!(bl.contains(&(Opcode::Call, 0x1010, rel(0x10))), "{bl:?}");
}

#[test]
fn aarch64_adrp_page_is_mixed() {
    // `(inst_start & ~0xfff) + (imm << 12)`.
    let adrp = constants(crate::aarch64::spec(), b"\x00\x00\x00\xb0", "adrp x0, 8192");
    assert_eq!(
        adrp,
        [(Opcode::Copy, 0x2000, ConstantOrigin::Mixed(0x2000))]
    );
}
