//! `plc ld --serve`: a synchronous simulation server speaking line-delimited
//! JSON over stdio, wrapping the deterministic runtime (PLC-113).
//!
//! Design: the CLIENT drives pacing — `{"op":"tick"}` runs exactly one scan
//! (input phase → logic → forces → snapshot), so the server has no threads,
//! no timers, and is fully deterministic. The VS Code editor paces ticks
//! with a host-side `setInterval` while running.
//!
//! Protocol version 1. Events: `ready`, `loaded`, `error`, `state`,
//! `powerFlow`, `diagnostics`.

use std::io::{BufRead, Write};

use serde::Deserialize;
use serde_json::{Value, json};

use plc_lang::LanguageRegistry;
use plc_ld::{LdProgram, evaluate_power_flow, normalize_ids, validate, var_state_from_watch};
use plc_runtime::Runtime;

use crate::SCAN_INTERVAL_MS;

/// One client→server operation.
#[derive(Debug, Deserialize)]
#[serde(tag = "op", rename_all = "camelCase")]
enum Op {
    Hello,
    Load { json: String },
    Reload { json: String },
    SetInterval { ms: i64 },
    SetInput { name: String, value: Value },
    Force { name: String, value: Value },
    Unforce { name: String },
    Tick,
}

/// Run the serve loop over any reader/writer pair (testable via duplex).
pub fn run_ld_serve<R: BufRead, W: Write>(input: R, mut output: W) -> std::io::Result<()> {
    let mut session = Session::default();
    for line in input.lines() {
        let line = line?;
        let trimmed = line.trim_end_matches(['\r', '\n']);
        if trimmed.trim().is_empty() {
            continue;
        }
        let op: Op = match serde_json::from_str(trimmed) {
            Ok(op) => op,
            Err(error) => {
                emit(
                    &mut output,
                    json!({"event": "error", "message": format!("bad op: {error}")}),
                )?;
                continue;
            }
        };
        match op {
            Op::Hello => emit(
                &mut output,
                json!({
                    "event": "ready",
                    "protocolVersion": 1,
                    "fbCatalog": fb_catalog(),
                }),
            )?,
            Op::Load { json } | Op::Reload { json } => session.load(json, &mut output)?,
            Op::SetInterval { ms } => {
                if ms < 0 {
                    emit(
                        &mut output,
                        json!({"event": "error", "message": "setInterval: ms must be >= 0"}),
                    )?;
                    continue;
                }
                session.interval_ms = Some(ms);
                if let Some(runtime) = session.runtime_mut() {
                    runtime.set_scan_interval_ms(ms);
                }
            }
            Op::SetInput { name, value } => {
                if let Some(runtime) = session.runtime_mut() {
                    runtime.set_input(&name, json_to_value(&value));
                } else {
                    emit(
                        &mut output,
                        json!({"event": "error", "message": "setInput: no program loaded"}),
                    )?;
                    continue;
                }
            }
            Op::Force { name, value } => {
                if let Some(runtime) = session.runtime_mut() {
                    runtime.force(&name, json_to_value(&value));
                } else {
                    emit(
                        &mut output,
                        json!({"event": "error", "message": "force: no program loaded"}),
                    )?;
                    continue;
                }
            }
            Op::Unforce { name } => {
                if let Some(runtime) = session.runtime_mut() {
                    runtime.unforce(&name);
                } else {
                    emit(
                        &mut output,
                        json!({"event": "error", "message": "unforce: no program loaded"}),
                    )?;
                    continue;
                }
            }
            Op::Tick => session.tick(&mut output)?,
        }
        output.flush()?;
    }
    Ok(())
}

/// Server state: the loaded (model, runtime) pair — set and cleared
/// together — plus the client's chosen scan interval (kept across reloads).
#[derive(Default)]
struct Session {
    loaded: Option<(LdProgram, Runtime)>,
    scan: u64,
    interval_ms: Option<i64>,
}

impl Session {
    fn runtime_mut(&mut self) -> Option<&mut Runtime> {
        self.loaded.as_mut().map(|(_ld, runtime)| runtime)
    }
}

