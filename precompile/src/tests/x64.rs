use crate::tests::integration::{assert_ast_eq, decode_ast};

#[test]
fn x64_mov_dword_rbp_minus4_edi() {
    let spec = crate::x64::spec();
    let context = spec.new_context();
    let (display, info, ast) = decode_ast(spec, &context, b"\x89\x7d\xfc");
    assert_eq!(display, "MOV dword ptr [RBP + -4],EDI");
    assert_eq!(info.address, 0x1000);
    assert_eq!(info.length, 3);

    let v0 = "v0";

    assert_ast_eq(
        spec,
        &ast,
        &format!(
            "{v0} = (RBP + 18446744073709551612:8);
                load(space=ram, size=4, ptr={v0}):4 = EDI;"
        ),
    );
}

#[test]
fn x64_xor_al_imm8() {
    let spec = crate::x64::spec();
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
            AF = undef();
            AL = (AL ^ 18:1);
            SF = (AL s< 0);
            ZF = (AL == 0);
            PF = ((popcount((AL & 255)) & 1:1) == 0);"#,
    );
}

#[test]
fn x64_out_imm8_includes_trailing_operand_in_length() {
    let spec = crate::x64::spec();
    let context = spec.new_context();
    let (display, info, _) = decode_ast(spec, &context, b"\xe6\x00");
    assert_eq!(display, "OUT 0,AL");
    assert_eq!(info.length, 2);
}

#[test]
fn x64_xor_eax_imm32_includes_trailing_operand_in_length() {
    let spec = crate::x64::spec();
    let context = spec.new_context();
    let (display, info, _) = decode_ast(spec, &context, b"\x35\x00\xff\xff\xff");
    assert_eq!(display, "XOR EAX,4294967040");
    assert_eq!(info.length, 5);
}

#[test]
fn x64_shl_rip_relative_imm8_includes_trailing_operand_in_length() {
    let spec = crate::x64::spec();
    let context = spec.new_context();
    let (display, info, _) = decode_ast(spec, &context, b"\xc1\x25\x00\xf1\x9a\xff\x00");
    assert_eq!(display, "SHL dword ptr [18446744073702932743],0");
    assert_eq!(info.length, 7);
}

#[test]
fn x64_mov_eax_imm32() {
    let spec = crate::x64::spec();
    let context = spec.new_context();
    let (display, info, ast) = decode_ast(spec, &context, b"\xb8\x78\x56\x34\x12");
    assert_eq!(display, "MOV EAX,305419896");
    assert_eq!(info.address, 0x1000);
    assert_eq!(info.length, 5);

    assert_ast_eq(spec, &ast, "RAX = 305419896:4;");
}

#[test]
fn x64_mov_rcx_ptr_rdx() {
    let spec = crate::x64::spec();
    let context = spec.new_context();
    let (display, info, ast) = decode_ast(spec, &context, b"\x48\x8b\x0a");
    assert_eq!(display, "MOV RCX,qword ptr [RDX]");
    assert_eq!(info.address, 0x1000);
    assert_eq!(info.length, 3);

    assert_ast_eq(
        spec,
        &ast,
        r#"v0 = RDX;
            RCX = load(space=ram, size=8, ptr=v0);"#,
    );
}

#[test]
fn x64_mov_ptr_r8_disp_r9() {
    let spec = crate::x64::spec();
    let context = spec.new_context();
    let (display, info, ast) = decode_ast(spec, &context, b"\x4d\x89\x48\x10");
    assert_eq!(display, "MOV qword ptr [R8 + 16],R9");
    assert_eq!(info.address, 0x1000);
    assert_eq!(info.length, 4);

    let v0 = "v0";

    assert_ast_eq(
        spec,
        &ast,
        &format!(
            "{v0} = (R8 + 16:8);
                load(space=ram, size=8, ptr={v0}):8 = R9;"
        ),
    );
}

#[test]
fn x64_mov_r10_imm64() {
    let spec = crate::x64::spec();
    let context = spec.new_context();
    let (display, info, ast) =
        decode_ast(spec, &context, b"\x49\xba\x88\x77\x66\x55\x44\x33\x22\x11");
    assert_eq!(display, "MOV R10,1234605616436508552");
    assert_eq!(info.address, 0x1000);
    assert_eq!(info.length, 10);

    assert_ast_eq(spec, &ast, "R10 = 1234605616436508552:8;");
}

#[test]
fn x64_lea_r11_rip_relative() {
    let spec = crate::x64::spec();
    let context = spec.new_context();
    let (display, info, ast) = decode_ast(spec, &context, b"\x4c\x8d\x1d\x20\x00\x00\x00");
    assert_eq!(display, "LEA R11,[4135]");
    assert_eq!(info.address, 0x1000);
    assert_eq!(info.length, 7);

    assert_ast_eq(spec, &ast, "R11 = 4135:8;");
}

#[test]
fn x64_add_rax_rcx() {
    let spec = crate::x64::spec();
    let context = spec.new_context();
    let (display, info, ast) = decode_ast(spec, &context, b"\x48\x01\xc8");
    assert_eq!(display, "ADD RAX,RCX");
    assert_eq!(info.address, 0x1000);
    assert_eq!(info.length, 3);

    assert_ast_eq(
        spec,
        &ast,
        r#"CF = carry(RAX, RCX);
            OF = scarry(RAX, RCX);
            v2 = (RAX + RCX);
            AF = ((((RAX ^ RCX) ^ v2) & 16) != 0);
            RAX = (RAX + RCX);
            SF = (RAX s< 0);
            ZF = (RAX == 0);
            PF = ((popcount((RAX & 255)) & 1:1) == 0);"#,
    );
}

#[test]
fn x64_sub_rdx_imm8() {
    let spec = crate::x64::spec();
    let context = spec.new_context();
    let (display, info, ast) = decode_ast(spec, &context, b"\x48\x83\xea\x05");
    assert_eq!(display, "SUB RDX,5");
    assert_eq!(info.address, 0x1000);
    assert_eq!(info.length, 4);

    assert_ast_eq(
        spec,
        &ast,
        r#"CF = (RDX < 5:8);
            OF = sborrow(RDX, 5:8);
            AF = ((RDX & 15) < (5:8 & 15));
            RDX = (RDX - 5:8);
            SF = (RDX s< 0);
            ZF = (RDX == 0);
            PF = ((popcount((RDX & 255)) & 1:1) == 0);"#,
    );
}

