//! Lower the Structured Text AST ([`plc_runtime::ast`]) to CPDev symbolic
//! assembly.
//!
//! The VM is a flat-code / flat-data three-address machine with no immediate
//! arithmetic operands, so lowering:
//! - assigns every variable / constant / temporary a data-segment slot,
//! - materializes each literal via `MCD` (in an `INIT` routine that runs once),
//! - lowers expression trees post-order into three-address ops writing temps, and
//! - lowers control flow to `JZ`/`JNZ`/`JMP` over generated labels (the assembler
//!   backpatches them).
//!
//! Program shape (one PROGRAM bound to a cyclic task):
//! ```text
//! TSKSTR:  CALB #0000, PROG_INIT   ; seed vars + consts, init FB instances (once)
//! TSKLOOP: CALB #0000, PROG_CODE   ; the cyclic body (TRML returns here)
//!          TRML TSKLOOP
//! PROG_INIT: <MCD seeds> <CALB #base, FB_INIT per instance> RETURN
//! PROG_CODE: <lowered statements> RETURN
//! FB_<t>_INIT: <frame-relative MCD seeds> RETURN
//! FB_<t>_CODE: <frame-relative body>       RETURN
//! ```
//!
//! **Function blocks** use the VM's data-offset model: a `CALB #base, target`
//! adds `base` to `wDataOfs` for the callee, so an instance's frame-relative
//! locals resolve to `pgmData[base + offset]`. Each instance is a contiguous
//! sub-frame in the program's data segment; a call copies inputs into the frame,
//! runs `CALB #base, FB_CODE`, and `inst.out` reads resolve to `base + offset`.
//! Two instances get distinct `base`s, hence independent state. (FB-in-FB
//! nesting is not supported yet.)

use std::collections::HashMap;

use plc_runtime::Value;
use plc_runtime::ast::{
    BinOp, CallArg, CaseLabel, Expr, PouKind, Stmt, StmtKind, UnOp, Unit, VarDecl, build_units,
};

use crate::asm::{Instr, Item, Operand, Program as AsmProgram};
use crate::layout::{DEFAULT_DATA_CAP, DataLayout};
use crate::spec::SpecTable;
use crate::types::CpType;

/// A program lowered to assembly plus the `.DCP` metadata.
pub struct Compiled {
    pub program: AsmProgram,
    /// Watchable globals (the program's declared scalar variables), declared order.
    pub globals: Vec<GlobalVar>,
    /// Total data-segment size in bytes.
    pub data_size: u16,
    /// The program's name (for `.DCP` qualified names).
    pub prog_name: String,
}

/// A declared, watchable global variable.
pub struct GlobalVar {
    pub name: String,
    pub addr: u16,
    pub ty: CpType,
}

/// Parse ST source and lower it to CPDev assembly + metadata.
pub fn compile_source(text: &str, spec: &SpecTable) -> Result<Compiled, String> {
    let units = build_units(text);
    let program = units
        .iter()
        .find(|u| u.kind == PouKind::Program)
        .or_else(|| units.first())
        .ok_or("source has no POU to compile")?;

    // The IEC standard function blocks (TON/CTU/...) are synthesized as ordinary
    // ST FB definitions and injected on demand, so they lower through the same
    // machinery as user FBs (no special-casing the bytecode). Only the ones the
    // program/user-FBs actually instantiate are pulled in.
    let std_units = crate::std_fb::units();
    let user_fb_names: std::collections::HashSet<String> = units
        .iter()
        .filter(|u| u.kind == PouKind::FunctionBlock)
        .map(|u| u.name.to_ascii_lowercase())
        .collect();
    let std_names: std::collections::HashSet<String> = std_units
        .iter()
        .map(|u| u.name.to_ascii_lowercase())
        .collect();
    let mut needed: std::collections::HashSet<String> = std::collections::HashSet::new();
    for unit in &units {
        for var in &unit.vars {
            let ty = var.type_name.to_ascii_lowercase();
            // A user definition of the same name takes precedence (no injection).
            if std_names.contains(&ty) && !user_fb_names.contains(&ty) {
                needed.insert(ty);
            }
        }
    }

    // FB types to lower: the user's, plus the referenced standard ones.
    let mut fb_units: Vec<&Unit> = units
        .iter()
        .filter(|u| u.kind == PouKind::FunctionBlock)
        .collect();
    fb_units.extend(
        std_units
            .iter()
            .filter(|u| needed.contains(&u.name.to_ascii_lowercase())),
    );

    // Lower every FB type (frame-relative, base 0) in dependency order, so a
    // block that instantiates another is lowered after its callee and can size
    // that callee's nested sub-frame.
    let mut fb_types: Vec<FbType> = Vec::new();
    for &idx in &topo_order_fbs(&fb_units)? {
        // `fb_types` already holds every dependency of `fb_units[idx]`.
        fb_types.push(FbType::lower(fb_units[idx], spec, &fb_types)?);
    }

    ProgramGen::new(spec, &fb_types)?.run(program)
}

/// Order function-block types so each appears after the blocks it instantiates
/// (dependencies first). Errors on a cyclic or self-referential instantiation,
/// which would imply an infinite data frame.
fn topo_order_fbs(fbs: &[&Unit]) -> Result<Vec<usize>, String> {
    let index: HashMap<String, usize> = fbs
        .iter()
        .enumerate()
        .map(|(i, u)| (u.name.to_ascii_lowercase(), i))
        .collect();
    let mut state = vec![Visit::Unseen; fbs.len()];
    let mut order = Vec::with_capacity(fbs.len());
    for i in 0..fbs.len() {
        visit_fb(i, fbs, &index, &mut state, &mut order)?;
    }
    Ok(order)
}

