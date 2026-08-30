//! LD model → PLCopen XML. Hand-written writer for a stable, diff-friendly
//! output: fixed attribute order, no timestamps, positions derived from the
//! webview grid constants.

use quick_xml::{Writer, events::*};

use plc_ld::{BlockArg, CoilVariant, ContactElement, LdProgram, OutputElement, Rung, SeriesBranch};

use crate::PlcopenError;

/// Grid metrics mirrored from the webview layout (cosmetic positions only).
const CELL_W: i32 = 88;
const CELL_H: i32 = 38;
const RAIL_GAP: i32 = 26;
const RUNG_GAP: i32 = 24;

/// Serialize an [`LdProgram`] to PLCopen XML (LD subset).
pub fn to_plcopen(program: &LdProgram) -> Result<String, PlcopenError> {
    let mut buffer = Vec::new();
    let mut writer = Writer::new_with_indent(&mut buffer, b' ', 2);

    write_event(
        &mut writer,
        Event::Decl(BytesDecl::new("1.0", Some("UTF-8"), None)),
    )?;
    write_empty_tag_with(&mut writer, b"project", |attrs| {
        attrs.push_attribute(("xmlns", "http://www.plcopen.org/xml/tc6_0201"));
    })?;
    write_empty_tag(&mut writer, b"types")?;
    write_self_closing(&mut writer, b"dataTypes", |_| {})?;
    write_empty_tag(&mut writer, b"pous")?;

    write_empty_tag_with(&mut writer, b"pou", |attrs| {
        attrs.push_attribute(("name", program.name.as_str()));
        attrs.push_attribute(("pouType", "program"));
    })?;
    write_empty_tag(&mut writer, b"body")?;
    write_empty_tag(&mut writer, b"LD")?;

    for (rung_index, rung) in program.rungs.iter().enumerate() {
        write_rung(&mut writer, rung_index, rung)?;
    }

    write_close_tag(&mut writer, b"LD")?;
    write_close_tag(&mut writer, b"body")?;
    write_close_tag(&mut writer, b"pou")?;
    write_close_tag(&mut writer, b"pous")?;
    write_close_tag(&mut writer, b"types")?;
    write_close_tag(&mut writer, b"project")?;

    String::from_utf8(buffer).map_err(|error| PlcopenError(format!("utf8: {error}")))
}