#[test]
fn x64_imul_rsi_rdi() {
    let spec = crate::x64::spec();
    let context = spec.new_context();
    let (display, info, ast) = decode_ast(spec, &context, b"\x48\x0f\xaf\xf7");
    assert_eq!(display, "IMUL RSI,RDI");
    assert_eq!(info.address, 0x1000);
    assert_eq!(info.length, 4);

    let v0 = "v0";
    let v1 = "v1";

    assert_ast_eq(
        spec,
        &ast,
        &format!(
            "{v0}:16 = (sext(RSI) * sext(RDI));
                RSI = (RSI * RDI);
                {v1}:8 = subpiece_msb({v0}, 8);
                CF = (sext(RSI) != {v0});
                OF = CF;
                AF = undef();
                PF = undef();
                SF = undef();
                ZF = undef();"
        ),
    );
}

#[test]
fn x64_xor_r8_r8() {
    let spec = crate::x64::spec();
    let context = spec.new_context();
    let (display, info, ast) = decode_ast(spec, &context, b"\x4d\x31\xc0");
    assert_eq!(display, "XOR R8,R8");
    assert_eq!(info.address, 0x1000);
    assert_eq!(info.length, 3);

    assert_ast_eq(
        spec,
        &ast,
        r#"CF = 0;
            OF = 0;
            AF = undef();
            R8 = (R8 ^ R8);
            SF = (R8 s< 0);
            ZF = (R8 == 0);
            PF = ((popcount((R8 & 255)) & 1:1) == 0);"#,
    );
}

#[test]
fn x64_cmp_r9_r10() {
    let spec = crate::x64::spec();
    let context = spec.new_context();
    let (display, info, ast) = decode_ast(spec, &context, b"\x4d\x39\xd1");
    assert_eq!(display, "CMP R9,R10");
    assert_eq!(info.address, 0x1000);
    assert_eq!(info.length, 3);

    let v0 = "v0";
    let v1 = "v1";

    assert_ast_eq(
        spec,
        &ast,
        &format!(
            "{v0}:8 = R9;
                CF = ({v0} < R10);
                OF = sborrow({v0}, R10);
                AF = (({v0} & 15) < (R10 & 15));
                {v1} = ({v0} - R10);
                SF = ({v1} s< 0);
                ZF = ({v1} == 0);
                PF = ((popcount(({v1} & 255)) & 1:1) == 0);"
        ),
    );
}

#[test]
fn x64_cdqe() {
    let spec = crate::x64::spec();
    let context = spec.new_context();
    let (display, info, ast) = decode_ast(spec, &context, b"\x48\x98");
    assert_eq!(display, "CDQE");
    assert_eq!(info.address, 0x1000);
    assert_eq!(info.length, 2);

    assert_ast_eq(spec, &ast, "RAX = sext(EAX);");
}

#[test]
fn x64_cqo() {
    let spec = crate::x64::spec();
    let context = spec.new_context();
    let (display, info, ast) = decode_ast(spec, &context, b"\x48\x99");
    assert_eq!(display, "CQO");
    assert_eq!(info.address, 0x1000);
    assert_eq!(info.length, 2);

    let v0 = "v0";

    assert_ast_eq(
        spec,
        &ast,
        &format!(
            "{v0}:16 = sext(RAX);
                RDX = subpiece_msb({v0}, 8);"
        ),
    );
}

#[test]
fn x64_shl_rax_imm() {
    let spec = crate::x64::spec();
    let context = spec.new_context();
    let (display, info, ast) = decode_ast(spec, &context, b"\x48\xc1\xe0\x03");
    assert_eq!(display, "SHL RAX,3");
    assert_eq!(info.address, 0x1000);
    assert_eq!(info.length, 4);

    assert_ast_eq(
        spec,
        &ast,
        r#"v0 = (3:1 & 63);
            v1 = RAX;
            RAX = (RAX << v0);
            v5 = (v0 != 0);
            v6 = ((v1 << (v0 - 1)) s< 0);
            CF = ((!v5 && CF) || (v5 && v6));
            v7 = (v0 == 1);
            v8 = ((v0 != 0) && !v7);
            v9 = (CF ^^ (RAX s< 0));
            v10 = OF;
            OF = undef();
            OF = ((((!v7 && !v8) && v10) || (v7 && v9)) || (v8 && OF));
            v13 = (v0 != 0);
            v14 = (RAX s< 0);
            SF = ((!v13 && SF) || (v13 && v14));
            v15 = (RAX == 0);
            ZF = ((!v13 && ZF) || (v13 && v15));
            v16 = ((popcount((RAX & 255)) & 1:1) == 0);
            PF = ((!v13 && PF) || (v13 && v16));
            v17 = AF;
            AF = undef();
            AF = ((!v13 && v17) || (v13 && AF));"#,
    );
}

#[test]
fn x64_sar_rcx_cl() {
    let spec = crate::x64::spec();
    let context = spec.new_context();
    let (display, info, ast) = decode_ast(spec, &context, b"\x48\xd3\xf9");
    assert_eq!(display, "SAR RCX,CL");
    assert_eq!(info.address, 0x1000);
    assert_eq!(info.length, 3);

    assert_ast_eq(
        spec,
        &ast,
        r#"v0 = (CL & 63);
            v1 = RCX;
            RCX = (RCX s>> v0);
            v5 = (v0 != 0);
            v6 = (((v1 s>> (v0 - 1)) & 1) != 0);
            CF = ((!v5 && CF) || (v5 && v6));
            v7 = (v0 == 1);
            v8 = ((v0 != 0) && !v7);
            v9 = OF;
            OF = undef();
            OF = ((((!v7 && !v8) && v9) || (v7 && 0)) || (v8 && OF));
            v12 = (v0 != 0);
            v13 = (RCX s< 0);
            SF = ((!v12 && SF) || (v12 && v13));
            v14 = (RCX == 0);
            ZF = ((!v12 && ZF) || (v12 && v14));
            v15 = ((popcount((RCX & 255)) & 1:1) == 0);
            PF = ((!v12 && PF) || (v12 && v15));
            v16 = AF;
            AF = undef();
            AF = ((!v12 && v16) || (v12 && AF));"#,
    );
}

#[test]
fn x64_rol_rdx_one() {
    let spec = crate::x64::spec();
    let context = spec.new_context();
    let (display, info, ast) = decode_ast(spec, &context, b"\x48\xd1\xc2");
    assert_eq!(display, "ROL RDX,1");
    assert_eq!(info.address, 0x1000);
    assert_eq!(info.length, 3);

    assert_ast_eq(
        spec,
        &ast,
        r#"CF = (RDX s< 0);
            RDX = ((RDX << 1) | zext(CF));
            OF = undef();
            OF = (CF ^ (RDX s< 0));"#,
    );
}

