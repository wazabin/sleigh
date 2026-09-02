use std::collections::HashMap;

/// Subtable exports are uncommon in the x86 semantics. Keep this map absent
/// until a `build` or a pre-emitted subtable actually produces one.
#[derive(Default)]
struct BuildExports(Option<HashMap<TableId, RuntimeValue>>);

impl BuildExports {
    fn contains_key(&self, table: &TableId) -> bool {
        self.0
            .as_ref()
            .is_some_and(|exports| exports.contains_key(table))
    }

    fn get(&self, table: &TableId) -> Option<&RuntimeValue> {
        self.0.as_ref().and_then(|exports| exports.get(table))
    }

    fn insert(&mut self, table: TableId, value: RuntimeValue) {
        self.0.get_or_insert_default().insert(table, value);
    }
}

use crate::{
    builder::SymbolId,
    instance::ConstructorInstance,
    objects::table::TableId,
    pmacro::{
        BodyTemplate, PMacroId,
        expression::{
            Expression, ExpressionTy, Ident, Load, LocalVarId, Range, RangeParam, SpaceRef,
            ident_size, infer_expr_size,
        },
        statement::{Ast, AstNode, LabelOrNode},
    },
    runtime::pcode::{RuntimeValue, collect, runtime_value_size},
    semantics::{EmitError, PcodeStatement},
    spec::Spec,
};

pub(crate) struct PcodeExpander<'spec, 'i> {
    spec: &'spec Spec,
    pub(crate) stmts: Vec<PcodeStatement>,

    /// Local widths resolved from each spliced body's compile-time widths,
    /// keyed by the rebased id the emitted statements use.
    pub(crate) local_sizes: pcode_types::LocalSizes,

    /// Cleared when a body's compile-time widths could not be resolved, so the
    /// consumer falls back to inferring them from the expanded statements.
    pub(crate) widths_resolved: bool,
    macro_stack: Vec<PMacroId>,
    /// Monotonically increasing counter: next available `LocalVarId` for this decode.
    next_var_id: u32,
    /// Base offset added to each constructor's locals so different subtable
    /// invocations never share an ID.
    current_base: u32,

    /// `inst_next` as the semantic section sees it — past the delay slot. Swapped
    /// while a delayed instruction is spliced, since that instruction's body
    /// means its *own* `inst_next`.
    inst_next: u64,

    /// Address past the instruction after this one, when a constructor asked
    /// for it by reading `inst_next2`.
    inst_next2: Option<u64>,

    /// Instructions filling the current instruction's delay slot, spliced where
    /// its `delayslot` directive sits. Held here rather than looked up from the
    /// instance being emitted, because the directive may live in a
    /// sub-constructor while the delayed instructions belong to the root.
    delay_slots: &'i [ConstructorInstance],

    /// Label namespace for the body being emitted. `0` is the instruction's
    /// own; each spliced delay-slot instruction gets its own, so a label the
    /// delayed instruction declares cannot collide with one of the same name in
    /// the instruction that delayed it.
    label_scope: u32,

    /// Next unused value for [`Self::label_scope`].
    next_label_scope: u32,
}

impl<'spec, 'i> PcodeExpander<'spec, 'i> {
    pub(crate) fn new(spec: &'spec Spec, instance: &'i ConstructorInstance) -> Self {
        Self {
            spec,
            stmts: Vec::new(),
            local_sizes: pcode_types::LocalSizes::default(),
            widths_resolved: true,
            macro_stack: Vec::new(),
            next_var_id: 0,
            current_base: 0,
            inst_next: instance.semantic_inst_next(),
            inst_next2: instance.inst_next2,
            delay_slots: &instance.delay_slots,
            label_scope: 0,
            next_label_scope: 1,
        }
    }

    /// Qualifies a label with the body it was declared in.
    ///
    /// `#` cannot appear in a SLEIGH identifier, so a scoped name can never
    /// collide with one a specification wrote. The instruction's own labels are
    /// left alone, so an instruction with no delay slot emits exactly what it
    /// emitted before.
    fn scoped_label(&self, name: &str) -> Box<str> {
        match self.label_scope {
            0 => name.into(),
            scope => format!("{name}#ds{scope}").into(),
        }
    }

