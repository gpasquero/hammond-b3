//! Real-time audio output via [`cpal`] (native CoreAudio/WASAPI/ALSA and the
//! WebAudio backend on wasm share this exact code path).

use crate::engine::Engine;
use crate::params::{MidiMsg, SharedState};
use crate::preset::Patch;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, Sample, SizedSample, StreamConfig};

/// A handle that keeps the audio stream alive. Drop it to stop audio.
pub struct AudioHandle {
    _stream: cpal::Stream,
    pub sample_rate: f32,
    pub channels: u16,
}

impl AudioHandle {
    /// Re-`play()` the stream. On the web this resumes the browser's
    /// `AudioContext`, which starts suspended and must be kicked from within a
    /// user gesture — so the GUI calls this on every interaction until sound
    /// flows. A no-op on native (the stream is already running).
    pub fn resume(&self) {
        let _ = self._stream.play();
    }
}

/// Start audio output, rendering the organ driven by `shared`.
pub fn start(shared: SharedState) -> Result<AudioHandle, String> {
    let host = cpal::default_host();
    let device = host
        .default_output_device()
        .ok_or_else(|| "no output audio device available".to_string())?;
    let supported = device
        .default_output_config()
        .map_err(|e| format!("default output config error: {e}"))?;

    let sample_format = supported.sample_format();
    let config: StreamConfig = supported.config();
    let sample_rate = config.sample_rate.0 as f32;
    let channels = config.channels;

    tracing::info!(
        device = device.name().unwrap_or_default(),
        sample_rate,
        channels,
        ?sample_format,
        "starting audio stream"
    );

    let stream = match sample_format {
        cpal::SampleFormat::F32 => build_stream::<f32>(&device, &config, shared)?,
        cpal::SampleFormat::I16 => build_stream::<i16>(&device, &config, shared)?,
        cpal::SampleFormat::U16 => build_stream::<u16>(&device, &config, shared)?,
        other => return Err(format!("unsupported sample format: {other:?}")),
    };

    stream
        .play()
        .map_err(|e| format!("failed to play stream: {e}"))?;

    Ok(AudioHandle {
        _stream: stream,
        sample_rate,
        channels,
    })
}

fn build_stream<T>(
    device: &cpal::Device,
    config: &StreamConfig,
    shared: SharedState,
) -> Result<cpal::Stream, String>
where
    T: SizedSample + FromSample<f32>,
{
    let sample_rate = config.sample_rate.0 as f32;
    let channels = config.channels as usize;

    let mut engine = Engine::new(sample_rate);
    let mut patch = shared.try_patch().unwrap_or_default();
    let mut events: Vec<MidiMsg> = Vec::with_capacity(64);

    let err_fn = |e| tracing::error!("audio stream error: {e}");

    device
        .build_output_stream(
            config,
            move |output: &mut [T], _: &cpal::OutputCallbackInfo| {
                render(
                    &mut engine,
                    &shared,
                    &mut patch,
                    &mut events,
                    channels,
                    output,
                );
            },
            err_fn,
            None,
        )
        .map_err(|e| format!("failed to build output stream: {e}"))
}

/// Fill one audio buffer. Kept generic so tests can exercise it without cpal.
fn render<T: Sample + FromSample<f32>>(
    engine: &mut Engine,
    shared: &SharedState,
    patch: &mut Patch,
    events: &mut Vec<MidiMsg>,
    channels: usize,
    output: &mut [T],
) {
    // Pick up the latest patch (reuse the last one if the lock is busy).
    if let Some(p) = shared.try_patch() {
        *patch = p;
    }
    // Apply queued note events.
    events.clear();
    shared.drain_events(events);
    for msg in events.drain(..) {
        match msg {
            MidiMsg::NoteOn { note, velocity } => engine.note_on(note, velocity),
            MidiMsg::NoteOff { note } => engine.note_off(note),
            MidiMsg::AllNotesOff => engine.all_notes_off(),
        }
    }

    for frame in output.chunks_mut(channels.max(1)) {
        let (l, r) = engine.process(patch);
        for (ch, slot) in frame.iter_mut().enumerate() {
            let v = if ch % 2 == 0 { l } else { r };
            *slot = T::from_sample(v);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_fills_buffer_without_cpal() {
        let shared = SharedState::default();
        shared.push_event(MidiMsg::NoteOn {
            note: 60,
            velocity: 100,
        });
        let mut engine = Engine::new(48_000.0);
        let mut patch = Patch::default();
        let mut events = Vec::new();
        let mut buf = vec![0.0f32; 512 * 2];
        render(&mut engine, &shared, &mut patch, &mut events, 2, &mut buf);
        assert!(buf.iter().all(|s| s.is_finite()));
    }
}
