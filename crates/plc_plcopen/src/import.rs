//! PLCopen XML → LD model. Pull-parses the LD subset, reconstructing rungs
//! by connectivity: contacts reference either the rung's left power rail
//! (starting a branch) or a preceding contact (extending one); outputs
//! reference branch tails or the rail. Unknown elements are skipped with a
//! fidelity note; positions are ignored (the `localId` carries our element
//! id; our layout is computed, not stored).

use std::collections::HashMap;

use quick_xml::{Reader, events::Event};

use plc_ld::{
    BlockArg, CURRENT_SCHEMA_VERSION, CoilVariant, ContactElement, LdProgram, OutputElement, Rung,
    SeriesBranch,
};

use crate::PlcopenError;

/// Parse PLCopen XML into an [`LdProgram`] (fidelity notes discarded).
pub fn from_plcopen(xml: &str) -> Result<LdProgram, PlcopenError> {
    let (program, _notes) = from_plcopen_with_notes(xml)?;
    Ok(program)
}

#[derive(Default, Debug)]
struct RawContact {
    id: String,
    negated: bool,
    variable: String,
    upstream: Vec<String>,
}

#[derive(Default, Debug)]
struct RawOutput {
    id: String,
    storage: String,
    variable: String,
    type_name: String,
    instance: String,
    args: Vec<(bool, BlockArg)>, // (is_input, arg)
    upstream: Vec<String>,
}

