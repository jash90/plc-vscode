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
    let mut in_connections = false;

    loop {
        match reader.read_event() {
            Ok(Event::Start(start)) => {
                let name = String::from_utf8_lossy(start.name().as_ref()).to_string();
                let attr = |key: &str| {
                    start.attributes().with_checks(false).find_map(|a| {
                        a.ok().and_then(|a| {
                            (a.key.as_ref() == key.as_bytes())
                                .then(|| String::from_utf8_lossy(&a.value).to_string())
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
                    "connectionPointIn" => in_connections = true,
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
                current_tag = name;
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
                            contact.variable = value.trim().to_owned();
                        } else if let Some(output) = current_output.as_mut() {
                            output.variable = value.trim().to_owned();
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
                            last.1.value = value.trim().to_owned();
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
                    "connectionPointIn" => in_connections = false,
                    "inVariable" | "outVariable" | "variable" => {
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
                    _ => {}
                }
            }
            Ok(Event::Empty(empty)) => {
                // Self-closing elements carry the same attributes.
                let name = String::from_utf8_lossy(empty.name().as_ref()).to_string();
                let attr = |key: &str| {
                    empty.attributes().with_checks(false).find_map(|a| {
                        a.ok().and_then(|a| {
                            (a.key.as_ref() == key.as_bytes())
                                .then(|| String::from_utf8_lossy(&a.value).to_string())
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
    void(in_connections);

    // --- reconstruct rungs by connectivity ---------------------------------
    let rail_ids: std::collections::HashSet<String> = rails.iter().cloned().collect();

    // contact id → rung rail id (walk upstream until a rail is found).
    let mut contact_rung: HashMap<String, String> = HashMap::new();
    let mut contact_branch_index: HashMap<String, usize> = HashMap::new();

    // rung rail id → ordered structure
    let mut rung_order: Vec<String> = Vec::new();
    let mut rungs: HashMap<String, RungBuilder> = HashMap::new();

    let rung_of_contact = |contact: &RawContact,
                           contacts: &[RawContact],
                           rail_ids: &std::collections::HashSet<String>|
     -> Option<String> {
        let mut seen = std::collections::HashSet::new();
        let mut current = contact.id.clone();
        loop {
            if rail_ids.contains(&current) {
                return Some(current);
            }
            if !seen.insert(current.clone()) {
                return None; // cycle guard
            }
            let found = contacts
                .iter()
                .find(|candidate| candidate.id == current)?
                .upstream
                .first()
                .cloned();
            match found {
                Some(next) => current = next,
                None => return None,
            }
        }
    };

    for contact in &contacts {
        let rail = rung_of_contact(contact, &contacts, &rail_ids).ok_or_else(|| {
            PlcopenError(format!("contact {} not connected to a rail", contact.id))
        })?;
        contact_rung.insert(contact.id.clone(), rail.clone());
        let builder = rungs.entry(rail.clone()).or_insert_with(|| {
            rung_order.push(rail.clone());
            RungBuilder::default()
        });

        let starts_branch = contact.upstream.iter().any(|up| {
            rail_ids.contains(up) || !contact_rung.contains_key(up) && rail_ids.contains(up)
        }) || contact.upstream.iter().any(|up| rail_ids.contains(up));
        if starts_branch {
            builder.branches.push(SeriesBranch {
                elements: vec![contact_element(contact)],
            });
            contact_branch_index.insert(contact.id.clone(), builder.branches.len() - 1);
        } else {
            let parent = contact
                .upstream
                .iter()
                .find_map(|up| contact_branch_index.get(up).copied());
            match parent {
                Some(branch_index) => {
                    builder.branches[branch_index]
                        .elements
                        .push(contact_element(contact));
                    contact_branch_index.insert(contact.id.clone(), branch_index);
                }
                None => {
                    builder.branches.push(SeriesBranch {
                        elements: vec![contact_element(contact)],
                    });
                    contact_branch_index.insert(contact.id.clone(), builder.branches.len() - 1);
                }
            }
        }
    }

    for output in &outputs {
        let rail = output
            .upstream
            .iter()
            .find(|up| rail_ids.contains(up.as_str()))
            .cloned()
            .or_else(|| {
                output
                    .upstream
                    .iter()
                    .find_map(|up| contact_rung.get(up).cloned())
            })
            .ok_or_else(|| PlcopenError(format!("output {} not connected to a rung", output.id)))?;
        let builder = rungs.entry(rail.clone()).or_insert_with(|| {
            rung_order.push(rail.clone());
            RungBuilder::default()
        });
        builder.outputs.push(output_element(output));
    }

    for (rung_id, text) in &comments {
        let rail = format!("{rung_id}-rail");
        let builder = rungs.entry(rail).or_insert_with(|| {
            rung_order.push(format!("{rung_id}-rail"));
            RungBuilder::default()
        });
        builder.comment = Some(text.clone());
        builder.id = Some(rung_id.clone());
    }

    // Rung ids: prefer the exported scheme (strip "-rail").
    let mut program = LdProgram::new(program_name);
    program.schema_version = CURRENT_SCHEMA_VERSION;
    for rail in &rung_order {
        let builder = &rungs[rail];
        let rung_id = builder
            .id
            .clone()
            .or_else(|| rail.strip_suffix("-rail").map(str::to_owned));
        program.rungs.push(Rung {
            id: rung_id,
            comment: builder.comment.clone(),
            branches: builder.branches.clone(),
            outputs: builder.outputs.clone(),
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

fn void(_: bool) {}
