use crate::{
    instance::ConstructorInstance,
    objects::field::{
        FIELD_INST_NEXT, FIELD_INST_NEXT2, FIELD_INST_START, FieldId, FieldParent, FieldValue,
    },
    pmacro::expression::{Expression, ExpressionTy, Ident},
    runtime::pcode::{RuntimeValue, make_int_expr},
    spec::Spec,
};

/// Resolve the runtime value of a field from a constructor instance, if known.
pub(super) fn resolve_field_value(
    spec: &Spec,
    instance: &ConstructorInstance,
    field_id: FieldId,
) -> Option<RuntimeValue> {
    let constructor = instance.constructor(spec);
    let field = &spec.fields[field_id];

    let (raw_value, size) = if field.parent == FieldParent::Global {
        constructor.global_map.get(&field_id).map(|idx| {
            (
                instance.global_values[*idx as usize],
                spec.spaces[spec.default_space].addr_size,
            )
        })
    } else {
        instance.field_value(spec, field_id).map(|value| {
            let size = if field.signed {
                spec.spaces[spec.default_space].addr_size
            } else {
                field.range.size().div_ceil(8)
            };
            (value as u64, size)
        })
    }?;

    let expr = match field.value(&spec.field_tables, raw_value)? {
        FieldValue::Int(value) => make_int_expr(value as u64, size),
        FieldValue::UInt(value) => make_int_expr(value, size),
        FieldValue::Register(register_id) => Expression {
            ty: ExpressionTy::Ident(Ident::Register(register_id)),
            size: Some(spec.registers[register_id].size),
            span: (),
        },
        FieldValue::String(_) => return None,
    };
    Some(RuntimeValue::Expr(expr))
}

/// Resolve inst_start / inst_next / inst_next2 / arbitrary field to a value.
///
/// `inst_next` is passed in rather than read off `instance`: the semantic
/// section sees the address past the delay slot, while the instance carries the
/// unextended value that disassembly actions and `globalset` use.
pub(super) fn resolve_field_ident(
    spec: &Spec,
    field_id: FieldId,
    instance: &ConstructorInstance,
    inst_next: u64,
    inst_next2: Option<u64>,
) -> RuntimeValue {
    // `inst_start`/`inst_next` are code addresses: size them to the target's
    // address width (8 on x86-64, 4 on i386), matching how `resolve_field_value`
    // sizes global address fields. Hardcoding 8 produced an 8-byte return-address
    // literal on i386, which a 4-byte `&:4 inst_next` push could not narrow, so
    // mem2reg refused to promote the (mixed-width) return-address slot.
    let addr_size = spec.spaces[spec.default_space].addr_size;
    if field_id == FIELD_INST_START {
        return RuntimeValue::Expr(make_int_expr(instance.inst_start, addr_size));
    }
    if field_id == FIELD_INST_NEXT {
        return RuntimeValue::Expr(make_int_expr(inst_next, addr_size));
    }
    if field_id == FIELD_INST_NEXT2
        && let Some(inst_next2) = inst_next2
    {
        return RuntimeValue::Expr(make_int_expr(inst_next2, addr_size));
    }
    resolve_field_value(spec, instance, field_id).unwrap_or(RuntimeValue::Expr(Expression {
        ty: ExpressionTy::Ident(Ident::Field(field_id)),
        size: None,
        span: (),
    }))
}