#[derive(Clone, Copy, PartialEq)]
enum Visit {
    Unseen,
    Active,
    Done,
}

fn visit_fb(
    i: usize,
    fbs: &[&Unit],
    index: &HashMap<String, usize>,
    state: &mut [Visit],
    order: &mut Vec<usize>,
) -> Result<(), String> {
    match state[i] {
        Visit::Done => return Ok(()),
        Visit::Active => {
            return Err(format!(
                "function block `{}` is part of a cyclic instantiation",
                fbs[i].name
            ));
        }
        Visit::Unseen => {}
    }
    state[i] = Visit::Active;
    for var in &fbs[i].vars {
        if let Some(&dep) = index.get(&var.type_name.to_ascii_lowercase()) {
            if dep == i {
                return Err(format!(
                    "function block `{}` cannot instantiate itself",
                    fbs[i].name
                ));
            }
            visit_fb(dep, fbs, index, state, order)?;
        }
    }
    state[i] = Visit::Done;
    order.push(i);
    Ok(())
}

// ---------------------------------------------------------------------------
// literal & type helpers
// ---------------------------------------------------------------------------

fn cp_type_of_name(name: &str) -> Result<CpType, String> {
    CpType::from_name(name).ok_or_else(|| format!("unsupported variable type `{name}`"))
}

/// Resolve a declared variable's CPDev type, taking a `STRING[N]` capacity from
/// its sizing clause (`type_size`) and defaulting unsized `STRING` to 80.
fn cp_type_of_var(var: &VarDecl) -> Result<CpType, String> {
    if var.type_name.eq_ignore_ascii_case("STRING") || var.type_name.eq_ignore_ascii_case("WSTRING")
    {
        return Ok(CpType::Str(parse_str_cap(var.type_size.as_deref())?));
    }
    cp_type_of_name(&var.type_name)
}

/// Parse a `STRING` sizing clause (`"[80]"`) into a capacity. The whole slot is
/// `4 + capacity` bytes and is initialized by a single `MCD`, whose length
/// operand is one byte, so the capacity is bounded to keep `4 + cap <= 255`.
fn parse_str_cap(type_size: Option<&str>) -> Result<u16, String> {
    let Some(spec) = type_size else {
        return Ok(crate::types::DEFAULT_STR_CAP);
    };
    let digits = spec
        .trim()
        .trim_start_matches('[')
        .trim_end_matches(']')
        .trim();
    let cap: u16 = digits
        .parse()
        .map_err(|_| format!("invalid STRING size `{spec}`"))?;
    if cap == 0 || cap > 251 {
        return Err(format!("STRING capacity {cap} out of range 1..=251"));
    }
    Ok(cap)
}

/// Build the inline CPDev STRING image: `[length][chars_size][padding:2]` then
/// the characters padded to `cap`. Total length is `4 + cap`.
fn string_image(text: &str, cap: u16) -> Vec<u8> {
    let cap = cap as usize;
    let chars = text.as_bytes();
    let length = chars.len().min(cap).min(255);
    let mut buf = vec![0u8; 4 + cap];
    buf[0] = length as u8;
    buf[1] = cap.min(255) as u8;
    // buf[2..4] padding stays zero.
    buf[4..4 + length].copy_from_slice(&chars[..length]);
    buf
}

fn value_cp(value: &Value) -> CpType {
    match value {
        Value::Bool(_) => CpType::Bool,
        Value::Int(_) | Value::Unknown => CpType::Int,
        Value::Real(_) => CpType::Real,
        Value::Time(_) => CpType::Time,
        Value::Str(s) => CpType::Str(s.len().min(251) as u16),
    }
}

/// Merge two operand types, letting a bare INT literal adopt the other side's
/// concrete type (e.g. `realVar > 1` is a REAL comparison).
fn unify(a: CpType, b: CpType) -> CpType {
    if a == b || a != CpType::Int { a } else { b }
}

fn int_le_bytes(value: i64, size: usize) -> Vec<u8> {
    value.to_le_bytes()[..size.clamp(1, 8)].to_vec()
}

fn encode_value(value: &Value, ty: CpType) -> Result<Vec<u8>, String> {
    match value {
        Value::Bool(b) => Ok(vec![u8::from(*b)]),
        Value::Int(v) => Ok(int_le_bytes(*v, ty.size())),
        Value::Time(ms) => Ok(int_le_bytes(*ms, ty.size())),
        Value::Real(r) => match ty {
            CpType::Real => Ok((*r as f32).to_le_bytes().to_vec()),
            CpType::Lreal => Ok(r.to_le_bytes().to_vec()),
            _ => Err(format!("real literal assigned to non-real type {ty:?}")),
        },
        Value::Str(s) => match ty {
            CpType::Str(cap) => Ok(string_image(s, cap)),
            _ => Err("string literal assigned to a non-string type".to_owned()),
        },
        Value::Unknown => Ok(vec![0u8; ty.size().max(1)]),
    }
}

fn is_comparison(op: BinOp) -> bool {
    matches!(
        op,
        BinOp::Eq | BinOp::Ne | BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge
    )
}

fn binop_name(op: BinOp) -> &'static str {
    match op {
        BinOp::Or => "OR",
        BinOp::Xor => "XOR",
        BinOp::And => "AND",
        BinOp::Eq => "EQ",
        BinOp::Ne => "NE",
        BinOp::Lt => "LT",
        BinOp::Le => "LE",
        BinOp::Gt => "GT",
        BinOp::Ge => "GE",
        BinOp::Add => "ADD",
        BinOp::Sub => "SUB",
        BinOp::Mul => "MUL",
        BinOp::Div => "DIV",
        BinOp::Mod => "MOD",
        BinOp::Pow => "EXPT",
    }
}