#[test]
fn x64_push_rbx() {
    let spec = crate::x64::spec();
    let context = spec.new_context();
    let (display, info, ast) = decode_ast(spec, &context, b"\x53");
    assert_eq!(display, "PUSH RBX");
    assert_eq!(info.address, 0x1000);
    assert_eq!(info.length, 1);

    let mysave = "v1";

    assert_ast_eq(
        spec,
        &ast,
        &format!(
            "{mysave}:8 = RBX;
                RSP = (RSP - 8);
                load(size=8, ptr=RSP) = {mysave};"
        ),
    );
}

#[test]
fn x64_pop_r12() {
    let spec = crate::x64::spec();
    let context = spec.new_context();
    let (display, info, ast) = decode_ast(spec, &context, b"\x41\x5c");
    assert_eq!(display, "POP R12");
    assert_eq!(info.address, 0x1000);
    assert_eq!(info.length, 2);

    assert_ast_eq(
        spec,
        &ast,
        r#"v0:8 = 0;
            v2:8 = load(size=8, ptr=RSP);
            RSP = (RSP + 8);
            v0 = v2;
            R12 = v0;"#,
    );
}

#[test]
fn x64_pushfq() {
    let spec = crate::x64::spec();
    let context = spec.new_context();
    let (display, info, ast) = decode_ast(spec, &context, b"\x9c");
    assert_eq!(display, "PUSHFQ");
    assert_eq!(info.address, 0x1000);
    assert_eq!(info.length, 1);

    let rflags = "rflags";
    let mysave = "v3";

    assert_ast_eq(
            spec,
            &ast,
            &format!(
                "{rflags} = ((((((((((16384 * zext((NT & 1))) | (2048 * zext((OF & 1)))) | (1024 * zext((DF & 1)))) | (512 * zext((IF & 1)))) | (256 * zext((TF & 1)))) | (128 * zext((SF & 1)))) | (64 * zext((ZF & 1)))) | (16 * zext((AF & 1)))) | (4 * zext((PF & 1)))) | (1 * zext((CF & 1))));
                {rflags} = (((({rflags} | (2097152 * zext((ID & 1)))) | (1048576 * zext((VIP & 1)))) | (524288 * zext((VIF & 1)))) | (262144 * zext((AC & 1))));
                {mysave}:8 = {rflags};
                RSP = (RSP - 8);
                load(size=8, ptr=RSP) = {mysave};"
            ),
        );
}

#[test]
fn x64_popfq() {
    let spec = crate::x64::spec();
    let context = spec.new_context();
    let (display, info, ast) = decode_ast(spec, &context, b"\x9d");
    assert_eq!(display, "POPFQ");
    assert_eq!(info.address, 0x1000);
    assert_eq!(info.length, 1);

    assert_ast_eq(
        spec,
        &ast,
        r#"v1:8 = load(size=8, ptr=RSP);
            RSP = (RSP + 8);
            rflags = v1;
            NT = ((rflags & 16384) != 0);
            OF = ((rflags & 2048) != 0);
            DF = ((rflags & 1024) != 0);
            IF = ((rflags & 512) != 0);
            TF = ((rflags & 256) != 0);
            SF = ((rflags & 128) != 0);
            ZF = ((rflags & 64) != 0);
            AF = ((rflags & 16) != 0);
            PF = ((rflags & 4) != 0);
            CF = ((rflags & 1) != 0);
            ID = ((rflags & 2097152) != 0);
            AC = ((rflags & 262144) != 0);
            VIP = 0;
            VIF = 0;"#,
    );
}

#[test]
fn x64_call_rax() {
    let spec = crate::x64::spec();
    let context = spec.new_context();
    let (display, info, ast) = decode_ast(spec, &context, b"\xff\xd0");
    assert_eq!(display, "CALL RAX");
    assert_eq!(info.address, 0x1000);
    assert_eq!(info.length, 2);

    let v0 = "v0";
    let v2 = "v2";

    assert_ast_eq(
        spec,
        &ast,
        &format!(
            "{v0}:8 = RAX;
                {v2}:8 = &:8 4098:8;
                RSP = (RSP - 8);
                load(size=8, ptr=RSP) = {v2};
                call [{v0}];"
        ),
    );
}

#[test]
fn x64_jmp_rel32() {
    let spec = crate::x64::spec();
    let context = spec.new_context();
    let (display, info, ast) = decode_ast(spec, &context, b"\xe9\x00\x00\x00\x00");
    assert_eq!(display, "JMP 4101");
    assert_eq!(info.address, 0x1000);
    assert_eq!(info.length, 5);

    assert_ast_eq(spec, &ast, "goto 4101:8;");
}

#[test]
fn x64_ret() {
    let spec = crate::x64::spec();
    let context = spec.new_context();
    let (display, info, ast) = decode_ast(spec, &context, b"\xc3");
    assert_eq!(display, "RET");
    assert_eq!(info.address, 0x1000);
    assert_eq!(info.length, 1);

    assert_ast_eq(
        spec,
        &ast,
        r#"v1:8 = load(size=8, ptr=RSP);
            RSP = (RSP + 8);
            RIP = v1;
            return [RIP];"#,
    );
}

#[test]
fn x64_mov_rax_indexed() {
    let spec = crate::x64::spec();
    let context = spec.new_context();
    let (display, info, ast) = decode_ast(spec, &context, b"\x48\x8b\x44\x8b\x20");
    assert_eq!(display, "MOV RAX,qword ptr [RBX + RCX*4 + 32]");
    assert_eq!(info.address, 0x1000);
    assert_eq!(info.length, 5);

    let v0 = "v0";

    assert_ast_eq(
        spec,
        &ast,
        &format!(
            "{v0} = ((32:8 + RBX) + (RCX * 4:1));
                RAX = load(space=ram, size=8, ptr={v0});"
        ),
    );
}

#[test]
fn x64_mov_byte_ptr_addr() {
    let spec = crate::x64::spec();
    let context = spec.new_context();
    let (display, info, ast) = decode_ast(spec, &context, b"\xc6\x05\x00\x01\x00\x00\x7f");
    assert_eq!(display, "MOV byte ptr [4359],127");
    assert_eq!(info.address, 0x1000);
    assert_eq!(info.length, 7);

    assert_ast_eq(spec, &ast, "load(space=ram, size=1, ptr=4359:8):1 = 127:1;");
}

