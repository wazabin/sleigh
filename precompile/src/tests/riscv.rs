use crate::tests::integration::{assert_ast_eq, decode_ast};

#[test]
fn riscv_nop() {
    let spec = crate::riscv::spec();
    let context = spec.new_context();

    // word: 0x00000013
    // sleigh-decode hex input: 13000000
    let (display, info, _) = decode_ast(spec, &context, b"\x13\x00\x00\x00");

    assert_eq!(display, "nop");
    assert_eq!(info.address, 0x1000);
    assert_eq!(info.length, 4);
}

#[test]
fn riscv_addi_x5_x6_12() {
    let spec = crate::riscv::spec();
    let context = spec.new_context();

    // word: 0x00c30293
    // sleigh-decode hex input: 9302c300
    let (display, info, ast) = decode_ast(spec, &context, b"\x93\x02\xc3\x00");

    assert_eq!(display, "addi t0,t1,12");
    assert_eq!(info.address, 0x1000);
    assert_eq!(info.length, 4);

    let tmp_1 = "v0";

    assert_ast_eq(
        spec,
        &ast,
        &format!(
            "{tmp_1}:8 = 12:8;
                t0 = (t1 + {tmp_1});"
        ),
    );
}

#[test]
fn riscv_add_x5_x6_x7() {
    let spec = crate::riscv::spec();
    let context = spec.new_context();

    // word: 0x007302b3
    // sleigh-decode hex input: b3027300
    let (display, info, ast) = decode_ast(spec, &context, b"\xb3\x02\x73\x00");

    assert_eq!(display, "add t0,t1,t2");
    assert_eq!(info.address, 0x1000);
    assert_eq!(info.length, 4);

    assert_ast_eq(spec, &ast, "t0 = (t1 + t2);");
}

#[test]
fn riscv_sub_x5_x6_x7() {
    let spec = crate::riscv::spec();
    let context = spec.new_context();

    // word: 0x407302b3
    // sleigh-decode hex input: b3027340
    let (display, info, ast) = decode_ast(spec, &context, b"\xb3\x02\x73\x40");

    assert_eq!(display, "sub t0,t1,t2");
    assert_eq!(info.address, 0x1000);
    assert_eq!(info.length, 4);

    assert_ast_eq(spec, &ast, "t0 = (t1 - t2);");
}

#[test]
fn riscv_ld_x5_x6_16() {
    let spec = crate::riscv::spec();
    let context = spec.new_context();

    // word: 0x01033283
    // sleigh-decode hex input: 83320301
    let (display, info, ast) = decode_ast(spec, &context, b"\x83\x32\x03\x01");

    assert_eq!(display, "ld t0,16(t1)");
    assert_eq!(info.address, 0x1000);
    assert_eq!(info.length, 4);

    let v0 = "v0";
    let v1 = "v1";

    assert_ast_eq(
        spec,
        &ast,
        &format!(
            "{v1}:8 = 16:8;
                {v0}:8 = (t1 + {v1});
                t0 = load(space=ram, size=8, ptr={v0});"
        ),
    );
}

#[test]
fn riscv_sd_x5_x6_24() {
    let spec = crate::riscv::spec();
    let context = spec.new_context();

    // word: 0x00533c23
    // sleigh-decode hex input: 233c5300
    let (display, info, ast) = decode_ast(spec, &context, b"\x23\x3c\x53\x00");

    assert_eq!(display, "sd t0,24(t1)");
    assert_eq!(info.address, 0x1000);
    assert_eq!(info.length, 4);

    let v1 = "v1";
    let v0 = "v0";

    assert_ast_eq(
        spec,
        &ast,
        &format!(
            "{v1}:8 = 24:8;
                {v0}:8 = (t1 + {v1});
                load(space=ram, size=8, ptr={v0}) = t0;"
        ),
    );
}

#[test]
fn riscv_jal_x0_rel16() {
    let spec = crate::riscv::spec();
    let context = spec.new_context();

    // word: 0x0100006f
    // sleigh-decode hex input: 6f000001
    let (display, info, ast) = decode_ast(spec, &context, b"\x6f\x00\x00\x01");

    assert_eq!(display, "j 4112");
    assert_eq!(info.address, 0x1000);
    assert_eq!(info.length, 4);

    assert_ast_eq(spec, &ast, "goto 4112:8;");
}

#[test]
fn riscv_beq_x5_x6_rel8() {
    let spec = crate::riscv::spec();
    let context = spec.new_context();

    // word: 0x00628463
    // sleigh-decode hex input: 63846200
    let (display, info, ast) = decode_ast(spec, &context, b"\x63\x84\x62\x00");

    assert_eq!(display, "beq t0,t1,4104");
    assert_eq!(info.address, 0x1000);
    assert_eq!(info.length, 4);

    assert_ast_eq(spec, &ast, "if (t0 == t1) goto 4104:8;");
}

#[test]
fn riscv_bne_x5_x6_rel8() {
    let spec = crate::riscv::spec();
    let context = spec.new_context();

    // word: 0x00629463
    // sleigh-decode hex input: 63946200
    let (display, info, ast) = decode_ast(spec, &context, b"\x63\x94\x62\x00");

    assert_eq!(display, "bne t0,t1,4104");
    assert_eq!(info.address, 0x1000);
    assert_eq!(info.length, 4);

    assert_ast_eq(spec, &ast, "if (t0 != t1) goto 4104:8;");
}

/// The RV64-only compressed instructions live behind
/// `@if (ADDRSIZE == "64") || (ADDRSIZE == "128")`. The preprocessor used to
/// strip the outer parentheses of the whole condition and then split on `||`,
/// leaving two unbalanced halves that failed to evaluate — so the condition
/// silently read as false and every constructor in those blocks was dropped.
#[test]
fn riscv_rv64c_compressed_constructors_are_compiled() {
    let spec = crate::riscv::spec();
    let context = spec.new_context();

    for (bytes, expected) in [
        (b"\x00\x63".as_slice(), "c.ld s0,0(a4)"),
        (b"\x00\xe3".as_slice(), "c.sd s0,0(a4)"),
        (b"\x05\x27".as_slice(), "c.addiw a4,1"),
    ] {
        let (display, info, _) = decode_ast(spec, &context, bytes);
        assert_eq!(display, expected);
        assert_eq!(info.length, 2);
    }
}