// ---------------------------------------------------------------------------
// function-block types
// ---------------------------------------------------------------------------

/// A lowered function-block type: a frame-relative INIT (seeds) + CODE (body),
/// the total frame size, its member offsets, and its inputs in declared order.
struct FbType {
    name: String,
    frame_size: u16,
    init: Vec<Item>,
    code: Vec<Item>,
    /// lowercased member name -> (frame offset, type)
    members: HashMap<String, (u16, CpType)>,
    /// lowercased nested-instance name -> (frame-relative base, fb type index).
    /// Lets `outer.inner.field` resolve through one level of nesting per hop.
    nested: HashMap<String, (u16, usize)>,
    /// (lowercased input name, type) in declared order, for positional calls.
    inputs: Vec<(String, CpType)>,
}

impl FbType {
    /// Lower one FB type to a frame-relative INIT + CODE routine. `deps` holds the
    /// already-lowered FB types this block may instantiate (its dependencies),
    /// so nested instances can be sized and called.
    fn lower(unit: &Unit, spec: &SpecTable, deps: &[FbType]) -> Result<Self, String> {
        let mut cg = BodyGen::new(spec, deps)?;

        // Scalar members first (stable offsets), then nested instance sub-frames.
        let mut scalar_decls = Vec::new();
        let mut inputs = Vec::new();
        for var in &unit.vars {
            if cg.is_fb_type(&var.type_name) {
                continue;
            }
            if var.is_fb {
                return Err(format!(
                    "function block `{}`: standard function block `{}` is not supported yet",
                    unit.name, var.type_name
                ));
            }
            let ty = cp_type_of_var(var)?;
            cg.declare_var(&var.name, ty)?;
            if var.is_input {
                inputs.push((var.name.to_ascii_lowercase(), ty));
            }
            scalar_decls.push(var.clone());
        }
        for var in &unit.vars {
            if cg.is_fb_type(&var.type_name) {
                cg.declare_instance(&var.name, &var.type_name)?;
            }
        }

        let mut init_seeds = cg.var_seed_items(&scalar_decls)?;
        init_seeds.extend(cg.instance_init_calls());
        cg.lower_block(&unit.body)?;
        let (init, code) = cg.finish_routine(init_seeds);

        let nested = cg
            .instances
            .iter()
            .map(|(name, inst)| (name.clone(), (inst.base, inst.fb_index)))
            .collect();
        Ok(FbType {
            name: unit.name.to_ascii_lowercase(),
            frame_size: cg.layout.size(),
            init,
            code,
            members: cg.vars,
            nested,
            inputs,
        })
    }
}

// ---------------------------------------------------------------------------
// program generation (stitches scaffold + program body + FB routines)
// ---------------------------------------------------------------------------

struct ProgramGen<'a> {
    spec: &'a SpecTable,
    fb_types: &'a [FbType],
}

impl<'a> ProgramGen<'a> {
    fn new(spec: &'a SpecTable, fb_types: &'a [FbType]) -> Result<Self, String> {
        Ok(Self { spec, fb_types })
    }

    fn run(self, program: &Unit) -> Result<Compiled, String> {
        let mut cg = BodyGen::new(self.spec, self.fb_types)?;

        // Declare scalar globals first (stable, watch-visible addresses).
        let mut globals = Vec::new();
        let mut scalar_decls = Vec::new();
        for var in &program.vars {
            if cg.is_fb_type(&var.type_name) {
                continue; // FB instances handled below
            }
            if var.is_fb {
                return Err(format!(
                    "variable `{}`: standard function block `{}` is not supported yet (user-defined FBs only)",
                    var.name, var.type_name
                ));
            }
            let ty = cp_type_of_var(var)?;
            let addr = cg.declare_var(&var.name, ty)?;
            scalar_decls.push(var.clone());
            globals.push(GlobalVar {
                name: var.name.clone(),
                addr,
                ty,
            });
        }

        // Allocate a data sub-frame for each FB instance, after the scalars.
        for var in &program.vars {
            if !cg.is_fb_type(&var.type_name) {
                continue;
            }
            cg.declare_instance(&var.name, &var.type_name)?;
        }

        // Seed scalar globals (init value or zero) at INIT.
        let mut init_seeds = cg.var_seed_items(&scalar_decls)?;
        // Then call each FB instance's INIT (frame-relative, via CALB #base).
        init_seeds.extend(cg.instance_init_calls());

        // Lower the program body.
        cg.lower_block(&program.body)?;

        let (prog_init, prog_code) = cg.finish_routine(init_seeds);
        let data_size = cg.layout.size();
        let prog_name = program.name.clone();

        // Stitch the final assembly program.
        let mut prog = AsmProgram::new();
        prog.label("TSKSTR");
        prog.push(cg.calb_instr(0, "PROG_INIT"));
        prog.label("TSKLOOP");
        prog.push(cg.calb_instr(0, "PROG_CODE"));
        prog.push(cg.trml_instr("TSKLOOP"));

        prog.label("PROG_INIT");
        prog.items.extend(prog_init);
        prog.push(cg.ret_instr());
        prog.label("PROG_CODE");
        prog.items.extend(prog_code);
        prog.push(cg.ret_instr());

        // Emit each FB type's INIT/CODE routines once (shared by all instances).
        for fb in self.fb_types {
            prog.label(format!("FB_{}_INIT", fb.name));
            prog.items.extend(fb.init.iter().cloned());
            prog.push(cg.ret_instr());
            prog.label(format!("FB_{}_CODE", fb.name));
            prog.items.extend(fb.code.iter().cloned());
            prog.push(cg.ret_instr());
        }

        Ok(Compiled {
            program: prog,
            globals,
            data_size,
            prog_name,
        })
    }
}