#[test]
fn x64_movaps_xmm0_xmm1() {
    let spec = crate::x64::spec();
    let context = spec.new_context();
    let (display, info, ast) = decode_ast(spec, &context, b"\x0f\x28\xc1");
    assert_eq!(display, "MOVAPS XMM0, XMM1");
    assert_eq!(info.address, 0x1000);
    assert_eq!(info.length, 3);

    assert_ast_eq(
        spec,
        &ast,
        r#"range(XMM0, 0, 32) = range(XMM1, 0, 32);
            range(XMM0, 32, 32) = range(XMM1, 32, 32);
            range(XMM0, 64, 32) = range(XMM1, 64, 32);
            range(XMM0, 96, 32) = range(XMM1, 96, 32);"#,
    );
}

#[test]
fn x64_movsd_xmm2_ptr_rax() {
    let spec = crate::x64::spec();
    let context = spec.new_context();
    let (display, info, ast) = decode_ast(spec, &context, b"\xf2\x0f\x10\x10");
    assert_eq!(display, "MOVSD XMM2, qword ptr [RAX]");
    assert_eq!(info.address, 0x1000);
    assert_eq!(info.length, 4);

    assert_ast_eq(
        spec,
        &ast,
        r#"v0 = RAX;
            range(XMM2, 0, 64) = load(space=ram, size=8, ptr=v0);
            range(XMM2, 64, 64) = 0;"#,
    );
}

#[test]
fn x64_addss_xmm3_xmm4() {
    let spec = crate::x64::spec();
    let context = spec.new_context();
    let (display, info, ast) = decode_ast(spec, &context, b"\xf3\x0f\x58\xdc");
    assert_eq!(display, "ADDSS XMM3, XMM4");
    assert_eq!(info.address, 0x1000);
    assert_eq!(info.length, 4);

    assert_ast_eq(
        spec,
        &ast,
        "range(XMM3, 0, 32) = (range(XMM3, 0, 32) f+ range(XMM4, 0, 32));",
    );
}

#[test]
fn x64_movsb_rep() {
    let spec = crate::x64::spec();
    let context = spec.new_context();
    let (display, info, ast) = decode_ast(spec, &context, b"\xf3\xa4");
    assert_eq!(display, "MOVSB.REP RDI,RSI");
    assert_eq!(info.address, 0x1000);
    assert_eq!(info.length, 2);

    assert_ast_eq(
        spec,
        &ast,
        r#"if (RCX == 0) goto 4098:8;
            v0 = RDI;
            RDI = ((RDI + 1) - (2 * zext(DF)));
            v1 = RSI;
            RSI = ((RSI + 1) - (2 * zext(DF)));
            load(space=ram, size=1, ptr=v0):1 = load(space=ram, size=1, ptr=v1);
            RCX = (RCX - 1);
            goto 4096:8;"#,
    );
}

#[test]
fn x64_cmpsb_repe() {
    let spec = crate::x64::spec();
    let context = spec.new_context();
    let (display, info, ast) = decode_ast(spec, &context, b"\xf3\xa6");
    assert_eq!(display, "CMPSB.REPE RDI,RSI");
    assert_eq!(info.address, 0x1000);
    assert_eq!(info.length, 2);

    assert_ast_eq(
        spec,
        &ast,
        r#"if (RCX == 0) goto 4098:8;
            v6 = RDI;
            RDI = ((RDI + 1) - (2 * zext(DF)));
            v7 = RSI;
            RSI = ((RSI + 1) - (2 * zext(DF)));
            v0:1 = load(space=ram, size=1, ptr=v6);
            v1:1 = load(space=ram, size=1, ptr=v7);
            CF = (v1 < v0);
            OF = sborrow(v1, v0);
            AF = ((v1 & 15) < (v0 & 15));
            v2 = (v1 - v0);
            SF = (v2 s< 0);
            ZF = (v2 == 0);
            PF = ((popcount((v2 & 255)) & 1:1) == 0);
            RCX = (RCX - 1);
            if ZF goto 4096:8;"#,
    );
}

#[test]
fn x64_scasb_repne() {
    let spec = crate::x64::spec();
    let context = spec.new_context();
    let (display, info, ast) = decode_ast(spec, &context, b"\xf2\xae");
    assert_eq!(display, "SCASB.REPNE RDI");
    assert_eq!(info.address, 0x1000);
    assert_eq!(info.length, 2);

    assert_ast_eq(
        spec,
        &ast,
        r#"if (RCX == 0) goto 4098:8;
            v4 = RDI;
            RDI = ((RDI + 1) - (2 * zext(DF)));
            CF = (AL < load(space=ram, size=1, ptr=v4));
            OF = sborrow(AL, load(space=ram, size=1, ptr=v4));
            AF = ((AL & 15) < (load(space=ram, size=1, ptr=v4) & 15));
            v0 = (AL - load(space=ram, size=1, ptr=v4));
            SF = (v0 s< 0);
            ZF = (v0 == 0);
            PF = ((popcount((v0 & 255)) & 1:1) == 0);
            RCX = (RCX - 1);
            if !ZF goto 4096:8;"#,
    );
}

#[test]
fn x64_stosq_rep() {
    let spec = crate::x64::spec();
    let context = spec.new_context();
    let (display, info, ast) = decode_ast(spec, &context, b"\xf3\x48\xab");
    assert_eq!(display, "STOSQ.REP RDI");
    assert_eq!(info.address, 0x1000);
    assert_eq!(info.length, 3);

    assert_ast_eq(
        spec,
        &ast,
        r#"if (RCX == 0) goto 4099:8;
            v0 = RDI;
            RDI = ((RDI + 8) - (16 * zext(DF)));
            load(space=ram, size=8, ptr=v0):8 = RAX;
            RCX = (RCX - 1);
            goto 4096:8;"#,
    );
}

#[test]
fn x64_lodsb_rep() {
    let spec = crate::x64::spec();
    let context = spec.new_context();
    let (display, info, ast) = decode_ast(spec, &context, b"\xf3\xac");
    assert_eq!(display, "LODSB.REP RSI");
    assert_eq!(info.address, 0x1000);
    assert_eq!(info.length, 2);

    assert_ast_eq(
        spec,
        &ast,
        r#"if (RCX == 0) goto 4098:8;
            v0 = RSI;
            RSI = ((RSI + 1) - (2 * zext(DF)));
            AL = load(space=ram, size=1, ptr=v0);
            RCX = (RCX - 1);
            goto 4096:8;"#,
    );
}