    fn scoped_target(&self, target: LabelOrNode) -> LabelOrNode {
        match target {
            LabelOrNode::Label(name) => LabelOrNode::Label(self.scoped_label(&name)),
            other => other,
        }
    }

    /// Emits one delayed instruction's p-code in place of the directive.
    fn emit_delay_slot(&mut self, delayed: &ConstructorInstance) -> Result<(), EmitError> {
        let base = self.next_var_id;
        self.next_var_id += delayed.constructor(self.spec).pmacro.local_var_count;

        let scope = self.next_label_scope;
        self.next_label_scope += 1;

        let old_base = std::mem::replace(&mut self.current_base, base);
        let old_scope = std::mem::replace(&mut self.label_scope, scope);
        let old_next = std::mem::replace(&mut self.inst_next, delayed.semantic_inst_next());
        let old_next2 = std::mem::replace(&mut self.inst_next2, delayed.inst_next2);

        let result = self.emit_instance(delayed, &Default::default());

        self.current_base = old_base;
        self.label_scope = old_scope;
        self.inst_next = old_next;
        self.inst_next2 = old_next2;

        result.map(|_| ())
    }

    pub(crate) fn emit_instance(
        &mut self,
        instance: &ConstructorInstance,
        env: &HashMap<LocalVarId, RuntimeValue>,
    ) -> Result<Option<RuntimeValue>, EmitError> {
        let constructor = instance.constructor(self.spec);
        let pmacro = &constructor.pmacro;

        // SLEIGH prefix/wrapper constructors: delegate to their self-table child.
        if pmacro.body.is_empty() && pmacro.export.is_none() {
            let self_table = TableId::from(usize::from(instance.tree));
            if let Some(child) = instance.child_value(self.spec, self_table) {
                return self.emit_instance(child, env);
            }
        }

        let own_end = self.current_base + pmacro.local_var_count;
        if self.next_var_id < own_end {
            self.next_var_id = own_end;
        }

        self.emit_body(instance, pmacro.template(), env)
    }

    fn emit_macro_call(
        &mut self,
        instance: &ConstructorInstance,
        id: PMacroId,
        args: &[Expression],
        outer_env: &HashMap<LocalVarId, RuntimeValue>,
        build_exports: &mut BuildExports,
    ) -> Result<Option<RuntimeValue>, EmitError> {
        if self.macro_stack.contains(&id) {
            return Err(EmitError::new("recursive p-code macro expansion"));
        }
        // Take an independent copy of the specification reference so the
        // macro's immutable template can stay borrowed while `self` emits it.
        let spec = self.spec;
        let macro_def = &spec.pmacros[id];
        if macro_def.args.len() != args.len() {
            return Err(EmitError::new(format!(
                "p-code macro expected {} arguments, got {}",
                macro_def.args.len(),
                args.len()
            )));
        }

        let mut env = HashMap::new();
        for (&arg_id, arg) in macro_def.args.iter().zip(args) {
            let arg = self.expand_expr(instance, arg, outer_env, build_exports)?;
            env.insert(arg_id, RuntimeValue::Expr(arg));
        }

        self.macro_stack.push(id);
        let export = self.emit_body(instance, macro_def.template(), &env);
        self.macro_stack.pop();
        export
    }

    fn emit_body(
        &mut self,
        instance: &ConstructorInstance,
        template: BodyTemplate<'_>,
        env: &HashMap<LocalVarId, RuntimeValue>,
    ) -> Result<Option<RuntimeValue>, EmitError> {
        let BodyTemplate {
            body,
            export,
            non_build_table_refs,
            local_widths,
            unsized_locals,
        } = template;
        let base = self.current_base;
        let mut build_exports = BuildExports::default();

        // Pre-emit all non-`build` subtable references.
        for &table_id in non_build_table_refs {
            if !build_exports.contains_key(&table_id)
                && let Ok(export_val) = self.emit_child_scoped(instance, table_id, env)
                && let Some(export_val) = export_val
            {
                build_exports.insert(table_id, export_val);
            }
        }

        for stmt in body {
            self.emit_stmt(instance, stmt, env, &mut build_exports)?;
        }
        // Resolve this body's widths now: a width naming a table operand needs
        // that operand's export, which only exists once the body has been
        // emitted.
        self.resolve_local_widths(base, local_widths, unsized_locals, &build_exports);
        export
            .map(|expr| self.expand_export_value(instance, expr, env, &mut build_exports))
            .transpose()
    }