// ---------------------------------------------------------------------------
// body generation (shared by the program and each function block)
// ---------------------------------------------------------------------------

/// A declared FB instance: its frame base in the owner's data segment and its type.
struct Instance {
    base: u16,
    fb_index: usize,
}

/// A loop's continue / break targets, for `CONTINUE` / `EXIT`.
struct LoopCtx {
    continue_label: String,
    break_label: String,
}

struct BodyGen<'a> {
    spec: &'a SpecTable,
    fb_types: &'a [FbType],
    layout: DataLayout,
    /// lowercased variable name -> (data address, type)
    vars: HashMap<String, (u16, CpType)>,
    /// lowercased instance name -> instance frame
    instances: HashMap<String, Instance>,
    consts: Vec<(u16, Vec<u8>)>,
    const_index: HashMap<Vec<u8>, u16>,
    code: Vec<Item>,
    loops: Vec<LoopCtx>,
    label_counter: u32,
    // resolved sysproc opcodes
    mcd: u16,
    calb: u16,
    trml: u16,
    ret: u16,
    memcp: u16,
    jz: u16,
    jnz: u16,
    jmp: u16,
    cur_time: u16,
    strasgn: u16,
}

impl<'a> BodyGen<'a> {
    fn new(spec: &'a SpecTable, fb_types: &'a [FbType]) -> Result<Self, String> {
        let op = |name: &str| {
            spec.untyped(name)
                .map(|v| v.encode(0))
                .ok_or_else(|| format!("spec table is missing `{name}`"))
        };
        Ok(Self {
            spec,
            fb_types,
            layout: DataLayout::new(DEFAULT_DATA_CAP),
            vars: HashMap::new(),
            instances: HashMap::new(),
            consts: Vec::new(),
            const_index: HashMap::new(),
            code: Vec::new(),
            loops: Vec::new(),
            label_counter: 0,
            mcd: op("MCD")?,
            calb: op("CALB")?,
            trml: op("TRML")?,
            ret: op("RETURN")?,
            memcp: op("MEMCP")?,
            jz: op("JZ")?,
            jnz: op("JNZ")?,
            jmp: op("JMP")?,
            cur_time: op("CUR_TIME")?,
            strasgn: op("STRASGN")?,
        })
    }

    // -- declarations ------------------------------------------------------

    fn is_fb_type(&self, type_name: &str) -> bool {
        let lname = type_name.to_ascii_lowercase();
        self.fb_types.iter().any(|fb| fb.name == lname)
    }

    fn fb_index(&self, type_name: &str) -> Option<usize> {
        let lname = type_name.to_ascii_lowercase();
        self.fb_types.iter().position(|fb| fb.name == lname)
    }

    fn declare_var(&mut self, name: &str, ty: CpType) -> Result<u16, String> {
        let addr = self.layout.alloc(&name.to_ascii_lowercase(), ty.size())?;
        self.vars.insert(name.to_ascii_lowercase(), (addr, ty));
        Ok(addr)
    }

    /// Allocate an FB instance's contiguous data sub-frame.
    fn declare_instance(&mut self, name: &str, type_name: &str) -> Result<(), String> {
        let fb_index = self
            .fb_index(type_name)
            .ok_or_else(|| format!("unknown function block type `{type_name}`"))?;
        let frame = self.fb_types[fb_index].frame_size.max(1);
        let base = self.layout.alloc_anon(frame as usize)?;
        self.instances
            .insert(name.to_ascii_lowercase(), Instance { base, fb_index });
        Ok(())
    }

    /// `MCD` items that seed each declared scalar var (init value or zero).
    fn var_seed_items(&mut self, vars: &[VarDecl]) -> Result<Vec<Item>, String> {
        let mut items = Vec::new();
        for var in vars {
            let (addr, ty) = self.lookup(&var.name)?;
            let bytes = match ty {
                // A STRING slot must carry its header (length + chars_size) so the
                // VM reads its capacity; zero-filling would leave chars_size = 0.
                CpType::Str(cap) => {
                    let init = match &var.init {
                        Some(Value::Str(s)) => s.as_str(),
                        _ => "",
                    };
                    string_image(init, cap)
                }
                _ => match &var.init {
                    Some(value) => encode_value(value, ty)?,
                    None => vec![0u8; ty.size()],
                },
            };
            items.push(Item::Instr(self.mcd_instr(addr, &bytes)));
        }
        Ok(items)
    }

    /// `CALB #base, FB_<type>_INIT` for each declared instance.
    fn instance_init_calls(&self) -> Vec<Item> {
        let mut items = Vec::new();
        // Deterministic order: by frame base.
        let mut instances: Vec<&Instance> = self.instances.values().collect();
        instances.sort_by_key(|i| i.base);
        for inst in instances {
            let label = format!("FB_{}_INIT", self.fb_types[inst.fb_index].name);
            items.push(Item::Instr(self.calb_instr(inst.base, &label)));
        }
        items
    }