/// Parse with the fidelity notes (unknown-element skips) exposed.
pub fn from_plcopen_with_notes(xml: &str) -> Result<(LdProgram, Vec<String>), PlcopenError> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(false);

    let mut notes: Vec<String> = Vec::new();
    let mut program_name = "Imported".to_owned();
    let mut contacts: Vec<RawContact> = Vec::new();
    let mut outputs: Vec<RawOutput> = Vec::new();
    let mut comments: Vec<(String, String)> = Vec::new(); // (rung_id, text)
    let mut rails: Vec<String> = Vec::new();

    let mut current_tag: String = String::new();
    let mut current_contact: Option<RawContact> = None;
    let mut current_output: Option<RawOutput> = None;
    let mut current_comment: Option<(String, String)> = None;

    loop {
        match reader.read_event() {
            Ok(Event::Start(start)) => {
                let name = String::from_utf8_lossy(start.name().as_ref()).to_string();
                let attr = |key: &str| {
                    start.attributes().with_checks(false).find_map(|a| {
                        a.ok().and_then(|a| {
                            (a.key.as_ref() == key.as_bytes()).then(|| {
                                a.unescape_value()
                                    .map(|v| v.to_string())
                                    .unwrap_or_else(|_| {
                                        String::from_utf8_lossy(&a.value).to_string()
                                    })
                            })
                        })
                    })
                };
                match name.as_str() {
                    "project" | "types" | "pous" | "body" | "LD" | "variable" | "position"
                    | "relPosition" | "expression" => {}
                    "pou" => program_name = attr("name").unwrap_or(program_name),
                    "leftPowerRail" => {
                        if let Some(id) = attr("localId") {
                            rails.push(id);
                        }
                    }
                    "contact" => {
                        current_contact = Some(RawContact {
                            id: attr("localId").unwrap_or_default(),
                            negated: attr("negated").as_deref() == Some("true"),
                            variable: String::new(),
                            upstream: Vec::new(),
                        });
                    }
                    "coil" => {
                        current_output = Some(RawOutput {
                            id: attr("localId").unwrap_or_default(),
                            storage: attr("storage").unwrap_or_else(|| "none".to_owned()),
                            ..RawOutput::default()
                        });
                    }
                    "block" => {
                        current_output = Some(RawOutput {
                            id: attr("localId").unwrap_or_default(),
                            type_name: attr("typeName").unwrap_or_default(),
                            instance: attr("instanceName").unwrap_or_default(),
                            ..RawOutput::default()
                        });
                    }
                    "comment" => {
                        current_comment =
                            Some((attr("localId").unwrap_or_default(), String::new()));
                    }
                    "content" if current_comment.is_some() => {
                        // TC6 nests comment text in <content>; treat it as
                        // comment body.
                        current_tag = "comment".to_owned();
                    }
                    "inVariable" | "outVariable" => {
                        if let Some(formal) = attr("formalParameter") {
                            if let Some(output) = current_output.as_mut() {
                                output.args.push((
                                    name == "inVariable",
                                    BlockArg {
                                        name: formal,
                                        value: String::new(),
                                    },
                                ));
                            } else {
                                // Loose FBD variable outside a block: skipped.
                                notes.push(format!("skipped unsupported element <{name}>"));
                            }
                        } else {
                            notes.push(format!("skipped unsupported element <{name}>"));
                        }
                    }
                    "connection" => {
                        if let Some(ref_id) = attr("refLocalId") {
                            let sink = if let Some(contact) = current_contact.as_mut() {
                                Some(&mut contact.upstream)
                            } else {
                                current_output.as_mut().map(|output| &mut output.upstream)
                            };
                            if let Some(upstream) = sink {
                                upstream.push(ref_id);
                            }
                        }
                    }
                    _ => {
                        // Unknown structural element (opening tag only —
                        // the matching End stays quiet).
                        let unsupported = matches!(
                            name.as_str(),
                            "rightPowerRail"
                                | "continuation"
                                | "connector"
                                | "actionBlock"
                                | "step"
                                | "transition"
                        ) || name.ends_with("Variable");
                        if unsupported {
                            notes.push(format!("skipped unsupported element <{name}>"));
                        } else if !name.is_empty() {
                            notes.push(format!("skipped unknown element <{name}>"));
                        }
                    }
                }
                if name != "content" {
                    current_tag = name;
                }
            }
            Ok(Event::Text(text)) => {
                // Unescape entities (&gt; etc.) — the event carries raw bytes.
                let value = quick_xml::escape::unescape(&String::from_utf8_lossy(&text))
                    .map_err(|error| PlcopenError(format!("text: {error}")))?
                    .to_string();
                if value.trim().is_empty() {
                    continue;
                }
                match current_tag.as_str() {
                    "variable" => {
                        if let Some(contact) = current_contact.as_mut() {
                            contact.variable.push_str(value.trim());
                        } else if let Some(output) = current_output.as_mut() {
                            output.variable.push_str(value.trim());
                        }
                    }
                    "comment" => {
                        if let Some(comment) = current_comment.as_mut() {
                            comment.1.push_str(&value);
                        }
                    }
                    "inVariable" | "outVariable" => {
                        if let Some(output) = current_output.as_mut()
                            && let Some(last) = output.args.last_mut()
                        {
                            last.1.value.push_str(value.trim());
                        }
                    }
                    _ => {}
                }
            }
            Ok(Event::End(end)) => {
                let name = String::from_utf8_lossy(end.name().as_ref()).to_string();
                match name.as_str() {
                    "contact" => {
                        if let Some(contact) = current_contact.take() {
                            contacts.push(contact);
                        }
                    }
                    "coil" | "block" => {
                        if let Some(output) = current_output.take() {
                            outputs.push(output);
                        }
                    }
                    "comment" => {
                        if let Some(comment) = current_comment.take() {
                            comments.push(comment);
                        }
                    }
                    "inVariable" | "outVariable" | "variable" | "position" | "relPosition"
                    | "content" => {
                        current_tag = String::new();
                    }
                    "LD" | "body" | "pou" | "project" | "types" | "pous" => {}
                    _ => {}
                }
            }
            Ok(Event::GeneralRef(reference)) => {
                // quick-xml splits text at entity boundaries; the resolved
                // entity arrives here and must join the current text.
                // Numeric char refs decode directly; the five predefined
                // XML named entities map by hand (that is the complete set
                // our writer emits, and all conforming XML defines).
                let raw = String::from_utf8_lossy(reference.as_ref()).to_string();
                let value = if reference.is_char_ref() {
                    reference
                        .decode()
                        .map_err(|error| PlcopenError(format!("entity: {error}")))?
                        .to_string()
                } else {
                    match raw.as_str() {
                        "gt" => ">".to_owned(),
                        "lt" => "<".to_owned(),
                        "amp" => "&".to_owned(),
                        "apos" => "'".to_owned(),
                        "quot" => "\"".to_owned(),
                        other => {
                            return Err(PlcopenError(format!("unknown entity &{other};")));
                        }
                    }
                };
                match current_tag.as_str() {
                    "variable" => {
                        if let Some(contact) = current_contact.as_mut() {
                            contact.variable.push_str(&value);
                        } else if let Some(output) = current_output.as_mut() {
                            output.variable.push_str(&value);
                        }
                    }
                    "comment" => {
                        if let Some(comment) = current_comment.as_mut() {
                            comment.1.push_str(&value);
                        }
                    }
                    "inVariable" | "outVariable" => {
                        if let Some(output) = current_output.as_mut()
                            && let Some(last) = output.args.last_mut()
                        {
                            last.1.value.push_str(&value);
                        }
                    }
                    _ => {}
                }
            }
            Ok(Event::Empty(empty)) => {
                // Self-closing elements carry the same attributes.
                let name = String::from_utf8_lossy(empty.name().as_ref()).to_string();
                let attr = |key: &str| {
                    empty.attributes().with_checks(false).find_map(|a| {
                        a.ok().and_then(|a| {
                            (a.key.as_ref() == key.as_bytes()).then(|| {
                                a.unescape_value()
                                    .map(|v| v.to_string())
                                    .unwrap_or_else(|_| {
                                        String::from_utf8_lossy(&a.value).to_string()
                                    })
                            })
                        })
                    })
                };
                match name.as_str() {
                    "leftPowerRail" => {
                        if let Some(id) = attr("localId") {
                            rails.push(id);
                        }
                    }
                    "connection" => {
                        if let Some(ref_id) = attr("refLocalId") {
                            let sink = if let Some(contact) = current_contact.as_mut() {
                                Some(&mut contact.upstream)
                            } else {
                                current_output.as_mut().map(|output| &mut output.upstream)
                            };
                            if let Some(upstream) = sink {
                                upstream.push(ref_id);
                            }
                        }
                    }
                    _ => {}
                }
            }
            Ok(Event::Eof) => break,
            Ok(_) => {}
            Err(error) => {
                return Err(PlcopenError(format!("XML parse: {error}")));
            }
        }
    }

    // --- reconstruct rungs ----------------------------------------------
    // Rails define the rung order (document order), so empty and
    // comment-only rungs survive and nothing is reordered.
    let mut rungs: Vec<RungBuilder> = rails
        .iter()
        .map(|rail| RungBuilder {
            id: rail.strip_suffix("-rail").map(str::to_owned),
            ..RungBuilder::default()
        })
        .collect();
    let rail_position: HashMap<&String, usize> = rails
        .iter()
        .enumerate()
        .map(|(index, rail)| (rail, index))
        .collect();

    // Resolve each contact's rung by walking upstream to a rail.
    let mut contact_rung: HashMap<String, usize> = HashMap::new();
    for contact in &contacts {
        let mut seen = std::collections::HashSet::new();
        let mut current = contact.id.clone();
        let rail = loop {
            if let Some(position) = rail_position.get(&current) {
                break *position;
            }
            if !seen.insert(current.clone()) {
                return Err(PlcopenError(format!(
                    "contact {} not connected to a rail (cycle)",
                    contact.id
                )));
            }
            let next = contacts
                .iter()
                .find(|candidate| candidate.id == current)
                .and_then(|candidate| candidate.upstream.first().cloned());
            match next {
                Some(next) => current = next,
                None => {
                    return Err(PlcopenError(format!(
                        "contact {} not connected to a rail",
                        contact.id
                    )));
                }
            }
        };
        contact_rung.insert(contact.id.clone(), rail);
    }

    // Assign contacts to branches. Forward references are resolved by
    // iterating to a fixpoint: each pass places contacts whose upstream
    // is a rail (branch start) or an already-placed contact.
    let mut contact_branch: HashMap<String, usize> = HashMap::new();
    let mut placed: Vec<bool> = vec![false; contacts.len()];
    for _ in 0..=contacts.len() {
        let mut progress = false;
        for (index, contact) in contacts.iter().enumerate() {
            if placed[index] {
                continue;
            }
            let builder = &mut rungs[contact_rung[&contact.id]];
            let rail = &rails[contact_rung[&contact.id]];
            let starts_branch = contact.upstream.iter().any(|up| up == rail);
            let extends_branch = contact
                .upstream
                .iter()
                .find_map(|up| contact_branch.get(up).copied());
            if starts_branch {
                builder.branches.push(SeriesBranch {
                    elements: vec![contact_element(contact)],
                });
                contact_branch.insert(contact.id.clone(), builder.branches.len() - 1);
                placed[index] = true;
                progress = true;
            } else if let Some(branch_index) = extends_branch {
                builder.branches[branch_index]
                    .elements
                    .push(contact_element(contact));
                contact_branch.insert(contact.id.clone(), branch_index);
                placed[index] = true;
                progress = true;
            }
        }
        if placed.iter().all(|done| *done) || !progress {
            break;
        }
    }
    if let Some(unplaced) = placed.iter().position(|done| !*done) {
        return Err(PlcopenError(format!(
            "contact {} references an unplaced or foreign upstream",
            contacts[unplaced].id
        )));
    }

    for output in &outputs {
        let position = output
            .upstream
            .iter()
            .find_map(|up| rail_position.get(up).copied())
            .or_else(|| {
                output
                    .upstream
                    .iter()
                    .find_map(|up| contact_rung.get(up).copied())
            })
            .ok_or_else(|| PlcopenError(format!("output {} not connected to a rung", output.id)))?;
        rungs[position].outputs.push(output_element(output));
    }

    // Comments attach to the rung exporting the matching {id}-rail; a
    // foreign comment whose id matches no rail becomes a fidelity note
    // instead of a phantom rung.
    for (rung_id, text) in &comments {
        let rail = format!("{rung_id}-rail");
        match rail_position.get(&rail) {
            Some(position) => {
                rungs[*position].comment = Some(text.clone());
                rungs[*position].id = Some(rung_id.clone());
            }
            None => {
                if !text.is_empty() {
                    notes.push(format!("skipped foreign comment '{text}'"));
                } else {
                    notes.push("skipped foreign comment".to_owned());
                }
            }
        }
    }

    let mut program = LdProgram::new(program_name);
    program.schema_version = CURRENT_SCHEMA_VERSION;
    for builder in rungs {
        program.rungs.push(Rung {
            id: builder.id,
            comment: builder.comment,
            branches: builder.branches,
            outputs: builder.outputs,
        });
    }

    Ok((program, notes))
}