#[test]
fn x64_add_lock() {
    let spec = crate::x64::spec();
    let context = spec.new_context();
    let (display, info, ast) = decode_ast(spec, &context, b"\xf0\x48\x83\x00\x01");
    assert_eq!(display, "ADD.LOCK qword ptr [RAX],1");
    assert_eq!(info.address, 0x1000);
    assert_eq!(info.length, 5);

    assert_ast_eq(
        spec,
        &ast,
        r#"LOCK();
            v4 = RAX;
            CF = carry(load(space=ram, size=8, ptr=v4), 1:8);
            OF = scarry(load(space=ram, size=8, ptr=v4), 1:8);
            v2 = (load(space=ram, size=8, ptr=v4) + 1:8);
            AF = ((((load(space=ram, size=8, ptr=v4) ^ 1:8) ^ v2) & 16) != 0);
            load(space=ram, size=8, ptr=v4):8 = (load(space=ram, size=8, ptr=v4) + 1:8);
            SF = (load(space=ram, size=8, ptr=v4) s< 0);
            ZF = (load(space=ram, size=8, ptr=v4) == 0);
            PF = ((popcount((load(space=ram, size=8, ptr=v4) & 255)) & 1:1) == 0);
            UNLOCK();"#,
    );
}

#[test]
fn x64_xchg_lock() {
    let spec = crate::x64::spec();
    let context = spec.new_context();
    let (display, info, ast) = decode_ast(spec, &context, b"\xf0\x48\x87\x0b");
    assert_eq!(display, "XCHG.LOCK qword ptr [RBX],RCX");
    assert_eq!(info.address, 0x1000);
    assert_eq!(info.length, 4);

    assert_ast_eq(
        spec,
        &ast,
        r#"LOCK();
            v1 = RBX;
            v0 = load(space=ram, size=8, ptr=v1);
            load(space=ram, size=8, ptr=v1):8 = RCX;
            RCX = v0;
            UNLOCK();"#,
    );
}

#[test]
fn x64_cmpxchg_lock() {
    let spec = crate::x64::spec();
    let context = spec.new_context();
    let (display, info, ast) = decode_ast(spec, &context, b"\xf0\x48\x0f\xb1\x37");
    assert_eq!(display, "CMPXCHG.LOCK qword ptr [RDI],RSI");
    assert_eq!(info.address, 0x1000);
    assert_eq!(info.length, 5);

    assert_ast_eq(
        spec,
        &ast,
        r#"v5 = RDI;
            LOCK();
            v0 = load(space=ram, size=8, ptr=v5);
            CF = (RAX < v0);
            OF = sborrow(RAX, v0);
            AF = ((RAX & 15) < (v0 & 15));
            v1 = (RAX - v0);
            SF = (v1 s< 0);
            ZF = (v1 == 0);
            PF = ((popcount((v1 & 255)) & 1:1) == 0);
            if ZF goto <equal>;
            RAX = v0;
            goto <inst_end>;
            <equal>
            load(space=ram, size=8, ptr=v5):8 = RSI;
            <inst_end>
            UNLOCK();"#,
    );
}

#[test]
fn x64_dec_ebx() {
    let spec = crate::x64::spec();
    let context = spec.new_context();
    let (display, info, ast) = decode_ast(spec, &context, b"\xff\xcb");
    assert_eq!(display, "DEC EBX");
    assert_eq!(info.address, 0x1000);
    assert_eq!(info.length, 2);

    assert_ast_eq(
        spec,
        &ast,
        r#"OF = sborrow(EBX, 1);
            AF = ((EBX & 15) == 0);
            EBX = (EBX - 1);
            RBX = zext(EBX);
            SF = (EBX s< 0);
            ZF = (EBX == 0);
            PF = ((popcount((EBX & 255)) & 1:1) == 0);"#,
    );
}

#[test]
fn x64_mov_ebx_imm32() {
    let spec = crate::x64::spec();
    let context = spec.new_context();
    let (display, info, ast) = decode_ast(spec, &context, b"\xbb\x78\x56\x34\x12");
    assert_eq!(display, "MOV EBX,305419896");
    assert_eq!(info.address, 0x1000);
    assert_eq!(info.length, 5);

    assert_ast_eq(spec, &ast, "RBX = 305419896:4;");
}

#[test]
fn x64_mov_ax_bx() {
    let spec = crate::x64::spec();
    let context = spec.new_context();
    let (display, info, ast) = decode_ast(spec, &context, b"\x66\x89\xd8");
    assert_eq!(display, "MOV AX,BX");
    assert_eq!(info.address, 0x1000);
    assert_eq!(info.length, 3);

    assert_ast_eq(spec, &ast, "AX = BX;");
}

#[test]
fn x64_add_word_ptr() {
    let spec = crate::x64::spec();
    let context = spec.new_context();
    let (display, info, ast) = decode_ast(spec, &context, b"\x66\x83\x00\x05");
    assert_eq!(display, "ADD word ptr [RAX],5");
    assert_eq!(info.address, 0x1000);
    assert_eq!(info.length, 4);

    assert_ast_eq(
        spec,
        &ast,
        r#"v4 = RAX;
            CF = carry(load(space=ram, size=2, ptr=v4), 5:2);
            OF = scarry(load(space=ram, size=2, ptr=v4), 5:2);
            v2 = (load(space=ram, size=2, ptr=v4) + 5:2);
            AF = ((((load(space=ram, size=2, ptr=v4) ^ 5:2) ^ v2) & 16) != 0);
            load(space=ram, size=2, ptr=v4):2 = (load(space=ram, size=2, ptr=v4) + 5:2);
            SF = (load(space=ram, size=2, ptr=v4) s< 0);
            ZF = (load(space=ram, size=2, ptr=v4) == 0);
            PF = ((popcount((load(space=ram, size=2, ptr=v4) & 255)) & 1:1) == 0);"#,
    );
}

#[test]
fn x64_push_imm32() {
    let spec = crate::x64::spec();
    let context = spec.new_context();
    let (display, info, ast) = decode_ast(spec, &context, b"\x68\x34\x12\x00\x00");
    assert_eq!(display, "PUSH 4660");
    assert_eq!(info.address, 0x1000);
    assert_eq!(info.length, 5);

    let tmp = "v0";
    let mysave = "v2";

    assert_ast_eq(
        spec,
        &ast,
        &format!(
            "{tmp}:8 = 4660:8;
                {mysave}:8 = {tmp};
                RSP = (RSP - 8);
                load(size=8, ptr=RSP) = {mysave};"
        ),
    );
}

#[test]
fn x64_imul_ax_cx() {
    let spec = crate::x64::spec();
    let context = spec.new_context();
    let (display, info, ast) = decode_ast(spec, &context, b"\x66\x0f\xaf\xc1");
    assert_eq!(display, "IMUL AX,CX");
    assert_eq!(info.address, 0x1000);
    assert_eq!(info.length, 4);

    let tmp = "v0";
    let high = "v1";

    assert_ast_eq(
        spec,
        &ast,
        &format!(
            "{tmp}:4 = (sext(AX) * sext(CX));
                AX = subpiece_msb({tmp}, 0);
                {high}:2 = subpiece_msb({tmp}, 2);
                CF = (sext(AX) != {tmp});
                OF = CF;
                AF = undef();
                PF = undef();
                SF = undef();
                ZF = undef();"
        ),
    );
}