    /// Split the accumulated body into (INIT seeds, CODE) for a routine.
    fn finish_routine(&mut self, mut init_seeds: Vec<Item>) -> (Vec<Item>, Vec<Item>) {
        // Constant slots are seeded in INIT, after the variable seeds.
        for (addr, bytes) in std::mem::take(&mut self.consts) {
            init_seeds.push(Item::Instr(self.mcd_instr(addr, &bytes)));
        }
        let code = std::mem::take(&mut self.code);
        (init_seeds, code)
    }

    // -- statements --------------------------------------------------------

    fn lower_block(&mut self, body: &[Stmt]) -> Result<(), String> {
        for stmt in body {
            self.lower_stmt(stmt)?;
        }
        Ok(())
    }

    fn lower_stmt(&mut self, stmt: &Stmt) -> Result<(), String> {
        match &stmt.kind {
            StmtKind::Assign { target, value } => {
                let (addr, ty) = self.lvalue(target)?;
                self.eval_into(addr, ty, value)?;
            }
            StmtKind::If {
                branches,
                else_body,
            } => {
                let end = self.new_label();
                for (cond, body) in branches {
                    let next = self.new_label();
                    let c = self.operand(cond, CpType::Bool)?;
                    self.emit(self.jz_instr(c, &next));
                    self.lower_block(body)?;
                    self.emit(self.jmp_instr(&end));
                    self.bind(&next);
                }
                self.lower_block(else_body)?;
                self.bind(&end);
            }
            StmtKind::While { cond, body } => {
                let head = self.new_label();
                let end = self.new_label();
                self.bind(&head);
                let c = self.operand(cond, CpType::Bool)?;
                self.emit(self.jz_instr(c, &end));
                self.loops.push(LoopCtx {
                    continue_label: head.clone(),
                    break_label: end.clone(),
                });
                self.lower_block(body)?;
                self.loops.pop();
                self.emit(self.jmp_instr(&head));
                self.bind(&end);
            }
            StmtKind::Repeat { body, until } => {
                let head = self.new_label();
                let cont = self.new_label();
                let end = self.new_label();
                self.bind(&head);
                self.loops.push(LoopCtx {
                    continue_label: cont.clone(),
                    break_label: end.clone(),
                });
                self.lower_block(body)?;
                self.loops.pop();
                self.bind(&cont);
                let c = self.operand(until, CpType::Bool)?;
                self.emit(self.jz_instr(c, &head)); // loop while UNTIL is false
                self.bind(&end);
            }
            StmtKind::For {
                var,
                from,
                to,
                by,
                body,
            } => self.lower_for(var, from, to, by.as_ref(), body)?,
            StmtKind::Case {
                selector,
                branches,
                else_body,
            } => self.lower_case(selector, branches, else_body)?,
            StmtKind::Return => self.emit(self.ret_instr()),
            StmtKind::Exit => {
                let target = self
                    .loops
                    .last()
                    .ok_or("EXIT outside a loop")?
                    .break_label
                    .clone();
                self.emit(self.jmp_instr(&target));
            }
            StmtKind::Continue => {
                let target = self
                    .loops
                    .last()
                    .ok_or("CONTINUE outside a loop")?
                    .continue_label
                    .clone();
                self.emit(self.jmp_instr(&target));
            }
            StmtKind::FbCall { instance, args } => self.lower_fb_call(instance, args)?,
        }
        Ok(())
    }

    fn lower_for(
        &mut self,
        var: &str,
        from: &Expr,
        to: &Expr,
        by: Option<&Expr>,
        body: &[Stmt],
    ) -> Result<(), String> {
        let (vaddr, vty) = self.lookup(var)?;
        self.eval_into(vaddr, vty, from)?;
        let head = self.new_label();
        let cont = self.new_label();
        let end = self.new_label();
        self.bind(&head);
        // Ascending loop guard: var <= to. (Descending BY is not handled yet.)
        let cond = self.layout.alloc_anon(CpType::Bool.size())?;
        let to_addr = self.operand(to, vty)?;
        let le = self.typed_vmcode("LE", vty)?;
        self.emit(bin3(le, cond, vaddr, to_addr));
        self.emit(self.jz_instr(cond, &end));
        self.loops.push(LoopCtx {
            continue_label: cont.clone(),
            break_label: end.clone(),
        });
        self.lower_block(body)?;
        self.loops.pop();
        self.bind(&cont);
        let step = match by {
            Some(expr) => self.operand(expr, vty)?,
            None => self.intern_const(int_le_bytes(1, vty.size()))?,
        };
        let add = self.typed_vmcode("ADD", vty)?;
        self.emit(bin3(add, vaddr, vaddr, step));
        self.emit(self.jmp_instr(&head));
        self.bind(&end);
        Ok(())
    }

    fn lower_case(
        &mut self,
        selector: &Expr,
        branches: &[(Vec<CaseLabel>, Vec<Stmt>)],
        else_body: &[Stmt],
    ) -> Result<(), String> {
        let selty = self.value_type(selector)?;
        let sel = self.operand(selector, selty)?;
        let end = self.new_label();
        for (labels, body) in branches {
            let body_lbl = self.new_label();
            let next = self.new_label();
            for label in labels {
                let matched = self.layout.alloc_anon(CpType::Bool.size())?;
                match label {
                    CaseLabel::Single(value) => {
                        let cst = self.intern_const(int_le_bytes(*value, selty.size()))?;
                        let eq = self.typed_vmcode("EQ", selty)?;
                        self.emit(bin3(eq, matched, sel, cst));
                    }
                    CaseLabel::Range(lo, hi) => {
                        let lo_c = self.intern_const(int_le_bytes(*lo, selty.size()))?;
                        let hi_c = self.intern_const(int_le_bytes(*hi, selty.size()))?;
                        let ge_t = self.layout.alloc_anon(CpType::Bool.size())?;
                        let le_t = self.layout.alloc_anon(CpType::Bool.size())?;
                        let ge = self.typed_vmcode("GE", selty)?;
                        let le = self.typed_vmcode("LE", selty)?;
                        let and = self.typed_vmcode("AND", CpType::Bool)?;
                        self.emit(bin3(ge, ge_t, sel, lo_c));
                        self.emit(bin3(le, le_t, sel, hi_c));
                        self.emit(bin3(and, matched, ge_t, le_t));
                    }
                }
                self.emit(self.jnz_instr(matched, &body_lbl));
            }
            self.emit(self.jmp_instr(&next));
            self.bind(&body_lbl);
            self.lower_block(body)?;
            self.emit(self.jmp_instr(&end));
            self.bind(&next);
        }
        self.lower_block(else_body)?;
        self.bind(&end);
        Ok(())
    }

