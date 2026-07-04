//! MIDI input via [`midir`] (desktop only). Connects to the first available
//! input port and forwards note on/off and a few control-change messages.

use crate::params::{MidiMsg, SharedState};
use midir::{MidiInput, MidiInputConnection};

/// Keeps the MIDI connection alive. Drop it to disconnect.
pub struct MidiHandle {
    _conn: MidiInputConnection<()>,
    pub port_name: String,
}

/// Connect to the first MIDI input port and feed events into `shared`.
pub fn start(shared: SharedState) -> Result<MidiHandle, String> {
    let midi_in = MidiInput::new("hammond-b3").map_err(|e| e.to_string())?;
    let ports = midi_in.ports();
    let port = ports
        .first()
        .ok_or_else(|| "no MIDI input ports found".to_string())?;
    let port_name = midi_in.port_name(port).unwrap_or_else(|_| "unknown".into());

    tracing::info!(port = %port_name, "connecting MIDI input");

    let conn = midi_in
        .connect(
            port,
            "hammond-b3-in",
            move |_stamp, message, _| handle_message(&shared, message),
            (),
        )
        .map_err(|e| e.to_string())?;

    Ok(MidiHandle {
        _conn: conn,
        port_name,
    })
}

fn handle_message(shared: &SharedState, message: &[u8]) {
    if message.is_empty() {
        return;
    }
    let status = message[0] & 0xF0;
    match status {
        0x90 => {
            let note = message.get(1).copied().unwrap_or(0);
            let vel = message.get(2).copied().unwrap_or(0);
            if vel == 0 {
                shared.push_event(MidiMsg::NoteOff { note });
            } else {
                shared.push_event(MidiMsg::NoteOn {
                    note,
                    velocity: vel,
                });
            }
        }
        0x80 => {
            let note = message.get(1).copied().unwrap_or(0);
            shared.push_event(MidiMsg::NoteOff { note });
        }
        0xB0
            // CC 123 = All Notes Off.
            if message.get(1) == Some(&123) => {
                shared.push_event(MidiMsg::AllNotesOff);
            }
        _ => {}
    }
}