#[test]
fn x64_mov_eax_ptr_ecx_addr32() {
    let spec = crate::x64::spec();
    let context = spec.new_context();
    let (display, info, ast) = decode_ast(spec, &context, b"\x67\x8b\x01");
    assert_eq!(display, "MOV EAX,dword ptr [ECX]");
    assert_eq!(info.address, 0x1000);
    assert_eq!(info.length, 3);

    assert_ast_eq(
        spec,
        &ast,
        r#"v1 = ECX;
            v0:8 = sext(v1);
            EAX = load(space=ram, size=4, ptr=v0);
            RAX = zext(EAX);"#,
    );
}

#[test]
fn x64_mov_dword_ptr_indexed() {
    let spec = crate::x64::spec();
    let context = spec.new_context();
    let (display, info, ast) = decode_ast(spec, &context, b"\x67\x89\x04\xb2");
    assert_eq!(display, "MOV dword ptr [EDX + ESI*4],EAX");
    assert_eq!(info.address, 0x1000);
    assert_eq!(info.length, 4);

    let addr32_tmp = "v1";
    let addr_tmp = "v0";

    assert_ast_eq(
        spec,
        &ast,
        &format!(
            "{addr32_tmp} = (EDX + (ESI * 4:1));
                {addr_tmp}:8 = sext({addr32_tmp});
                load(space=ram, size=4, ptr={addr_tmp}):4 = EAX;"
        ),
    );
}

#[test]
fn x64_lea_eax_offset() {
    let spec = crate::x64::spec();
    let context = spec.new_context();
    let (display, info, ast) = decode_ast(spec, &context, b"\x67\x8d\x43\x04");
    assert_eq!(display, "LEA EAX,[EBX + 4]");
    assert_eq!(info.address, 0x1000);
    assert_eq!(info.length, 4);

    assert_ast_eq(
        spec,
        &ast,
        r#"v1 = (EBX + 4:4);
            v0:8 = sext(v1);
            EAX = subpiece_lsb(v0, 4);
            RAX = zext(EAX);"#,
    );
}

#[test]
fn x64_mov_fs() {
    let spec = crate::x64::spec();
    let context = spec.new_context();
    let (display, info, ast) = decode_ast(spec, &context, b"\x64\x48\x8b\x04\x25\x00\x00\x00\x00");
    assert_eq!(display, "MOV RAX,qword ptr FS:[0]");
    assert_eq!(info.address, 0x1000);
    assert_eq!(info.length, 9);

    let addr_tmp = "v0";

    assert_ast_eq(
        spec,
        &ast,
        &format!(
            "{addr_tmp}:8 = (FS_OFFSET + 0:8);
                RAX = load(space=ram, size=8, ptr={addr_tmp});"
        ),
    );
}

#[test]
fn x64_mov_gs() {
    let spec = crate::x64::spec();
    let context = spec.new_context();
    let (display, info, ast) = decode_ast(spec, &context, b"\x65\x48\x8b\x1c\x25\x30\x00\x00\x00");
    assert_eq!(display, "MOV RBX,qword ptr GS:[48]");
    assert_eq!(info.address, 0x1000);
    assert_eq!(info.length, 9);

    let addr_tmp = "v0";

    assert_ast_eq(
        spec,
        &ast,
        &format!(
            "{addr_tmp}:8 = (GS_OFFSET + 48:8);
                RBX = load(space=ram, size=8, ptr={addr_tmp});"
        ),
    );
}

#[test]
fn x64_mov_cs() {
    let spec = crate::x64::spec();
    let context = spec.new_context();
    let (display, info, ast) = decode_ast(spec, &context, b"\x2e\x8b\x05\x10\x00\x00\x00");
    assert_eq!(display, "MOV EAX,dword ptr CS:[4119]");
    assert_eq!(info.address, 0x1000);
    assert_eq!(info.length, 7);

    assert_ast_eq(
        spec,
        &ast,
        r#"EAX = load(space=ram, size=4, ptr=4119:8);
            RAX = zext(EAX);"#,
    );
}

#[test]
fn x64_mov_rdx_rsp() {
    let spec = crate::x64::spec();
    let context = spec.new_context();
    let (display, info, ast) = decode_ast(spec, &context, b"\x48\x8b\x14\x24");
    assert_eq!(display, "MOV RDX,qword ptr [RSP]");
    assert_eq!(info.address, 0x1000);
    assert_eq!(info.length, 4);

    assert_ast_eq(
        spec,
        &ast,
        r#"v0 = RSP;
            RDX = load(space=ram, size=8, ptr=v0);"#,
    );
}

#[test]
fn x64_mov_r8_r9() {
    let spec = crate::x64::spec();
    let context = spec.new_context();
    let (display, info, ast) = decode_ast(spec, &context, b"\x4d\x89\xc8");
    assert_eq!(display, "MOV R8,R9");
    assert_eq!(info.address, 0x1000);
    assert_eq!(info.length, 3);

    assert_ast_eq(spec, &ast, "R8 = R9;");
}

#[test]
fn x64_add_r10_r11() {
    let spec = crate::x64::spec();
    let context = spec.new_context();
    let (display, info, ast) = decode_ast(spec, &context, b"\x4d\x01\xda");
    assert_eq!(display, "ADD R10,R11");
    assert_eq!(info.address, 0x1000);
    assert_eq!(info.length, 3);

    assert_ast_eq(
        spec,
        &ast,
        r#"CF = carry(R10, R11);
            OF = scarry(R10, R11);
            v2 = (R10 + R11);
            AF = ((((R10 ^ R11) ^ v2) & 16) != 0);
            R10 = (R10 + R11);
            SF = (R10 s< 0);
            ZF = (R10 == 0);
            PF = ((popcount((R10 & 255)) & 1:1) == 0);"#,
    );
}

#[test]
fn x64_mov_r12_r13_ptr() {
    let spec = crate::x64::spec();
    let context = spec.new_context();
    let (display, info, ast) = decode_ast(spec, &context, b"\x4d\x8b\x65\x00");
    assert_eq!(display, "MOV R12,qword ptr [R13]");
    assert_eq!(info.address, 0x1000);
    assert_eq!(info.length, 4);

    assert_ast_eq(
        spec,
        &ast,
        r#"v0 = R13;
            R12 = load(space=ram, size=8, ptr=v0);"#,
    );
}