impl Session {
    fn load(&mut self, json: String, output: &mut impl Write) -> std::io::Result<()> {
        let mut program = match plc_ld::parse_ld_json(&json) {
            Ok(program) => program,
            Err(error) => {
                return emit(
                    output,
                    json!({"event": "error", "message": format!("invalid LD JSON: {error}")}),
                );
            }
        };
        normalize_ids(&mut program);
        let diagnostics = validate(&program);

        // Diagnostics first — errors fail the conversion below, and the
        // LD codes must reach the client either way.
        fn emit_diagnostics(
            diagnostics: &[plc_ld::LdDiagnostic],
            output: &mut impl Write,
        ) -> std::io::Result<()> {
            if diagnostics.is_empty() {
                return Ok(());
            }
            let items: Vec<Value> = diagnostics
                .iter()
                .map(|d| {
                    json!({
                        "code": d.code,
                        "severity": match d.severity {
                            plc_ld::LdSeverity::Error => "error",
                            plc_ld::LdSeverity::Warning => "warning",
                        },
                        "elementId": d.element_id,
                        "rung": d.rung,
                        "message": d.message,
                    })
                })
                .collect();
            emit(output, json!({"event": "diagnostics", "items": items}))
        }

        // LD → ST → runtime, exactly like `plc ld`.
        let registry = LanguageRegistry::with_builtins();
        let document = plc_api::SourceDocument::new("file:///served.ld".to_owned(), 0, json);
        let result = registry.convert("ld", "st", &document);
        if result.error.is_some() {
            emit_diagnostics(&diagnostics, output)?;
            return emit(
                output,
                json!({"event": "error", "message": "ld → st conversion failed: see diagnostics"}),
            );
        }
        let mut runtime = Runtime::from_source(&result.text);
        runtime.set_scan_interval_ms(self.interval_ms.unwrap_or(SCAN_INTERVAL_MS));
        self.loaded = Some((program, runtime));
        self.scan = 0;
        emit(output, json!({"event": "loaded", "ok": true}))?;
        emit_diagnostics(&diagnostics, output)?;

        Ok(())
    }

    fn tick(&mut self, output: &mut impl Write) -> std::io::Result<()> {
        let Some((program, runtime)) = self.loaded.as_mut() else {
            return emit(
                output,
                json!({"event": "error", "message": "no program loaded"}),
            );
        };
        runtime.run_scan();
        self.scan += 1;
        let watch = runtime.watch();
        let forced: Vec<String> = runtime
            .inspect()
            .iter()
            .filter(|snapshot| snapshot.forced)
            .map(|snapshot| snapshot.name.clone())
            .collect();

        emit(
            output,
            json!({
                "event": "state",
                "scan": self.scan,
                "timeMs": runtime.clock().now_ms(),
                "watch": watch,
                "forced": forced,
            }),
        )?;

        let flow = evaluate_power_flow(program, &var_state_from_watch(&watch));
        emit(output, json!({"event": "powerFlow", "rungs": flow.rungs}))
    }
}

/// Map JSON scalars onto runtime values (the LD surface is BOOL/INT for now).
fn json_to_value(value: &Value) -> plc_runtime::Value {
    match value {
        Value::Bool(b) => plc_runtime::Value::Bool(*b),
        Value::Number(n) if n.is_i64() => plc_runtime::Value::Int(n.as_i64().unwrap_or(0)),
        Value::Number(n) => plc_runtime::Value::Real(n.as_f64().unwrap_or(0.0)),
        Value::String(s) => plc_runtime::Value::Str(s.clone()),
        _ => plc_runtime::Value::Bool(false),
    }
}

/// The standard FB catalog with pin tables, from plc_ld (authoritative).
fn fb_catalog() -> Vec<Value> {
    plc_ld::STANDARD_FB_TYPES
        .iter()
        .filter_map(|fb_type| {
            plc_ld::fb_pins(fb_type).map(|(inputs, outputs)| {
                json!({
                    "fbType": fb_type,
                    "inputs": inputs,
                    "outputs": outputs,
                })
            })
        })
        .collect()
}

fn emit<W: Write>(output: &mut W, event: Value) -> std::io::Result<()> {
    writeln!(output, "{event}")
}
