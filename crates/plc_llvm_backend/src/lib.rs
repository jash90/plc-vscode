//! LLVM IR backend prototype via `inkwell`.
//!
//! Lowers backend-agnostic [`plc_hir`] modules into LLVM IR. This MVP models
//! every program (POU) as a `void` function over `i64` locals: declared
//! variables become `alloca`s, assignments become `store`s, and integer
//! expressions lower to `add`/`sub`. The textual IR is returned so it can be
//! golden-tested without a JIT.
//!
//! Requires an LLVM 18.x toolchain (see `docs/architecture/llvm-toolchain.md`).

use std::collections::HashMap;

use inkwell::builder::Builder;
use inkwell::context::Context;
use inkwell::module::Module;
use inkwell::values::{IntValue, PointerValue};

use plc_hir::{BinaryOp, HirExpr, HirModule, HirType, lower_source};

/// Lower Structured Text source to LLVM IR text.
pub fn emit_ir_from_source(text: &str) -> String {
    emit_ir(&lower_source(text))
}

/// Lower a HIR module to LLVM IR text.
pub fn emit_ir(module: &HirModule) -> String {
    let context = Context::create();
    let llvm_module = context.create_module("plc");
    let builder = context.create_builder();

    for program in &module.programs {
        emit_program(&context, &llvm_module, &builder, program);
    }

    llvm_module.print_to_string().to_string()
}

fn emit_program<'ctx>(
    context: &'ctx Context,
    module: &Module<'ctx>,
    builder: &Builder<'ctx>,
    program: &plc_hir::HirProgram,
) {
    let i64_type = context.i64_type();
    let fn_type = context.void_type().fn_type(&[], false);
    let function = module.add_function(&program.name, fn_type, None);
    let entry = context.append_basic_block(function, "entry");
    builder.position_at_end(entry);

    let mut slots: HashMap<String, PointerValue> = HashMap::new();

    // Allocate and zero-initialize integer-typed locals.
    for var in &program.vars {
        if var.ty == HirType::Int || var.ty == HirType::Bool {
            let slot = builder
                .build_alloca(i64_type, &var.name)
                .expect("alloca succeeds");
            builder
                .build_store(slot, i64_type.const_zero())
                .expect("store succeeds");
            slots.insert(var.name.to_ascii_lowercase(), slot);
        }
    }

    for assign in &program.body {
        let value = eval_int(context, builder, &mut slots, &assign.value);
        let slot = *slots
            .entry(assign.target.to_ascii_lowercase())
            .or_insert_with(|| {
                let slot = builder
                    .build_alloca(i64_type, &assign.target)
                    .expect("alloca succeeds");
                builder
                    .build_store(slot, i64_type.const_zero())
                    .expect("store succeeds");
                slot
            });
        builder.build_store(slot, value).expect("store succeeds");
    }

    builder.build_return(None).expect("return succeeds");
}

fn eval_int<'ctx>(
    context: &'ctx Context,
    builder: &Builder<'ctx>,
    slots: &mut HashMap<String, PointerValue<'ctx>>,
    expr: &HirExpr,
) -> IntValue<'ctx> {
    let i64_type = context.i64_type();
    match expr {
        HirExpr::Int(value) => i64_type.const_int(*value as u64, true),
        HirExpr::Bool(value) => i64_type.const_int(u64::from(*value), false),
        HirExpr::Real(value) => i64_type.const_int(*value as i64 as u64, true),
        HirExpr::Str(_) => i64_type.const_zero(),
        HirExpr::Var(name) => {
            let slot = *slots.entry(name.to_ascii_lowercase()).or_insert_with(|| {
                let slot = builder
                    .build_alloca(i64_type, name)
                    .expect("alloca succeeds");
                builder
                    .build_store(slot, i64_type.const_zero())
                    .expect("store succeeds");
                slot
            });
            builder
                .build_load(i64_type, slot, "load")
                .expect("load succeeds")
                .into_int_value()
        }
        HirExpr::Binary { op, lhs, rhs } => {
            let left = eval_int(context, builder, slots, lhs);
            let right = eval_int(context, builder, slots, rhs);
            match op {
                BinaryOp::Add => builder
                    .build_int_add(left, right, "add")
                    .expect("add succeeds"),
                BinaryOp::Sub => builder
                    .build_int_sub(left, right, "sub")
                    .expect("sub succeeds"),
            }
        }
    }
}