    /// Rebases one body's compile-time widths into the ids its emitted
    /// statements use, resolving any that name an operand.
    fn resolve_local_widths(
        &mut self,
        base: u32,
        local_widths: &HashMap<LocalVarId, pcode_types::SymbolicWidth>,
        unsized_locals: &[LocalVarId],
        build_exports: &BuildExports,
    ) {
        // A body with a local nothing can size leaves the widths incomplete,
        // whatever the rest of them resolved to.
        if !unsized_locals.is_empty() {
            self.widths_resolved = false;
        }
        for (id, width) in local_widths {
            let size = match width {
                pcode_types::SymbolicWidth::Fixed(size) => Some(*size),
                pcode_types::SymbolicWidth::SameAs(table) => {
                    build_exports.get(table).and_then(runtime_value_size)
                }
            };
            match size {
                Some(size) => {
                    self.local_sizes.insert(LocalVarId(base + id.0), size);
                }
                None => self.widths_resolved = false,
            }
        }
    }

    fn expand_export_value(
        &mut self,
        instance: &ConstructorInstance,
        expr: &Expression,
        env: &HashMap<LocalVarId, RuntimeValue>,
        build_exports: &mut BuildExports,
    ) -> Result<RuntimeValue, EmitError> {
        if let ExpressionTy::Load(load) = &expr.ty {
            let ptr = self.expand_expr(instance, &load.ptr, env, build_exports)?;
            let space = match &load.space {
                Some(SpaceRef::Resolved(id)) => *id,
                Some(SpaceRef::Deferred(name)) => {
                    return Err(EmitError::new(format!(
                        "unresolved space `{name}` reached runtime"
                    )));
                }
                None => self.spec.default_space,
            };
            let size = load
                .size
                .ok_or_else(|| EmitError::new("address export must have an explicit load size"))?;
            return Ok(RuntimeValue::Address { ptr, space, size });
        }
        self.expand_value(instance, expr, env, build_exports)
    }