#[test]
fn x64_lea_r14_r15_offset() {
    let spec = crate::x64::spec();
    let context = spec.new_context();
    let (display, info, ast) = decode_ast(spec, &context, b"\x4d\x8d\x77\x08");
    assert_eq!(display, "LEA R14,[R15 + 8]");
    assert_eq!(info.address, 0x1000);
    assert_eq!(info.length, 4);

    let addr_tmp = "v0";

    assert_ast_eq(
        spec,
        &ast,
        &format!(
            "{addr_tmp} = (R15 + 8:8);
                R14 = {addr_tmp};"
        ),
    );
}

#[test]
fn x64_movupd_xmm0_xmm1() {
    let spec = crate::x64::spec();
    let context = spec.new_context();
    let (display, info, ast) = decode_ast(spec, &context, b"\x66\x0f\x10\xc1");
    assert_eq!(display, "MOVUPD XMM0, XMM1");
    assert_eq!(info.address, 0x1000);
    assert_eq!(info.length, 4);

    assert_ast_eq(
        spec,
        &ast,
        r#"range(XMM0, 0, 64) = range(XMM1, 0, 64);
            range(XMM0, 64, 64) = range(XMM1, 64, 64);"#,
    );
}

#[test]
fn x64_addsd_xmm1_xmm2() {
    let spec = crate::x64::spec();
    let context = spec.new_context();
    let (display, info, ast) = decode_ast(spec, &context, b"\xf2\x0f\x58\xca");
    assert_eq!(display, "ADDSD XMM1, XMM2");
    assert_eq!(info.address, 0x1000);
    assert_eq!(info.length, 4);

    assert_ast_eq(
        spec,
        &ast,
        "range(XMM1, 0, 64) = (range(XMM1, 0, 64) f+ range(XMM2, 0, 64));",
    );
}

#[test]
fn x64_movsd_xmm2_xmm3() {
    let spec = crate::x64::spec();
    let context = spec.new_context();
    let (display, info, ast) = decode_ast(spec, &context, b"\xf2\x0f\x10\xd3");
    assert_eq!(display, "MOVSD XMM2, XMM3");
    assert_eq!(info.address, 0x1000);
    assert_eq!(info.length, 4);

    assert_ast_eq(spec, &ast, "range(XMM2, 0, 64) = range(XMM3, 0, 64);");
}

#[test]
fn x64_movss_xmm4_xmm5() {
    let spec = crate::x64::spec();
    let context = spec.new_context();
    let (display, info, ast) = decode_ast(spec, &context, b"\xf3\x0f\x10\xe5");
    assert_eq!(display, "MOVSS XMM4, XMM5");
    assert_eq!(info.address, 0x1000);
    assert_eq!(info.length, 4);

    assert_ast_eq(spec, &ast, "range(XMM4, 0, 32) = range(XMM5, 0, 32);");
}

#[test]
fn x64_addpd_xmm6_xmm7() {
    let spec = crate::x64::spec();
    let context = spec.new_context();
    let (display, info, ast) = decode_ast(spec, &context, b"\x66\x0f\x58\xf7");
    assert_eq!(display, "ADDPD XMM6, XMM7");
    assert_eq!(info.address, 0x1000);
    assert_eq!(info.length, 4);

    assert_ast_eq(
        spec,
        &ast,
        r#"range(XMM6, 0, 64) = (range(XMM6, 0, 64) f+ range(XMM7, 0, 64));
            range(XMM6, 64, 64) = (range(XMM6, 64, 64) f+ range(XMM7, 64, 64));"#,
    );
}

#[test]
fn x64_imul_mem() {
    let spec = crate::x64::spec();
    let context = spec.new_context();
    let (display, info, ast) = decode_ast(spec, &context, b"\x48\x0f\xaf\x45\xd0");
    assert_eq!(display, "IMUL RAX,qword ptr [RBP + -48]");
    assert_eq!(info.address, 0x1000);
    assert_eq!(info.length, 5);

    let addr = "v4";
    let wide = "v0";
    let high = "v1";

    assert_ast_eq(
        spec,
        &ast,
        &format!(
            "{addr} = (RBP + 18446744073709551568:8);
                {wide}:16 = (sext(RAX) * sext(load(space=ram, size=8, ptr={addr})));
                RAX = (RAX * load(space=ram, size=8, ptr={addr}));
                {high}:8 = subpiece_msb({wide}, 8);
                CF = (sext(RAX) != {wide});
                OF = CF;
                AF = undef();
                PF = undef();
                SF = undef();
                ZF = undef();"
        ),
    );
}

/// Writing a 32-bit register destination zero-extends into the full 64-bit
/// register. This guards the `check_Reg32_dest` fix on `:MOVBE Reg32, m32`
/// in open_sleigh's `src/x86/ia.sinc`.
#[test]
fn x64_movbe_reg32_zext() {
    let spec = crate::x64::spec();
    let context = spec.new_context();
    let (display, info, ast) = decode_ast(spec, &context, b"\x0f\x38\xf0\x00");
    assert_eq!(display, "MOVBE EAX, dword ptr [RAX]");
    assert_eq!(info.address, 0x1000);
    assert_eq!(info.length, 4);

    let pretty = ast.pretty_print(spec);
    assert!(
        pretty.contains("RAX = zext(EAX)"),
        "MOVBE into EAX must zero-extend RAX; got:\n{pretty}"
    );
}

// ---------------------------------------------------------------------------
// `IMUL r, r/m, imm` (opcodes 0x69 / 0x6B) regression matrix.
//
// The trailing immediate of these forms is a *subtable* operand
// (`simm8_*` / `simm32_32`), not a raw token field, so it took the
// `OperandType::Table` arm of the `;`-relative offset resolution in
// `sleigh::runtime::walker`. That arm trusted the single relative base picked
// by `TokenPattern::cat`, which is the *last* operand of the left-hand
// pattern — here `Reg64`/`Reg32`/`Reg16` (zero extent, decoded out of the
// ModRM reg field) rather than `rm64`/`rm32`/`rm16` (which also eats SIB and
// displacement bytes). The immediate was therefore read from the byte right
// after ModRM, and the instruction length came out short.
//
// Expected values below were taken from `objdump -b binary -m i386:x86-64`.
// ---------------------------------------------------------------------------

/// `imul $0xf4240,(%rax),%rax` — no SIB, no displacement (the case that
/// always worked, kept as the control).
#[test]
fn x64_imul_imm32_no_sib_no_disp() {
    let spec = crate::x64::spec();
    let context = spec.new_context();
    let (display, info, _) = decode_ast(spec, &context, b"\x48\x69\x00\x40\x42\x0f\x00");
    assert_eq!(display, "IMUL RAX,qword ptr [RAX],1000000");
    assert_eq!(info.length, 7);
}

