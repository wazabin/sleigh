use crate::tests::integration::{assert_ast_eq, decode_ast};

#[test]
fn x86_push_ebp() {
    let spec = crate::x86::spec();
    let context = spec.new_context();
    let (display, info, ast) = decode_ast(spec, &context, b"\x55");
    assert_eq!(display, "PUSH EBP");
    assert_eq!(info.address, 0x1000);
    assert_eq!(info.length, 1);

    let v1 = "v1";

    assert_ast_eq(
        spec,
        &ast,
        &format!(
            "{v1}:4 = EBP;
                ESP = (ESP - 4);
                load(size=4, ptr=ESP) = {v1};"
        ),
    );
}

#[test]
fn x86_mov_eax_ptr_ecx() {
    let spec = crate::x86::spec();
    let context = spec.new_context();
    let (display, info, ast) = decode_ast(spec, &context, b"\x8b\x01");
    assert_eq!(display, "MOV EAX,dword ptr [ECX]");
    assert_eq!(info.address, 0x1000);
    assert_eq!(info.length, 2);

    assert_ast_eq(
        spec,
        &ast,
        r#"v0 = ECX;
            EAX = load(space=ram, size=4, ptr=v0);"#,
    );
}

#[test]
fn x86_mov_eax_imm32() {
    let spec = crate::x86::spec();
    let context = spec.new_context();
    let (display, info, ast) = decode_ast(spec, &context, b"\xb8\x78\x56\x34\x12");
    assert_eq!(display, "MOV EAX,305419896");
    assert_eq!(info.address, 0x1000);
    assert_eq!(info.length, 5);

    assert_ast_eq(spec, &ast, "EAX = 305419896:4;");
}

#[test]
fn x86_mov_ptr_eax_ecx() {
    let spec = crate::x86::spec();
    let context = spec.new_context();
    let (display, info, ast) = decode_ast(spec, &context, b"\x89\x08");
    assert_eq!(display, "MOV dword ptr [EAX],ECX");
    assert_eq!(info.address, 0x1000);
    assert_eq!(info.length, 2);

    assert_ast_eq(
        spec,
        &ast,
        r#"v0 = EAX;
            load(space=ram, size=4, ptr=v0):4 = ECX;"#,
    );
}

#[test]
fn x86_mov_eax_ebp_minus4() {
    let spec = crate::x86::spec();
    let context = spec.new_context();
    let (display, info, ast) = decode_ast(spec, &context, b"\x8b\x45\xfc");
    assert_eq!(display, "MOV EAX,dword ptr [EBP + -4]");
    assert_eq!(info.address, 0x1000);
    assert_eq!(info.length, 3);

    let v1 = "v0";

    assert_ast_eq(
        spec,
        &ast,
        &format!(
            "{v1} = (EBP + 18446744073709551612:4);
                EAX = load(space=ram, size=4, ptr={v1});"
        ),
    );
}

#[test]
fn x86_ret() {
    let spec = crate::x86::spec();
    let context = spec.new_context();
    let (display, info, ast) = decode_ast(spec, &context, b"\xc3");
    assert_eq!(display, "RET");
    assert_eq!(info.address, 0x1000);
    assert_eq!(info.length, 1);

    assert_ast_eq(
        spec,
        &ast,
        r#"v1:4 = load(size=4, ptr=ESP);
            ESP = (ESP + 4);
            EIP = v1;
            return [EIP];"#,
    );
}

#[test]
fn x86_add_eax_ebx() {
    let spec = crate::x86::spec();
    let context = spec.new_context();
    let (display, info, ast) = decode_ast(spec, &context, b"\x01\xd8");
    assert_eq!(display, "ADD EAX,EBX");
    assert_eq!(info.address, 0x1000);
    assert_eq!(info.length, 2);

    assert_ast_eq(
        spec,
        &ast,
        r#"CF = carry(EAX, EBX);
            OF = scarry(EAX, EBX);
            v2 = (EAX + EBX);
            AF = ((((EAX ^ EBX) ^ v2) & 16) != 0);
            EAX = (EAX + EBX);
            SF = (EAX s< 0);
            ZF = (EAX == 0);
            PF = ((popcount((EAX & 255)) & 1:1) == 0);"#,
    );
}