    fn emit_stmt(
        &mut self,
        instance: &ConstructorInstance,
        stmt: &Ast,
        env: &HashMap<LocalVarId, RuntimeValue>,
        build_exports: &mut BuildExports,
    ) -> Result<(), EmitError> {
        let ty = match &stmt.ty {
            AstNode::Assignment { lhs, size, rhs } => {
                let rhs = self.expand_expr(instance, rhs, env, build_exports)?;
                match self.resolve_lvalue(instance, lhs, env, build_exports)? {
                    RuntimeValue::Expr(Expression {
                        ty: ExpressionTy::Ident(ident),
                        ..
                    }) => AstNode::Assignment {
                        lhs: ident,
                        size: *size,
                        rhs,
                    },
                    RuntimeValue::Address { ptr, space, size } => AstNode::LoadAssignment {
                        lhs: Load {
                            space: Some(SpaceRef::Resolved(space)),
                            size: Some(size),
                            ptr: Box::new(ptr),
                        },
                        size: Some(size),
                        rhs,
                    },
                    RuntimeValue::Expr(_) => AstNode::Expression(rhs),
                }
            }
            AstNode::LoadAssignment { lhs, size, rhs } => AstNode::LoadAssignment {
                lhs: Load {
                    space: lhs.space.clone(),
                    size: lhs.size,
                    ptr: Box::new(self.expand_expr(instance, &lhs.ptr, env, build_exports)?),
                },
                size: *size,
                rhs: self.expand_expr(instance, rhs, env, build_exports)?,
            },
            AstNode::RangeAssignment { lhs, size, rhs } => AstNode::RangeAssignment {
                lhs: Range {
                    value: Box::new(self.expand_expr(instance, &lhs.value, env, build_exports)?),
                    start: self.expand_range_param(&lhs.start, env)?,
                    size: self.expand_range_param(&lhs.size, env)?,
                },
                size: *size,
                rhs: self.expand_expr(instance, rhs, env, build_exports)?,
            },
            AstNode::Build(table_id) => {
                let export = self.emit_child_scoped(instance, *table_id, env)?;
                if let Some(export) = export {
                    build_exports.insert(*table_id, export);
                }
                return Ok(());
            }
            AstNode::DeferredBuild(name) => {
                return Err(EmitError::new(format!(
                    "unresolved `build {name}` reached runtime"
                )));
            }
            AstNode::DelaySlot(_) => {
                // The directive splices the delayed instructions' p-code where
                // it stands, not at the end: the manual's `beq` shape reads the
                // compared registers before the delayed instruction is free to
                // clobber them.
                for delayed in self.delay_slots {
                    self.emit_delay_slot(delayed)?;
                }
                return Ok(());
            }
            AstNode::Label(name) => AstNode::Label(self.scoped_label(name)),
            AstNode::Branch { target } => AstNode::Branch {
                target: {
                    let target = self.expand_target(instance, target, env, build_exports)?;
                    self.scoped_target(target)
                },
            },
            AstNode::ConditionalBranch { condition, target } => AstNode::ConditionalBranch {
                condition: self.expand_expr(instance, condition, env, build_exports)?,
                target: {
                    let target = self.expand_target(instance, target, env, build_exports)?;
                    self.scoped_target(target)
                },
            },
            AstNode::BranchIndirect { target } => AstNode::BranchIndirect {
                target: self.expand_expr(instance, target, env, build_exports)?,
            },
            AstNode::Call { target } => AstNode::Call {
                target: self.expand_target(instance, target, env, build_exports)?,
            },
            AstNode::CallIndirect { target } => AstNode::CallIndirect {
                target: self.expand_expr(instance, target, env, build_exports)?,
            },
            AstNode::Return { target } => AstNode::Return {
                target: self.expand_expr(instance, target, env, build_exports)?,
            },
            AstNode::Expression(expr) => {
                if let ExpressionTy::MacroCall { id, args } = &expr.ty {
                    self.emit_macro_call(instance, *id, args, env, build_exports)?;
                    return Ok(());
                }
                AstNode::Expression(self.expand_expr(instance, expr, env, build_exports)?)
            }
            AstNode::Export(_) => return Ok(()),
        };
        self.stmts.push(PcodeStatement { ty, span: () });
        Ok(())
    }

    fn expand_expr(
        &mut self,
        instance: &ConstructorInstance,
        expr: &Expression,
        env: &HashMap<LocalVarId, RuntimeValue>,
        build_exports: &mut BuildExports,
    ) -> Result<Expression, EmitError> {
        Ok(self
            .expand_value(instance, expr, env, build_exports)?
            .into_expr())
    }