/// `imul $0x28,0x10(%rbx),%rax` — disp8, no SIB.
#[test]
fn x64_imul_imm8_disp8_no_sib() {
    let spec = crate::x64::spec();
    let context = spec.new_context();
    let (display, info, _) = decode_ast(spec, &context, b"\x48\x6b\x43\x10\x28");
    assert_eq!(display, "IMUL RAX,qword ptr [RBX + 16],40");
    assert_eq!(info.length, 5);
}

/// `imul $0xf4240,0x12345678(%rbx),%rax` — disp32, no SIB.
#[test]
fn x64_imul_imm32_disp32_no_sib() {
    let spec = crate::x64::spec();
    let context = spec.new_context();
    let (display, info, _) = decode_ast(
        spec,
        &context,
        b"\x48\x69\x83\x78\x56\x34\x12\x40\x42\x0f\x00",
    );
    assert_eq!(display, "IMUL RAX,qword ptr [RBX + 305419896],1000000");
    assert_eq!(info.length, 11);
}

/// `imul $0x28,0x12345678(%rbx),%rax` — disp32, no SIB, imm8 opcode.
#[test]
fn x64_imul_imm8_disp32_no_sib() {
    let spec = crate::x64::spec();
    let context = spec.new_context();
    let (display, info, _) = decode_ast(spec, &context, b"\x48\x6b\x83\x78\x56\x34\x12\x28");
    assert_eq!(display, "IMUL RAX,qword ptr [RBX + 305419896],40");
    assert_eq!(info.length, 8);
}

/// `imul $0xf4240,(%rsp),%rax` — SIB, no displacement.
#[test]
fn x64_imul_imm32_sib_no_disp() {
    let spec = crate::x64::spec();
    let context = spec.new_context();
    let (display, info, _) = decode_ast(spec, &context, b"\x48\x69\x04\x24\x40\x42\x0f\x00");
    assert_eq!(display, "IMUL RAX,qword ptr [RSP],1000000");
    assert_eq!(info.length, 8);
}

/// `imul $0x28,0x10(%rsp),%rax` — SIB + disp8.
#[test]
fn x64_imul_imm8_sib_disp8() {
    let spec = crate::x64::spec();
    let context = spec.new_context();
    let (display, info, _) = decode_ast(spec, &context, b"\x48\x6b\x44\x24\x10\x28");
    assert_eq!(display, "IMUL RAX,qword ptr [RSP + 16],40");
    assert_eq!(info.length, 6);
}

/// `imul $0xf4240,0x12345678(%rsp),%rax` — SIB + disp32.
#[test]
fn x64_imul_imm32_sib_disp32() {
    let spec = crate::x64::spec();
    let context = spec.new_context();
    let (display, info, _) = decode_ast(
        spec,
        &context,
        b"\x48\x69\x84\x24\x78\x56\x34\x12\x40\x42\x0f\x00",
    );
    assert_eq!(display, "IMUL RAX,qword ptr [RSP + 305419896],1000000");
    assert_eq!(info.length, 12);
}

/// `imul $0x28,0x12345678(%rip),%rax` — RIP-relative (disp32, no SIB, mod=0
/// r/m=5). The displayed target folds in `inst_next`, so a short length is
/// visible twice over.
#[test]
fn x64_imul_imm8_rip_relative() {
    let spec = crate::x64::spec();
    let context = spec.new_context();
    let (display, info, _) = decode_ast(spec, &context, b"\x48\x6b\x05\x78\x56\x34\x12\x28");
    // decoded at 0x1000, so inst_next = 0x1008 and 0x1008 + 0x12345678 = 305424000
    assert_eq!(display, "IMUL RAX,qword ptr [305424000],40");
    assert_eq!(info.length, 8);
}

/// `imul $0x28,0x12345678,%rax` — SIB with no base (base=5, mod=0) + disp32.
#[test]
fn x64_imul_imm8_sib_no_base_disp32() {
    let spec = crate::x64::spec();
    let context = spec.new_context();
    let (display, info, _) = decode_ast(spec, &context, b"\x48\x6b\x04\x25\x78\x56\x34\x12\x28");
    assert_eq!(display, "IMUL RAX,qword ptr [305419896],40");
    assert_eq!(info.length, 9);
}

/// `imul $0x28,%rsp,%rax` — register form (mod=3), the other control case.
#[test]
fn x64_imul_imm8_register_form() {
    let spec = crate::x64::spec();
    let context = spec.new_context();
    let (display, info, _) = decode_ast(spec, &context, b"\x48\x6b\xc4\x28");
    assert_eq!(display, "IMUL RAX,RSP,40");
    assert_eq!(info.length, 4);
}

/// `imul $0xf4240,(%rsp),%eax` — 32-bit operand size, SIB.
#[test]
fn x64_imul_imm32_sib_no_disp_opsize32() {
    let spec = crate::x64::spec();
    let context = spec.new_context();
    let (display, info, _) = decode_ast(spec, &context, b"\x69\x04\x24\x40\x42\x0f\x00");
    assert_eq!(display, "IMUL EAX,dword ptr [RSP],1000000");
    assert_eq!(info.length, 7);
}

/// `imul $0x28,0x10(%rsp),%eax` — 32-bit operand size, SIB + disp8.
#[test]
fn x64_imul_imm8_sib_disp8_opsize32() {
    let spec = crate::x64::spec();
    let context = spec.new_context();
    let (display, info, _) = decode_ast(spec, &context, b"\x6b\x44\x24\x10\x28");
    assert_eq!(display, "IMUL EAX,dword ptr [RSP + 16],40");
    assert_eq!(info.length, 5);
}

/// `imul $0x4240,(%rsp),%ax` — 16-bit operand size, SIB, imm16.
#[test]
fn x64_imul_imm16_sib_no_disp_opsize16() {
    let spec = crate::x64::spec();
    let context = spec.new_context();
    let (display, info, _) = decode_ast(spec, &context, b"\x66\x69\x04\x24\x40\x42");
    assert_eq!(display, "IMUL AX,word ptr [RSP],16960");
    assert_eq!(info.length, 6);
}

/// `imul $0x28,0x10(%rsp),%ax` — 16-bit operand size, SIB + disp8, imm8.
#[test]
fn x64_imul_imm8_sib_disp8_opsize16() {
    let spec = crate::x64::spec();
    let context = spec.new_context();
    let (display, info, _) = decode_ast(spec, &context, b"\x66\x6b\x44\x24\x10\x28");
    assert_eq!(display, "IMUL AX,word ptr [RSP + 16],40");
    assert_eq!(info.length, 6);
}