#[derive(Default)]
struct RungBuilder {
    id: Option<String>,
    comment: Option<String>,
    branches: Vec<SeriesBranch>,
    outputs: Vec<OutputElement>,
}

fn contact_element(raw: &RawContact) -> ContactElement {
    ContactElement {
        id: Some(raw.id.clone()),
        name: raw.variable.clone(),
        negated: raw.negated,
    }
}

fn output_element(raw: &RawOutput) -> OutputElement {
    if raw.type_name.is_empty() {
        OutputElement::Coil {
            id: Some(raw.id.clone()),
            name: raw.variable.clone(),
            variant: match raw.storage.as_str() {
                "set" => CoilVariant::Set,
                "reset" => CoilVariant::Reset,
                _ => CoilVariant::Normal,
            },
        }
    } else {
        OutputElement::Block {
            id: Some(raw.id.clone()),
            fb_type: raw.type_name.clone(),
            instance: raw.instance.clone(),
            inputs: raw
                .args
                .iter()
                .filter(|(is_input, _)| *is_input)
                .map(|(_, arg)| arg.clone())
                .collect(),
            outputs: raw
                .args
                .iter()
                .filter(|(is_input, _)| !*is_input)
                .map(|(_, arg)| arg.clone())
                .collect(),
        }
    }
}