fn write_rung(
    writer: &mut Writer<&mut Vec<u8>>,
    rung_index: usize,
    rung: &Rung,
) -> Result<(), PlcopenError> {
    let rung_id = rung.id.clone().unwrap_or_else(|| format!("r{rung_index}"));
    let base_y = (rung_index as i32) * (CELL_H + RUNG_GAP) + RUNG_GAP;

    if let Some(comment) = &rung.comment {
        write_start_tag_with(writer, b"comment", |attrs| {
            attrs.push_attribute(("localId", rung_id.as_str()));
        })?;
        write_text(writer, comment)?;
        write_close_tag(writer, b"comment")?;
    }

    // Left power rail feeds the first branch (self-closing; position attr).
    write_self_closing(writer, b"leftPowerRail", |attrs| {
        attrs.push_attribute(("localId", format!("{rung_id}-rail").as_str()));
        attrs.push_attribute(("x", RAIL_GAP.to_string().as_str()));
        attrs.push_attribute(("y", base_y.to_string().as_str()));
    })?;

    let mut previous_ids: Vec<String> = Vec::new();
    for (branch_index, branch) in rung.branches.iter().enumerate() {
        let branch_y = base_y + (branch_index as i32) * CELL_H;
        previous_ids = write_branch(
            writer,
            &rung_id,
            branch_index,
            branch,
            branch_y,
            &previous_ids,
            &format!("{rung_id}-rail"),
        )?;
    }

    let rung_source_ids: Vec<String> = if previous_ids.is_empty() {
        vec![format!("{rung_id}-rail")]
    } else {
        previous_ids
    };

    for (output_index, output) in rung.outputs.iter().enumerate() {
        let output_id =
            output_id_of(output).unwrap_or_else(|| format!("{rung_id}-o{output_index}"));
        let output_x = output_x(rung);
        let output_y = base_y + (output_index as i32) * (CELL_H + 8);
        match output {
            OutputElement::Coil { name, variant, .. } => {
                let storage = match variant {
                    CoilVariant::Normal => "none",
                    CoilVariant::Set => "set",
                    CoilVariant::Reset => "reset",
                };
                write_start_tag_with(writer, b"coil", |attrs| {
                    attrs.push_attribute(("localId", output_id.as_str()));
                    attrs.push_attribute(("storage", storage));
                })?;
                write_position(writer, output_x, output_y)?;
                write_connections(writer, &rung_source_ids)?;
                write_variable(writer, name)?;
                write_close_tag(writer, b"coil")?;
            }
            OutputElement::Block {
                fb_type,
                instance,
                inputs,
                outputs,
                ..
            } => {
                write_start_tag_with(writer, b"block", |attrs| {
                    attrs.push_attribute(("localId", output_id.as_str()));
                    attrs.push_attribute(("instanceName", instance.as_str()));
                    attrs.push_attribute(("typeName", fb_type.as_str()));
                })?;
                write_position(writer, output_x, output_y)?;
                write_connections(writer, &rung_source_ids)?;
                for arg in inputs {
                    write_block_arg(writer, b"inVariable", &output_id, arg)?;
                }
                for arg in outputs {
                    write_block_arg(writer, b"outVariable", &output_id, arg)?;
                }
                write_close_tag(writer, b"block")?;
            }
        }
    }

    Ok(())
}

/// Write one series branch; returns the ids of the branch-tail elements.
#[allow(clippy::too_many_arguments)]
fn write_branch(
    writer: &mut Writer<&mut Vec<u8>>,
    rung_id: &str,
    branch_index: usize,
    branch: &SeriesBranch,
    branch_y: i32,
    _previous_ids: &[String],
    rail_id: &str,
) -> Result<Vec<String>, PlcopenError> {
    let mut upstream = vec![rail_id.to_string()];
    let mut last_id: Option<String> = None;

    for (contact_index, contact) in branch.elements.iter().enumerate() {
        let contact_id = contact
            .id
            .clone()
            .unwrap_or_else(|| format!("{rung_id}-c{branch_index}-{contact_index}"));
        let x = RAIL_GAP + (contact_index as i32 + 1) * CELL_W;
        write_contact(writer, &contact_id, contact, x, branch_y, &upstream)?;
        upstream = vec![contact_id.clone()];
        last_id = Some(contact_id);
    }

    Ok(vec![last_id.unwrap_or_else(|| rail_id.to_string())])
}

fn write_contact(
    writer: &mut Writer<&mut Vec<u8>>,
    id: &str,
    contact: &ContactElement,
    x: i32,
    y: i32,
    upstream: &[String],
) -> Result<(), PlcopenError> {
    let negated = if contact.negated { "true" } else { "false" };
    write_start_tag_with(writer, b"contact", |attrs| {
        attrs.push_attribute(("localId", id));
        attrs.push_attribute(("negated", negated));
    })?;
    write_position(writer, x, y)?;
    write_connections(writer, upstream)?;
    write_variable(writer, &contact.name)?;
    write_close_tag(writer, b"contact")
}

fn write_block_arg(
    writer: &mut Writer<&mut Vec<u8>>,
    tag: &[u8],
    block_id: &str,
    arg: &BlockArg,
) -> Result<(), PlcopenError> {
    write_start_tag_with(writer, tag, |attrs| {
        // localIds are identities within the body — include the owning
        // block so identical pins across blocks stay unique.
        attrs.push_attribute((
            "localId",
            format!("{block_id}-{}-{}", arg.name, arg.value).as_str(),
        ));
        attrs.push_attribute(("formalParameter", arg.name.as_str()));
    })?;
    write_position(writer, 0, 0)?;
    write_text(writer, &arg.value)?;
    write_close_tag(writer, tag)
}