    /// Lower `inst(IN := x, ...)`: copy inputs into the instance frame, then
    /// `CALB #base, FB_<type>_CODE`.
    fn lower_fb_call(&mut self, instance: &str, args: &[CallArg]) -> Result<(), String> {
        let (base, fb_index) = {
            let inst = self
                .instances
                .get(&instance.to_ascii_lowercase())
                .ok_or_else(|| {
                    format!("call to undeclared function-block instance `{instance}`")
                })?;
            (inst.base, inst.fb_index)
        };
        let inputs = self.fb_types[fb_index].inputs.clone();
        let members = self.fb_types[fb_index].members.clone();

        for (i, arg) in args.iter().enumerate() {
            let (in_name, in_ty) = match &arg.name {
                Some(name) => {
                    let lname = name.to_ascii_lowercase();
                    let ty = inputs
                        .iter()
                        .find(|(n, _)| *n == lname)
                        .map(|(_, t)| *t)
                        .ok_or_else(|| format!("`{instance}` has no input `{name}`"))?;
                    (lname, ty)
                }
                None => inputs
                    .get(i)
                    .cloned()
                    .ok_or_else(|| format!("`{instance}` called with too many arguments"))?,
            };
            let (off, _) = members
                .get(&in_name)
                .copied()
                .ok_or_else(|| format!("`{instance}` input `{in_name}` has no slot"))?;
            self.eval_into(base + off, in_ty, &arg.value)?;
        }

        let label = format!("FB_{}_CODE", self.fb_types[fb_index].name);
        self.emit(self.calb_instr(base, &label));
        Ok(())
    }

    // -- expressions -------------------------------------------------------

    /// Compute `expr` (of result type `ty`) and store it at `dst`.
    fn eval_into(&mut self, dst: u16, ty: CpType, expr: &Expr) -> Result<(), String> {
        match expr {
            Expr::Lit(value) => {
                let bytes = encode_value(value, ty)?;
                self.emit(self.mcd_instr(dst, &bytes));
            }
            Expr::Var(_) | Expr::Member(_, _) => {
                let (src, src_ty) = self.rvalue_addr(expr)?;
                if src != dst {
                    // STRING copy goes through STRASGN (it respects the
                    // destination's capacity); fixed-width types use MEMCP.
                    if matches!(ty, CpType::Str(_)) {
                        self.emit(self.strasgn_instr(dst, src));
                    } else {
                        self.emit(self.memcp_instr(dst, src, src_ty.size()));
                    }
                }
            }
            Expr::Unary(UnOp::Not, inner) => {
                let a = self.operand(inner, ty)?;
                let vmcode = self.typed_vmcode("NOT", ty)?;
                self.emit(bin2(vmcode, dst, a));
            }
            Expr::Unary(UnOp::Neg, inner) => {
                let a = self.operand(inner, ty)?;
                let vmcode = self.typed_vmcode("NEG", ty)?;
                self.emit(bin2(vmcode, dst, a));
            }
            Expr::Binary(op, lhs, rhs) if is_comparison(*op) => {
                let ot = self.operand_type(lhs, rhs)?;
                let a = self.operand(lhs, ot)?;
                let b = self.operand(rhs, ot)?;
                let vmcode = self.typed_vmcode(binop_name(*op), ot)?;
                self.emit(bin3(vmcode, dst, a, b));
            }
            Expr::Binary(op, lhs, rhs) => {
                if matches!(ty, CpType::Str(_)) {
                    return Err("string operators are not supported yet".to_owned());
                }
                let a = self.operand(lhs, ty)?;
                let b = self.operand(rhs, ty)?;
                let vmcode = self.typed_vmcode(binop_name(*op), ty)?;
                self.emit(bin3(vmcode, dst, a, b));
            }
            Expr::Call(name, args) => self.eval_call_into(dst, name, args)?,
        }
        Ok(())
    }

    /// Evaluate `expr` to an address holding its value (no copy for vars/literals).
    fn operand(&mut self, expr: &Expr, ty: CpType) -> Result<u16, String> {
        match expr {
            Expr::Var(_) | Expr::Member(_, _) => Ok(self.rvalue_addr(expr)?.0),
            Expr::Lit(value) => {
                let bytes = encode_value(value, ty)?;
                self.intern_const(bytes)
            }
            _ => {
                let temp = self.layout.alloc_anon(ty.size())?;
                // A STRING result temp needs its header (chars_size) seeded before
                // the producing op reads/writes it; fixed-width temps don't.
                if let CpType::Str(cap) = ty {
                    let image = string_image("", cap);
                    self.emit(self.mcd_instr(temp, &image));
                }
                self.eval_into(temp, ty, expr)?;
                Ok(temp)
            }
        }
    }