#[test]
fn x86_sub_edx_imm8() {
    let spec = crate::x86::spec();
    let context = spec.new_context();
    let (display, info, ast) = decode_ast(spec, &context, b"\x83\xea\x05");
    assert_eq!(display, "SUB EDX,5");
    assert_eq!(info.address, 0x1000);
    assert_eq!(info.length, 3);

    assert_ast_eq(
        spec,
        &ast,
        r#"CF = (EDX < 5:4);
            OF = sborrow(EDX, 5:4);
            AF = ((EDX & 15) < (5:4 & 15));
            EDX = (EDX - 5:4);
            SF = (EDX s< 0);
            ZF = (EDX == 0);
            PF = ((popcount((EDX & 255)) & 1:1) == 0);"#,
    );
}

#[test]
fn x86_xor_al_imm8() {
    let spec = crate::x86::spec();
    let context = spec.new_context();
    let (display, info, ast) = decode_ast(spec, &context, b"\x34\x12");
    assert_eq!(display, "XOR AL,18");
    assert_eq!(info.address, 0x1000);
    assert_eq!(info.length, 2);

    assert_ast_eq(
        spec,
        &ast,
        r#"CF = 0;
            OF = 0;
            AL = (AL ^ 18:1);
            SF = (AL s< 0);
            ZF = (AL == 0);
            PF = ((popcount((AL & 255)) & 1:1) == 0);"#,
    );
}

#[test]
fn x86_cmp_ecx_edx() {
    let spec = crate::x86::spec();
    let context = spec.new_context();
    let (display, info, ast) = decode_ast(spec, &context, b"\x39\xd1");
    assert_eq!(display, "CMP ECX,EDX");
    assert_eq!(info.address, 0x1000);
    assert_eq!(info.length, 2);

    let v1 = "v0";
    let v2 = "v1";

    assert_ast_eq(
        spec,
        &ast,
        &format!(
            "{v1}:4 = ECX;
                CF = ({v1} < EDX);
                OF = sborrow({v1}, EDX);
                AF = (({v1} & 15) < (EDX & 15));
                {v2} = ({v1} - EDX);
                SF = ({v2} s< 0);
                ZF = ({v2} == 0);
                PF = ((popcount(({v2} & 255)) & 1:1) == 0);"
        ),
    );
}

#[test]
fn x86_imul_esi_edi() {
    let spec = crate::x86::spec();
    let context = spec.new_context();
    let (display, info, ast) = decode_ast(spec, &context, b"\x0f\xaf\xf7");
    assert_eq!(display, "IMUL ESI,EDI");
    assert_eq!(info.address, 0x1000);
    assert_eq!(info.length, 3);

    let v1 = "v0";
    let v2 = "v1";

    assert_ast_eq(
        spec,
        &ast,
        &format!(
            "{v1}:8 = (sext(ESI) * sext(EDI));
                ESI = subpiece_msb({v1}, 0);
                {v2}:4 = subpiece_msb({v1}, 4);
                CF = (sext(ESI) != {v1});
                OF = CF;",
        ),
    );
}

#[test]
fn x86_cdq() {
    let spec = crate::x86::spec();
    let context = spec.new_context();
    let (display, info, ast) = decode_ast(spec, &context, b"\x99");
    assert_eq!(display, "CDQ");
    assert_eq!(info.address, 0x1000);
    assert_eq!(info.length, 1);

    let v1 = "v0";

    assert_ast_eq(
        spec,
        &ast,
        &format!(
            "{v1}:8 = sext(EAX);
                EDX = subpiece_msb({v1}, 4);"
        ),
    );
}

#[test]
fn x86_jz_rel8() {
    let spec = crate::x86::spec();
    let context = spec.new_context();
    let (display, info, ast) = decode_ast(spec, &context, b"\x74\x05");
    assert_eq!(display, "JZ 4103");
    assert_eq!(info.address, 0x1000);
    assert_eq!(info.length, 2);

    assert_ast_eq(spec, &ast, "if ZF goto 4103:4;");
}

#[test]
fn x86_call_rel32() {
    let spec = crate::x86::spec();
    let context = spec.new_context();
    let (display, info, ast) = decode_ast(spec, &context, b"\xe8\x78\x56\x34\x12");
    assert_eq!(display, "CALL 305423997");
    assert_eq!(info.address, 0x1000);
    assert_eq!(info.length, 5);

    let v1 = "v1";

    assert_ast_eq(
        spec,
        &ast,
        &format!(
            "{v1}:4 = &:4 4101:4;
                ESP = (ESP - 4);
                load(size=4, ptr=ESP) = {v1};
                call 305423997:4;"
        ),
    );
}
