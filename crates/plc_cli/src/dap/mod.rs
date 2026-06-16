//! `plc debug`: a Debug Adapter Protocol server over stdio.
//!
//! stdout carries only framed DAP messages; diagnostics go to stderr. A reader
//! thread frames stdin requests and a forwarder thread relays worker pauses, so
//! the single-threaded event loop can interleave editor requests with execution
//! events without one blocking the other.

mod protocol;
mod session;

use std::io;
use std::sync::mpsc;

use plc_runtime::PauseEvent;
use serde_json::Value;

use session::Session;

/// A unit of work for the event loop, merging editor requests with worker
/// pause/termination signals onto one channel.
pub(crate) enum Incoming {
    /// A framed DAP request read from stdin.
    Client(Value),
    /// stdin closed (editor went away).
    ClientClosed,
    /// The worker paused at a breakpoint / step.
    Pause(PauseEvent),
    /// The worker finished all scan cycles.
    Terminated,
}

/// Serve the Debug Adapter Protocol over stdio until the editor disconnects or
/// stdin closes.
pub fn run() -> Result<(), String> {
    let (incoming_tx, incoming_rx) = mpsc::channel::<Incoming>();

    // Reader thread: frame stdin requests onto the loop. Decoupling reads from
    // the pause-event stream means a paused worker never blocks input.
    let reader_tx = incoming_tx.clone();
    std::thread::spawn(move || {
        let stdin = io::stdin();
        let mut reader = stdin.lock();
        loop {
            match protocol::read_message(&mut reader) {
                Ok(Some(message)) => {
                    if reader_tx.send(Incoming::Client(message)).is_err() {
                        return;
                    }
                }
                Ok(None) | Err(_) => {
                    let _ = reader_tx.send(Incoming::ClientClosed);
                    return;
                }
            }
        }
    });

    let stdout = io::stdout();
    let mut writer = stdout.lock();
    let mut session = Session::new();

    while let Ok(incoming) = incoming_rx.recv() {
        let outcome = match incoming {
            Incoming::Client(message) => session
                .handle_client(&message, &incoming_tx, &mut writer)
                .map(|flow| flow.is_break()),
            Incoming::Pause(event) => session.handle_pause(event, &mut writer).map(|_| false),
            Incoming::Terminated => session.handle_terminated(&mut writer).map(|_| false),
            Incoming::ClientClosed => break,
        };
        match outcome {
            Ok(true) => break, // editor disconnected
            Ok(false) => {}
            Err(error) => return Err(format!("debug adapter I/O error: {error}")),
        }
    }
    Ok(())
}