fn output_id_of(output: &OutputElement) -> Option<String> {
    match output {
        OutputElement::Coil { id, .. } => id.clone(),
        OutputElement::Block { id, .. } => id.clone(),
    }
}

fn output_x(rung: &Rung) -> i32 {
    let widest = rung
        .branches
        .iter()
        .map(|branch| branch.elements.len())
        .max()
        .unwrap_or(0);
    RAIL_GAP + (widest as i32 + 1) * CELL_W + RAIL_GAP
}

// --- small writer helpers ----------------------------------------------------

fn write_event(writer: &mut Writer<&mut Vec<u8>>, event: Event<'_>) -> Result<(), PlcopenError> {
    writer
        .write_event(event)
        .map_err(|error| PlcopenError(format!("write: {error}")))
}

fn write_start_tag_with(
    writer: &mut Writer<&mut Vec<u8>>,
    tag: &[u8],
    fill: impl FnOnce(&mut BytesStart<'_>),
) -> Result<(), PlcopenError> {
    let mut start = BytesStart::new(String::from_utf8_lossy(tag));
    fill(&mut start);
    write_event(writer, Event::Start(start.into_owned()))
}

fn write_empty_tag_with(
    writer: &mut Writer<&mut Vec<u8>>,
    tag: &[u8],
    fill: impl FnOnce(&mut BytesStart<'_>),
) -> Result<(), PlcopenError> {
    let mut start = BytesStart::new(String::from_utf8_lossy(tag));
    fill(&mut start);
    write_event(writer, Event::Start(start.into_owned()))
}

fn write_empty_tag(writer: &mut Writer<&mut Vec<u8>>, tag: &[u8]) -> Result<(), PlcopenError> {
    write_empty_tag_with(writer, tag, |_| {})
}

/// Emit a self-closing element (no separate End event).
fn write_self_closing(
    writer: &mut Writer<&mut Vec<u8>>,
    tag: &[u8],
    fill: impl FnOnce(&mut BytesStart<'_>),
) -> Result<(), PlcopenError> {
    let mut start = BytesStart::new(String::from_utf8_lossy(tag));
    fill(&mut start);
    write_event(writer, Event::Empty(start.into_owned()))
}

fn write_close_tag(writer: &mut Writer<&mut Vec<u8>>, tag: &[u8]) -> Result<(), PlcopenError> {
    write_event(
        writer,
        Event::End(BytesEnd::new(String::from_utf8_lossy(tag))),
    )
}

fn write_position(writer: &mut Writer<&mut Vec<u8>>, x: i32, y: i32) -> Result<(), PlcopenError> {
    write_self_closing(writer, b"position", |attrs| {
        attrs.push_attribute(("x", x.to_string().as_str()));
        attrs.push_attribute(("y", y.to_string().as_str()));
    })
}

fn write_connections(
    writer: &mut Writer<&mut Vec<u8>>,
    upstream: &[String],
) -> Result<(), PlcopenError> {
    if upstream.is_empty() {
        return Ok(());
    }
    write_start_tag_with(writer, b"connectionPointIn", |_| {})?;
    for source in upstream {
        write_self_closing(writer, b"connection", |attrs| {
            attrs.push_attribute(("refLocalId", source.as_str()));
        })?;
    }
    write_close_tag(writer, b"connectionPointIn")
}

fn write_variable(writer: &mut Writer<&mut Vec<u8>>, name: &str) -> Result<(), PlcopenError> {
    write_start_tag_with(writer, b"variable", |_| {})?;
    write_text(writer, name)?;
    write_close_tag(writer, b"variable")
}

fn write_text(writer: &mut Writer<&mut Vec<u8>>, text: &str) -> Result<(), PlcopenError> {
    write_event(writer, Event::Text(BytesText::new(text)))
}
