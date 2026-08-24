use sleigh::{
    CompiledSpec, ContextBytes, Decoder, EmitError, InstructionInfo, PcodeAst, SemanticsSink,
};

#[derive(Default)]
pub struct RecordingSink {
    instructions: Vec<InstructionInfo>,
    pcode: Vec<PcodeAst>,
}

impl SemanticsSink for RecordingSink {
    fn instruction(&mut self, info: &InstructionInfo, pcode: &PcodeAst) -> Result<(), EmitError> {
        self.instructions.push(info.clone());
        self.pcode.push(pcode.clone());
        Ok(())
    }
}

pub(crate) fn decode_ast(
    spec: &'static CompiledSpec,
    context: &ContextBytes,
    bytes: &[u8],
) -> (String, InstructionInfo, PcodeAst) {
    let instruction = Decoder::new(spec)
        .decode_one(0x1000, bytes, context)
        .expect("instruction should decode");
    let display = instruction.to_string();

    let mut sink = RecordingSink::default();
    instruction
        .emit_into(&mut sink)
        .expect("p-code AST should emit into sink");

    assert_eq!(sink.instructions.len(), 1);
    assert_eq!(sink.pcode.len(), 1);
    (display, sink.instructions.remove(0), sink.pcode.remove(0))
}

pub(crate) fn assert_ast_eq(spec: &CompiledSpec, ast: &PcodeAst, expected: &str) {
    let actual = ast.pretty_print(spec);

    println!("Actual Pcode:\n{actual}");

    let actual = actual.lines().collect::<Vec<_>>();
    let expected = expected.lines().collect::<Vec<_>>();

    assert_eq!(expected.len(), actual.len(), "Number of lines should match");

    for (actual_line, expected_line) in actual.iter().zip(expected.iter()) {
        assert_eq!(
            actual_line.trim(),
            expected_line.trim(),
            "Pcode pretty-print should match expected"
        );
    }
}
