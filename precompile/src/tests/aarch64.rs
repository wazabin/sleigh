use crate::tests::integration::{assert_ast_eq, decode_ast};

#[test]
fn aarch64_nop() {
    let spec = crate::aarch64::spec();
    let context = spec.new_context();
    let (display, info, ast) = decode_ast(spec, &context, b"\x1f\x20\x03\xd5");
    assert_eq!(display, "nop");
    assert_eq!(info.address, 0x1000);
    assert_eq!(info.length, 4);
    assert!(ast.pretty_print(spec).is_empty());
}

#[test]
fn aarch64_mov_x0_x1() {
    let spec = crate::aarch64::spec();
    let context = spec.new_context();
    let (display, info, ast) = decode_ast(spec, &context, b"\xe0\x03\x01\xaa");
    assert_eq!(display, "mov x0, x1");
    assert_eq!(info.address, 0x1000);
    assert_eq!(info.length, 4);

    let tmp_1 = "v0";

    assert_ast_eq(
        spec,
        &ast,
        &format!(
            "{tmp_1}:8 = x1;
                x0 = {tmp_1};"
        ),
    );
}

#[test]
fn aarch64_add_x0_x1_x2() {
    let spec = crate::aarch64::spec();
    let context = spec.new_context();
    let (display, info, ast) = decode_ast(spec, &context, b"\x20\x00\x02\x8b");
    assert_eq!(display, "add x0, x1, x2");
    assert_eq!(info.address, 0x1000);
    assert_eq!(info.length, 4);

    let tmp_2 = "v0";
    let tmp_cy = "tmpCY";
    let tmp_ov = "tmpOV";
    let tmp_1 = "v1";
    let tmp_ng = "tmpNG";
    let tmp_zr = "tmpZR";

    assert_ast_eq(
        spec,
        &ast,
        &format!(
            "{tmp_2}:8 = x2;
                {tmp_cy} = carry(x1, {tmp_2});
                {tmp_ov} = scarry(x1, {tmp_2});
                {tmp_1}:8 = (x1 + {tmp_2});
                {tmp_ng} = ({tmp_1} s< 0);
                {tmp_zr} = ({tmp_1} == 0);
                x0 = {tmp_1};"
        ),
    );
}

#[test]
fn aarch64_sub_x3_x4_imm32() {
    let spec = crate::aarch64::spec();
    let context = spec.new_context();
    let (display, info, ast) = decode_ast(spec, &context, b"\x83\x80\x00\xd1");
    assert_eq!(display, "sub x3, x4, #32");
    assert_eq!(info.address, 0x1000);
    assert_eq!(info.length, 4);

    let tmp_2 = "v0";
    let tmp_1 = "v1";

    assert_ast_eq(
        spec,
        &ast,
        &format!(
            "{tmp_2}:8 = 32:8;
                {tmp_1}:8 = (x4 - {tmp_2});
                x3 = {tmp_1};"
        ),
    );
}

#[test]
fn aarch64_ldr_x0_x1_8() {
    let spec = crate::aarch64::spec();
    let context = spec.new_context();
    let (display, info, ast) = decode_ast(spec, &context, b"\x20\x04\x40\xf9");
    assert_eq!(display, "ldr x0, [x1, #8]");
    assert_eq!(info.address, 0x1000);
    assert_eq!(info.length, 4);

    let addr_tmp = "v0";

    assert_ast_eq(
        spec,
        &ast,
        &format!(
            "{addr_tmp}:8 = (x1 + 8:8);
                x0 = load(ptr={addr_tmp});"
        ),
    );
}

#[test]
fn aarch64_str_x2_x3_16() {
    let spec = crate::aarch64::spec();
    let context = spec.new_context();
    let (display, info, ast) = decode_ast(spec, &context, b"\x62\x08\x00\xf9");
    assert_eq!(display, "str x2, [x3, #16]");
    assert_eq!(info.address, 0x1000);
    assert_eq!(info.length, 4);

    let addr_tmp = "v0";

    assert_ast_eq(
        spec,
        &ast,
        &format!(
            "{addr_tmp}:8 = (x3 + 16:8);
                load(ptr={addr_tmp}) = x2;"
        ),
    );
}

#[test]
fn aarch64_b_rel16() {
    let spec = crate::aarch64::spec();
    let context = spec.new_context();
    let (display, info, ast) = decode_ast(spec, &context, b"\x04\x00\x00\x14");
    assert_eq!(display, "b 4112");
    assert_eq!(info.address, 0x1000);
    assert_eq!(info.length, 4);

    assert_ast_eq(spec, &ast, "goto 4112:8;");
}

#[test]
fn aarch64_cbz_x0_rel8() {
    let spec = crate::aarch64::spec();
    let context = spec.new_context();
    let (display, info, ast) = decode_ast(spec, &context, b"\x40\x00\x00\xb4");
    assert_eq!(display, "cbz x0, 4104");
    assert_eq!(info.address, 0x1000);
    assert_eq!(info.length, 4);

    assert_ast_eq(spec, &ast, "if (x0 == 0) goto 4104:8;");
}