    /// Evaluate a STRING-typed expression to an address (var/member slot, an
    /// interned string constant, or a seeded temp for a nested string call).
    fn string_operand(&mut self, expr: &Expr) -> Result<u16, String> {
        match expr {
            Expr::Var(_) | Expr::Member(_, _) => Ok(self.rvalue_addr(expr)?.0),
            Expr::Lit(Value::Str(s)) => {
                let cap = s.len().min(251) as u16;
                self.intern_const(string_image(s, cap))
            }
            _ => self.operand(expr, CpType::Str(crate::types::DEFAULT_STR_CAP)),
        }
    }

    fn eval_call_into(&mut self, dst: u16, name: &str, args: &[Expr]) -> Result<(), String> {
        match (name.to_ascii_uppercase().as_str(), args) {
            ("CUR_TIME", []) => {
                self.emit(
                    Instr::new(self.cur_time, vec![Operand::Addr(dst)]).with_mnemonic("CUR_TIME"),
                );
                Ok(())
            }
            // LEN(s) -> INT : [dst:INT][src:STRING]
            ("LEN", [s]) => {
                let src = self.string_operand(s)?;
                let vmcode = self.string_fn_vmcode("LEN")?;
                self.emit(bin2(vmcode, dst, src).with_mnemonic("LEN"));
                Ok(())
            }
            // LEFT/RIGHT(s, n) -> STRING : [dst:STRING][src:STRING][len:INT]
            ("LEFT" | "RIGHT", [s, n]) => {
                let src = self.string_operand(s)?;
                let len = self.operand(n, CpType::Int)?;
                let vmcode = self.string_fn_vmcode(&name.to_ascii_uppercase())?;
                self.emit(bin3(vmcode, dst, src, len).with_mnemonic("LEFT/RIGHT"));
                Ok(())
            }
            // MID(s, len, pos) -> STRING : [dst][src][len:INT][from:INT]
            ("MID", [s, len_e, from_e]) => {
                let src = self.string_operand(s)?;
                let len = self.operand(len_e, CpType::Int)?;
                let from = self.operand(from_e, CpType::Int)?;
                let vmcode = self.string_fn_vmcode("MID")?;
                self.emit(
                    Instr::new(
                        vmcode,
                        vec![
                            Operand::Addr(dst),
                            Operand::Addr(src),
                            Operand::Addr(len),
                            Operand::Addr(from),
                        ],
                    )
                    .with_mnemonic("MID"),
                );
                Ok(())
            }
            // CONCAT(a, b, ...) -> STRING : ADD_STRING variadic [dst][a][b]...
            ("CONCAT", parts) if parts.len() >= 2 => {
                let srcs: Vec<u16> = parts
                    .iter()
                    .map(|p| self.string_operand(p))
                    .collect::<Result<_, _>>()?;
                let vmcode = self
                    .spec
                    .typed("ADD", CpType::Str(0))
                    .map(|v| v.encode(srcs.len() as u8))
                    .ok_or("spec table has no ADD for STRING (CONCAT)")?;
                let mut ops = vec![Operand::Addr(dst)];
                ops.extend(srcs.into_iter().map(Operand::Addr));
                self.emit(Instr::new(vmcode, ops).with_mnemonic("CONCAT"));
                Ok(())
            }
            (other, _) => Err(format!("function `{other}` is not supported yet")),
        }
    }

    /// Resolve a STRING function opcode (capacity-agnostic) from the spec table.
    fn string_fn_vmcode(&self, name: &str) -> Result<u16, String> {
        self.spec
            .typed(name, CpType::Str(0))
            .map(|v| v.encode(0))
            .ok_or_else(|| format!("spec table has no string function `{name}`"))
    }

    // -- name resolution ---------------------------------------------------

    /// Resolve an assignment target (`var` or `inst.member`) to (addr, type).
    fn lvalue(&self, target: &str) -> Result<(u16, CpType), String> {
        // Parser only produces bare-identifier assignment targets today; member
        // targets are dropped upstream. Resolve as a plain variable.
        self.lookup(target)
    }

    /// Resolve an r-value expression that is a name or member to (addr, type).
    /// Member access resolves through any depth of FB nesting
    /// (`outer.inner.field`) via [`resolve_instance`](Self::resolve_instance).
    fn rvalue_addr(&self, expr: &Expr) -> Result<(u16, CpType), String> {
        match expr {
            Expr::Var(name) => self.lookup(name),
            Expr::Member(base, member) => {
                let (abs_base, fb_index) = self.resolve_instance(base)?;
                let (off, ty) = self.fb_types[fb_index]
                    .members
                    .get(&member.to_ascii_lowercase())
                    .copied()
                    .ok_or_else(|| format!("function block has no member `{member}`"))?;
                Ok((abs_base + off, ty))
            }
            _ => Err("expected a variable or member reference".to_owned()),
        }
    }

    /// Resolve an instance reference (`inst` or `outer.inner` ...) to its absolute
    /// data-frame base and FB type index. Each `.hop` descends into a nested
    /// instance, composing frame-relative bases.
    fn resolve_instance(&self, expr: &Expr) -> Result<(u16, usize), String> {
        match expr {
            Expr::Var(name) => {
                let inst = self
                    .instances
                    .get(&name.to_ascii_lowercase())
                    .ok_or_else(|| format!("`{name}` is not a function-block instance"))?;
                Ok((inst.base, inst.fb_index))
            }
            Expr::Member(base, field) => {
                let (base_addr, fb_index) = self.resolve_instance(base)?;
                let (rel, inner) = self.fb_types[fb_index]
                    .nested
                    .get(&field.to_ascii_lowercase())
                    .copied()
                    .ok_or_else(|| format!("`{field}` is not a nested function-block instance"))?;
                Ok((base_addr + rel, inner))
            }
            _ => Err("expected a function-block instance reference".to_owned()),
        }
    }