    fn expand_value(
        &mut self,
        instance: &ConstructorInstance,
        expr: &Expression,
        env: &HashMap<LocalVarId, RuntimeValue>,
        build_exports: &mut BuildExports,
    ) -> Result<RuntimeValue, EmitError> {
        let ty = match &expr.ty {
            ExpressionTy::SizedInt { .. } => return Ok(RuntimeValue::Expr(expr.clone())),

            ExpressionTy::Ident(Ident::Named(id)) => {
                if let Some(value) = env.get(id).cloned() {
                    return Ok(value);
                }
                return Ok(RuntimeValue::Expr(Expression {
                    ty: ExpressionTy::Ident(Ident::Named(LocalVarId(self.current_base + id.0))),
                    size: None,
                    span: (),
                }));
            }

            ExpressionTy::Ident(Ident::Field(field_id)) => {
                return Ok(collect::resolve_field_ident(
                    self.spec,
                    *field_id,
                    instance,
                    self.inst_next,
                    self.inst_next2,
                ));
            }

            ExpressionTy::Ident(Ident::Table(table_id)) => {
                return Ok(build_exports
                    .get(table_id)
                    .cloned()
                    .unwrap_or_else(|| RuntimeValue::Expr(expr.clone())));
            }

            ExpressionTy::Ident(Ident::Global(name)) => {
                return Err(EmitError::new(format!(
                    "unresolved global `{name}` reached runtime"
                )));
            }

            ExpressionTy::Ident(_) => return Ok(RuntimeValue::Expr(expr.clone())),

            ExpressionTy::SubPieceMsb { src, count } => ExpressionTy::SubPieceMsb {
                src: Box::new(self.expand_expr(instance, src, env, build_exports)?),
                count: *count,
            },
            ExpressionTy::SubPieceLsb { src, count } => ExpressionTy::SubPieceLsb {
                src: Box::new(self.expand_expr(instance, src, env, build_exports)?),
                count: *count,
            },
            ExpressionTy::Load(load) => ExpressionTy::Load(Load {
                space: load.space.clone(),
                size: load.size,
                ptr: Box::new(self.expand_expr(instance, &load.ptr, env, build_exports)?),
            }),
            ExpressionTy::Range(range) => ExpressionTy::Range(Range {
                value: Box::new(self.expand_expr(instance, &range.value, env, build_exports)?),
                start: self.expand_range_param(&range.start, env)?,
                size: self.expand_range_param(&range.size, env)?,
            }),
            ExpressionTy::FunctionCall { builtin, args } => ExpressionTy::FunctionCall {
                builtin: *builtin,
                args: self.expand_exprs(instance, args, env, build_exports)?,
            },
            ExpressionTy::PcodeOp { id, args } => ExpressionTy::PcodeOp {
                id: *id,
                args: self.expand_exprs(instance, args, env, build_exports)?,
            },
            ExpressionTy::MacroCall { id, args } => {
                let export = self.emit_macro_call(instance, *id, args, env, build_exports)?;
                return export.ok_or_else(|| {
                    EmitError::new("p-code macro used as an expression did not export a value")
                });
            }
            ExpressionTy::DeferredCall { name, .. } => {
                return Err(EmitError::new(format!(
                    "unresolved deferred p-code call `{name}`"
                )));
            }
            ExpressionTy::Unop(unop) => ExpressionTy::Unop(crate::pmacro::expression::Unop {
                op: unop.op,
                e: Box::new(self.expand_expr(instance, &unop.e, env, build_exports)?),
            }),
            ExpressionTy::Binop(binop) => ExpressionTy::Binop(crate::pmacro::expression::Binop {
                op: binop.op,
                lhs: Box::new(self.expand_expr(instance, &binop.lhs, env, build_exports)?),
                rhs: Box::new(self.expand_expr(instance, &binop.rhs, env, build_exports)?),
            }),
        };
        let mut expanded = Expression {
            ty,
            size: expr.size,
            span: (),
        };
        infer_expr_size(self.spec, &mut expanded);
        Ok(RuntimeValue::Expr(expanded))
    }

    fn expand_exprs(
        &mut self,
        instance: &ConstructorInstance,
        args: &[Expression],
        env: &HashMap<LocalVarId, RuntimeValue>,
        build_exports: &mut BuildExports,
    ) -> Result<Vec<Expression>, EmitError> {
        args.iter()
            .map(|arg| self.expand_expr(instance, arg, env, build_exports))
            .collect()
    }

    fn expand_target(
        &mut self,
        instance: &ConstructorInstance,
        target: &LabelOrNode,
        env: &HashMap<LocalVarId, RuntimeValue>,
        build_exports: &mut BuildExports,
    ) -> Result<LabelOrNode, EmitError> {
        Ok(match target {
            LabelOrNode::Label(name) => LabelOrNode::Label(name.clone()),
            LabelOrNode::Node(name) => self.resolve_named_target(instance, name, build_exports)?,
            LabelOrNode::Expr(expr) => {
                LabelOrNode::Expr(self.expand_expr(instance, expr, env, build_exports)?)
            }
        })
    }

