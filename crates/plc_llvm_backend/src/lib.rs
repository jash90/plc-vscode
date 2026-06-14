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

use inkwell::AddressSpace;
use inkwell::builder::Builder;
use inkwell::context::Context;
use inkwell::module::Module;
use inkwell::types::BasicTypeEnum;
use inkwell::values::{IntValue, PointerValue};

use plc_hir::{BinaryOp, HirExpr, HirModule, HirPouKind, HirType, lower_source};

/// Lower Structured Text source to LLVM IR text.
pub fn emit_ir_from_source(text: &str) -> String {
    emit_ir(&lower_source(text))
}

/// Lower a HIR module to LLVM IR text.
pub fn emit_ir(module: &HirModule) -> String {
    let context = Context::create();
    let llvm_module = build_llvm_module(&context, module);
    llvm_module.print_to_string().to_string()
}

/// Backend output artifact modes.
///
/// `LlvmIr` and `Assembly` are textual; `Object` and the linkable artifacts
/// (`StaticLibrary`, `SharedLibrary`, `Executable`) emit machine code. The
/// linkable modes share object emission as their compile step; producing the
/// final archive/shared-object/executable is a subsequent link step (the object
/// bytes returned here are the linker input). Shared output is compiled
/// position-independent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputMode {
    LlvmIr,
    Assembly,
    Object,
    StaticLibrary,
    SharedLibrary,
    Executable,
}

/// Compile a HIR module to the requested output mode, returning the bytes.
///
/// Native modes target the host triple; cross-compilation is documented in
/// `docs/architecture/llvm-toolchain.md` (set the target triple/CPU features).
pub fn compile(module: &HirModule, mode: OutputMode) -> Result<Vec<u8>, String> {
    use inkwell::OptimizationLevel;
    use inkwell::targets::{
        CodeModel, FileType, InitializationConfig, RelocMode, Target, TargetMachine,
    };

    let context = Context::create();
    let llvm_module = build_llvm_module(&context, module);

    if mode == OutputMode::LlvmIr {
        return Ok(llvm_module.print_to_string().to_bytes().to_vec());
    }

    Target::initialize_native(&InitializationConfig::default())
        .map_err(|err| format!("failed to initialize native target: {err}"))?;
    let triple = TargetMachine::get_default_triple();
    let target =
        Target::from_triple(&triple).map_err(|err| format!("unknown target triple: {err}"))?;

    // Shared libraries require position-independent code.
    let reloc = match mode {
        OutputMode::SharedLibrary => RelocMode::PIC,
        _ => RelocMode::Default,
    };

    let machine = target
        .create_target_machine(
            &triple,
            TargetMachine::get_host_cpu_name().to_str().unwrap_or(""),
            TargetMachine::get_host_cpu_features()
                .to_str()
                .unwrap_or(""),
            OptimizationLevel::Default,
            reloc,
            CodeModel::Default,
        )
        .ok_or_else(|| "failed to create target machine".to_owned())?;

    let file_type = match mode {
        OutputMode::Assembly => FileType::Assembly,
        _ => FileType::Object,
    };

    let buffer = machine
        .write_to_memory_buffer(&llvm_module, file_type)
        .map_err(|err| format!("failed to emit code: {err}"))?;
    Ok(buffer.as_slice().to_vec())
}

fn build_llvm_module<'ctx>(context: &'ctx Context, module: &HirModule) -> Module<'ctx> {
    let llvm_module = context.create_module("plc");
    let builder = context.create_builder();

    for program in &module.programs {
        if program.kind == HirPouKind::FunctionBlock {
            emit_function_block(context, &llvm_module, &builder, program);
        } else {
            emit_program(context, &llvm_module, &builder, program);
        }
    }

    llvm_module
}

/// Lower a FUNCTION_BLOCK so its state persists across calls: the instance
/// variables become fields of a named struct, and the body is emitted as a
/// `<name>_run(ptr %self)` function that reads/writes those fields via GEP.
fn emit_function_block<'ctx>(
    context: &'ctx Context,
    module: &Module<'ctx>,
    builder: &Builder<'ctx>,
    program: &plc_hir::HirProgram,
) {
    let i64_type = context.i64_type();

    // Named state struct: one i64 field per instance variable.
    let struct_ty = context.opaque_struct_type(&format!("FB_{}", program.name));
    let field_types: Vec<BasicTypeEnum> = program.vars.iter().map(|_| i64_type.into()).collect();
    struct_ty.set_body(&field_types, false);

    let field_index: HashMap<String, u32> = program
        .vars
        .iter()
        .enumerate()
        .map(|(index, var)| (var.name.to_ascii_lowercase(), index as u32))
        .collect();

    // void @<name>_run(ptr %self)
    let ptr_ty = context.ptr_type(AddressSpace::default());
    let fn_type = context.void_type().fn_type(&[ptr_ty.into()], false);
    let function = module.add_function(&format!("{}_run", program.name), fn_type, None);
    let entry = context.append_basic_block(function, "entry");
    builder.position_at_end(entry);
    let self_ptr = function
        .get_nth_param(0)
        .expect("self parameter")
        .into_pointer_value();

    for assign in &program.body {
        let value = eval_fb_int(
            context,
            builder,
            struct_ty,
            self_ptr,
            &field_index,
            &assign.value,
        );
        if let Some(index) = field_index.get(&assign.target.to_ascii_lowercase()) {
            let field = builder
                .build_struct_gep(struct_ty, self_ptr, *index, &assign.target)
                .expect("struct gep succeeds");
            builder.build_store(field, value).expect("store succeeds");
        }
    }

    builder.build_return(None).expect("return succeeds");
}

fn eval_fb_int<'ctx>(
    context: &'ctx Context,
    builder: &Builder<'ctx>,
    struct_ty: inkwell::types::StructType<'ctx>,
    self_ptr: PointerValue<'ctx>,
    field_index: &HashMap<String, u32>,
    expr: &HirExpr,
) -> IntValue<'ctx> {
    let i64_type = context.i64_type();
    match expr {
        HirExpr::Int(value) => i64_type.const_int(*value as u64, true),
        HirExpr::Bool(value) => i64_type.const_int(u64::from(*value), false),
        HirExpr::Real(value) => i64_type.const_int(*value as i64 as u64, true),
        HirExpr::Str(_) => i64_type.const_zero(),
        HirExpr::Var(name) => {
            if let Some(index) = field_index.get(&name.to_ascii_lowercase()) {
                let field = builder
                    .build_struct_gep(struct_ty, self_ptr, *index, name)
                    .expect("struct gep succeeds");
                builder
                    .build_load(i64_type, field, "load")
                    .expect("load succeeds")
                    .into_int_value()
            } else {
                i64_type.const_zero()
            }
        }
        HirExpr::Binary { op, lhs, rhs } => {
            let left = eval_fb_int(context, builder, struct_ty, self_ptr, field_index, lhs);
            let right = eval_fb_int(context, builder, struct_ty, self_ptr, field_index, rhs);
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
