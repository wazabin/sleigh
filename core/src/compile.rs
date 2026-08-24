//! Compilation facade for building runtime SLEIGH specifications.

use std::collections::HashMap;

use crate::{
    builder::SpecBuilder,
    diagnostic::{CompileError, Diagnostic, DiagnosticCode},
    resolve::resolve,
    runtime::CompiledSpec,
    source::{FileId, PreprocessOptions, SourceDb},
    spec::Spec,
    syntax::parse_to_ast,
};

/// Options that control SLEIGH compilation.
#[derive(Debug, Clone, Default)]
pub struct CompileOptions {
    /// Reserved for the source layer that will handle preprocessing in a later slice.
    pub defines: HashMap<String, String>,
}

/// Compiler for SLEIGH source files stored in a [`SourceDb`].
pub struct Compiler<'src> {
    sources: &'src mut SourceDb,
    options: CompileOptions,
}

impl<'src> Compiler<'src> {
    /// Creates a compiler with default options.
    pub fn new(sources: &'src mut SourceDb) -> Self {
        Self {
            sources,
            options: CompileOptions::default(),
        }
    }

    /// Replaces the compile options.
    pub fn with_options(mut self, options: CompileOptions) -> Self {
        self.options = options;
        self
    }

    /// Compiles `root` into a [`CompiledSpec`].
    ///
    /// # Errors
    ///
    /// Returns a [`CompileError`] carrying one or more [`Diagnostic`]s if the
    /// specification cannot be preprocessed, parsed, resolved or concretized —
    /// including for SLEIGH this crate parses but does not yet compile, such as
    /// a right-aligned pattern or a comparison between two fields.
    ///
    /// [`Diagnostic`]: crate::Diagnostic
    ///
    /// ```
    /// use sleigh::{Compiler, SourceDb};
    ///
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let mut sources = SourceDb::new();
    /// let root = sources.add_file(
    ///     "tiny.slaspec",
    ///     "define endian=little;
    ///      define space ram type=ram_space size=4 default;
    ///      define space register type=register_space size=4;
    ///      define register offset=0 size=4 [ r0 ];
    ///      define token instr(8) op=(0,7);
    ///      :nop is op=0 { }",
    /// );
    ///
    /// let spec = Compiler::new(&mut sources).compile(root)?;
    /// assert!(spec.register("r0").is_some());
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// Compiling is the expensive step — hundreds of milliseconds for a real
    /// processor — so do it once and keep the [`CompiledSpec`].
    pub fn compile(self, root: FileId) -> Result<CompiledSpec, CompileError> {
        let options = PreprocessOptions {
            defines: self.options.defines,
        };

        let (file, prepared) =
            parse_to_ast(self.sources, root, &options).map_err(CompileError::new)?;

        // Warnings are `analyze`'s business; compiling wants the specification.
        let (mut builder, _warnings): (SpecBuilder, _) =
            resolve(&file).map_err(CompileError::new)?;

        builder.concretize().map_err(|e| CompileError::one(*e))?;

        if let Err(error) = builder.finalize_pcode() {
            let location = error
                .span
                .and_then(|(s, e)| self.sources.try_map_preprocessed_bytes(prepared, s, e))
                .unwrap_or_else(|| crate::source::Span::file_level(root));
            let diagnostic =
                Diagnostic::error(DiagnosticCode::Compile, format!("{error}"), location);
            return Err(CompileError::one(diagnostic));
        }

        let spec = Spec::from_builder(builder);
        Ok(CompiledSpec::from_spec(spec))
    }
}