    fn expand_range_param(
        &self,
        param: &RangeParam,
        env: &HashMap<LocalVarId, RuntimeValue>,
    ) -> Result<RangeParam, EmitError> {
        match param {
            RangeParam::Literal(n) => Ok(RangeParam::Literal(*n)),
            RangeParam::MacroArg(id) => match env.get(id) {
                Some(RuntimeValue::Expr(Expression {
                    ty: ExpressionTy::SizedInt { value, .. },
                    ..
                })) => Ok(RangeParam::Literal(*value as usize)),
                Some(_) => Err(EmitError::new(format!(
                    "range macro argument `v{}` is not a literal",
                    id.0
                ))),
                None => Ok(RangeParam::MacroArg(*id)),
            },
        }
    }

    fn resolve_lvalue(
        &mut self,
        instance: &ConstructorInstance,
        ident: &Ident,
        env: &HashMap<LocalVarId, RuntimeValue>,
        build_exports: &mut BuildExports,
    ) -> Result<RuntimeValue, EmitError> {
        Ok(match ident {
            Ident::Register(_) | Ident::BitRange(_) => RuntimeValue::Expr(Expression {
                ty: ExpressionTy::Ident(ident.clone()),
                size: ident_size(self.spec, ident),
                span: (),
            }),
            Ident::Named(id) => {
                if let Some(value) = env.get(id) {
                    value.clone()
                } else {
                    RuntimeValue::Expr(Expression {
                        ty: ExpressionTy::Ident(Ident::Named(LocalVarId(self.current_base + id.0))),
                        size: None,
                        span: (),
                    })
                }
            }
            Ident::Field(field_id) => collect::resolve_field_ident(
                self.spec,
                *field_id,
                instance,
                self.inst_next,
                self.inst_next2,
            ),
            Ident::Table(table_id) => {
                build_exports
                    .get(table_id)
                    .cloned()
                    .unwrap_or(RuntimeValue::Expr(Expression {
                        ty: ExpressionTy::Ident(Ident::Table(*table_id)),
                        size: None,
                        span: (),
                    }))
            }
            Ident::Global(name) => {
                return Err(EmitError::new(format!(
                    "unresolved global `{name}` reached runtime"
                )));
            }
        })
    }

    fn resolve_named_target(
        &mut self,
        instance: &ConstructorInstance,
        name: &str,
        build_exports: &mut BuildExports,
    ) -> Result<LabelOrNode, EmitError> {
        match self.spec.symbols.get(name).copied() {
            Some(SymbolId::Field(field_id)) => Ok(LabelOrNode::Expr(
                collect::resolve_field_ident(
                    self.spec,
                    field_id,
                    instance,
                    self.inst_next,
                    self.inst_next2,
                )
                .into_direct_target_expr(),
            )),
            Some(SymbolId::Table(table_id)) => {
                if let Some(value) = build_exports.get(&table_id) {
                    Ok(LabelOrNode::Expr(value.clone().into_direct_target_expr()))
                } else {
                    Ok(LabelOrNode::Node(name.into()))
                }
            }
            _ => Ok(LabelOrNode::Node(name.into())),
        }
    }

    /// Emit a child subtable with a fresh ID allocation scope.
    pub(super) fn emit_child_scoped(
        &mut self,
        instance: &ConstructorInstance,
        table_id: TableId,
        env: &HashMap<LocalVarId, RuntimeValue>,
    ) -> Result<Option<RuntimeValue>, EmitError> {
        let child = instance.child_value(self.spec, table_id).ok_or_else(|| {
            EmitError::new(format!(
                "failed to build semantic operand (table {}): no matched child constructor",
                usize::from(table_id)
            ))
        })?;

        let child_local_count = child.constructor(self.spec).pmacro.local_var_count;
        let child_base = self.next_var_id;
        self.next_var_id += child_local_count;

        let old_base = std::mem::replace(&mut self.current_base, child_base);
        let result = self.emit_instance(child, env);
        self.current_base = old_base;
        result
    }
}