    fn lookup(&self, name: &str) -> Result<(u16, CpType), String> {
        self.vars
            .get(&name.to_ascii_lowercase())
            .copied()
            .ok_or_else(|| format!("reference to undeclared variable `{name}`"))
    }

    // -- type inference ----------------------------------------------------

    fn value_type(&self, expr: &Expr) -> Result<CpType, String> {
        Ok(match expr {
            Expr::Lit(value) => value_cp(value),
            Expr::Var(_) | Expr::Member(_, _) => self.rvalue_addr(expr)?.1,
            Expr::Unary(UnOp::Not | UnOp::Neg, inner) => self.value_type(inner)?,
            Expr::Binary(op, _, _) if is_comparison(*op) => CpType::Bool,
            Expr::Binary(_, lhs, rhs) => unify(self.value_type(lhs)?, self.value_type(rhs)?),
            Expr::Call(name, _) => match name.to_ascii_uppercase().as_str() {
                "CUR_TIME" => CpType::Time,
                "LEN" => CpType::Int,
                "LEFT" | "RIGHT" | "MID" | "CONCAT" => CpType::Str(crate::types::DEFAULT_STR_CAP),
                other => return Err(format!("cannot infer return type of `{other}`")),
            },
        })
    }

    fn operand_type(&self, lhs: &Expr, rhs: &Expr) -> Result<CpType, String> {
        Ok(unify(self.value_type(lhs)?, self.value_type(rhs)?))
    }

    // -- low-level helpers -------------------------------------------------

    fn intern_const(&mut self, bytes: Vec<u8>) -> Result<u16, String> {
        if let Some(&addr) = self.const_index.get(&bytes) {
            return Ok(addr);
        }
        let addr = self.layout.alloc_anon(bytes.len())?;
        self.const_index.insert(bytes.clone(), addr);
        self.consts.push((addr, bytes));
        Ok(addr)
    }

    fn typed_vmcode(&self, name: &str, ty: CpType) -> Result<u16, String> {
        self.spec
            .typed(name, ty)
            .map(|v| v.encode(2))
            .ok_or_else(|| format!("spec table has no `{name}` for type {ty:?}"))
    }

    fn new_label(&mut self) -> String {
        let label = format!("L{}", self.label_counter);
        self.label_counter += 1;
        label
    }

    fn bind(&mut self, label: &str) {
        self.code.push(Item::Label(label.to_owned()));
    }

    fn emit(&mut self, instr: Instr) {
        self.code.push(Item::Instr(instr));
    }

    fn mcd_instr(&self, addr: u16, bytes: &[u8]) -> Instr {
        Instr::new(
            self.mcd,
            vec![
                Operand::Addr(addr),
                Operand::ImmByte(bytes.len() as u8),
                Operand::ImmBytes(bytes.to_vec()),
            ],
        )
        .with_mnemonic("MCD")
    }

    fn calb_instr(&self, base: u16, target: &str) -> Instr {
        Instr::new(
            self.calb,
            vec![Operand::ImmWord(base), Operand::Code(target.to_owned())],
        )
        .with_mnemonic("CALB")
    }

    fn trml_instr(&self, target: &str) -> Instr {
        Instr::new(self.trml, vec![Operand::Code(target.to_owned())]).with_mnemonic("TRML")
    }

    fn ret_instr(&self) -> Instr {
        Instr::new(self.ret, vec![]).with_mnemonic("RETURN")
    }

    fn memcp_instr(&self, dst: u16, src: u16, size: usize) -> Instr {
        Instr::new(
            self.memcp,
            vec![
                Operand::Addr(dst),
                Operand::Addr(src),
                Operand::ImmWord(size as u16),
            ],
        )
        .with_mnemonic("MEMCP")
    }

    fn jz_instr(&self, cond: u16, target: &str) -> Instr {
        Instr::new(
            self.jz,
            vec![Operand::Addr(cond), Operand::Code(target.to_owned())],
        )
        .with_mnemonic("JZ")
    }

    fn jnz_instr(&self, cond: u16, target: &str) -> Instr {
        Instr::new(
            self.jnz,
            vec![Operand::Addr(cond), Operand::Code(target.to_owned())],
        )
        .with_mnemonic("JNZ")
    }

    fn jmp_instr(&self, target: &str) -> Instr {
        Instr::new(self.jmp, vec![Operand::Code(target.to_owned())]).with_mnemonic("JMP")
    }

    fn strasgn_instr(&self, dst: u16, src: u16) -> Instr {
        Instr::new(self.strasgn, vec![Operand::Addr(dst), Operand::Addr(src)])
            .with_mnemonic("STRASGN")
    }
}

/// A two-address instruction `OP dst, a` (unary ops).
fn bin2(vmcode: u16, dst: u16, a: u16) -> Instr {
    Instr::new(vmcode, vec![Operand::Addr(dst), Operand::Addr(a)])
}

/// A three-address instruction `OP dst, a, b`.
fn bin3(vmcode: u16, dst: u16, a: u16, b: u16) -> Instr {
    Instr::new(
        vmcode,
        vec![Operand::Addr(dst), Operand::Addr(a), Operand::Addr(b)],
    )
}
