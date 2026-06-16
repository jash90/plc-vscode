//! Debug Adapter Protocol wire framing: `Content-Length: N\r\n\r\n<json>` over a
//! byte stream (the same framing LSP uses). Bodies are arbitrary JSON, so we
//! work in terms of [`serde_json::Value`] rather than fully-typed messages.

use std::io::{self, BufRead, Write};

use serde_json::Value;

/// Read one framed DAP message. Returns `Ok(None)` at a clean EOF (no more
/// messages); `Err` only on malformed framing or I/O failure.
pub fn read_message<R: BufRead>(reader: &mut R) -> io::Result<Option<Value>> {
    let mut content_length: Option<usize> = None;
    loop {
        let mut line = String::new();
        if reader.read_line(&mut line)? == 0 {
            return Ok(None); // EOF before/between messages.
        }
        let header = line.trim_end_matches(['\r', '\n']);
        if header.is_empty() {
            break; // Blank line terminates the header block.
        }
        if let Some(value) = header.strip_prefix("Content-Length:") {
            content_length = value.trim().parse::<usize>().ok();
        }
        // Other headers (e.g. Content-Type) are ignored.
    }

    let length = content_length.ok_or_else(|| {
        io::Error::new(io::ErrorKind::InvalidData, "missing Content-Length header")
    })?;
    let mut body = vec![0u8; length];
    reader.read_exact(&mut body)?;
    let message = serde_json::from_slice(&body)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    Ok(Some(message))
}

/// Write one framed DAP message and flush it.
pub fn write_message<W: Write + ?Sized>(writer: &mut W, message: &Value) -> io::Result<()> {
    let body = serde_json::to_vec(message)?;
    write!(writer, "Content-Length: {}\r\n\r\n", body.len())?;
    writer.write_all(&body)?;
    writer.flush()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::io::Cursor;

    #[test]
    fn round_trips_a_message() {
        let mut buffer = Vec::new();
        let message = json!({ "type": "event", "event": "stopped", "seq": 3 });
        write_message(&mut buffer, &message).unwrap();

        let framed = String::from_utf8(buffer.clone()).unwrap();
        assert!(framed.starts_with("Content-Length: "));
        assert!(framed.contains("\r\n\r\n"));

        let mut reader = Cursor::new(buffer);
        let decoded = read_message(&mut reader).unwrap().unwrap();
        assert_eq!(decoded, message);
        // A second read sees EOF.
        assert!(read_message(&mut reader).unwrap().is_none());
    }
}
