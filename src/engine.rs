//! The tonewheel synthesis engine.
//!
//! Signal flow (mono until the Leslie):
//! ```text
//! tonewheels × drawbars ─┐
//! percussion ────────────┤→ vibrato/chorus → overdrive → Leslie → stereo out
//! key click + leakage ───┘
//! ```
//!
//! The engine models a bank of continuously spinning tonewheels (as in a real
//! B3, the wheels never stop). Pressing a key simply *gates* a set of wheels
//! selected by the drawbar footages, which is why notes sharing a wheel phase-
//! lock together — a big part of the Hammond character.

use crate::leslie::Leslie;
use crate::preset::{Patch, VibratoMode};

use std::f32::consts::TAU;

/// Semitone offset of each drawbar relative to the 8' unison, in classic order:
/// `16', 5⅓', 8', 4', 2⅔', 2', 1⅗', 1⅓', 1'`.
const FOOTAGE_OFFSETS: [i32; 9] = [-12, 7, 0, 12, 19, 24, 28, 31, 36];

/// Lowest and highest tonewheel id (a MIDI note number). 91 wheels, 24..=114.
const WHEEL_MIN: i32 = 24;
const WHEEL_MAX: i32 = 114;
const WHEEL_SLOTS: usize = 128;

/// Convert a MIDI note number to frequency in Hz (A4 = note 69 = 440 Hz).
#[inline]
pub fn midi_to_freq(note: f32) -> f32 {
    440.0 * 2f32.powf((note - 69.0) / 12.0)
}

/// Fold a wheel id back into the valid range, one octave at a time. This is the
/// Hammond "foldback" that keeps the extreme drawbars within the generator.
#[inline]
pub fn foldback(mut id: i32) -> i32 {
    while id < WHEEL_MIN {
        id += 12;
    }
    while id > WHEEL_MAX {
        id -= 12;
    }
    id
}

/// A bank of free-running sine tonewheels.
struct WheelBank {
    phase: [f32; WHEEL_SLOTS],
    inc: [f32; WHEEL_SLOTS],
    value: [f32; WHEEL_SLOTS],
}

impl WheelBank {
    fn new(sample_rate: f32) -> Self {
        let mut inc = [0.0f32; WHEEL_SLOTS];
        for (id, slot) in inc.iter_mut().enumerate() {
            if (WHEEL_MIN..=WHEEL_MAX).contains(&(id as i32)) {
                *slot = midi_to_freq(id as f32) / sample_rate;
            }
        }
        Self {
            phase: [0.0; WHEEL_SLOTS],
            inc,
            value: [0.0; WHEEL_SLOTS],
        }
    }

    fn set_sample_rate(&mut self, sample_rate: f32) {
        for id in WHEEL_MIN..=WHEEL_MAX {
            self.inc[id as usize] = midi_to_freq(id as f32) / sample_rate;
        }
    }

    /// Advance every wheel by one sample and cache its current amplitude.
    #[inline]
    fn tick(&mut self) {
        for id in WHEEL_MIN..=WHEEL_MAX {
            let i = id as usize;
            let mut p = self.phase[i] + self.inc[i];
            if p >= 1.0 {
                p -= 1.0;
            }
            self.phase[i] = p;
            self.value[i] = (p * TAU).sin();
        }
    }

    #[inline]
    fn sample(&self, id: i32) -> f32 {
        self.value[id as usize]
    }
}

/// State of one held key.
#[derive(Clone, Copy)]
struct Voice {
    note: u8,
    /// Amplitude envelope (fast attack/release to avoid clicks; the audible
    /// "key click" transient is added separately).
    env: f32,
    /// `true` while the key is down.
    gate: bool,
    velocity: f32,
}

/// A simple xorshift PRNG so key-click noise works identically on native & wasm
/// (no `rand`, no `Math.random`).
struct Rng(u32);
impl Rng {
    #[inline]
    fn next_bipolar(&mut self) -> f32 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.0 = x;
        (x as f32 / u32::MAX as f32) * 2.0 - 1.0
    }
}

/// A modulated delay line implementing the scanner vibrato/chorus.
struct Vibrato {
    buf: Vec<f32>,
    write: usize,
    lfo_phase: f32,
    lfo_inc: f32,
    base_delay: f32,
}

impl Vibrato {
    fn new(sample_rate: f32) -> Self {
        let len = (sample_rate * 0.01) as usize + 4; // ~10 ms line
        Self {
            buf: vec![0.0; len],
            write: 0,
            lfo_phase: 0.0,
            lfo_inc: 6.9 / sample_rate, // ~6.9 Hz scanner
            base_delay: sample_rate * 0.0016,
        }
    }

    fn set_sample_rate(&mut self, sample_rate: f32) {
        let len = (sample_rate * 0.01) as usize + 4;
        self.buf = vec![0.0; len];
        self.write = 0;
        self.lfo_inc = 6.9 / sample_rate;
        self.base_delay = sample_rate * 0.0016;
    }

    #[inline]
    fn process(&mut self, x: f32, mode: VibratoMode) -> f32 {
        let len = self.buf.len();
        self.buf[self.write] = x;

        self.lfo_phase += self.lfo_inc;
        if self.lfo_phase >= 1.0 {
            self.lfo_phase -= 1.0;
        }

        if mode == VibratoMode::Off {
            self.write = (self.write + 1) % len;
            return x;
        }

        let (depth, dry) = mode.depth_and_dry();
        let mod_samples = self.base_delay * (1.0 + depth * (self.lfo_phase * TAU).sin());
        let read = self.write as f32 - mod_samples;
        let read = if read < 0.0 { read + len as f32 } else { read };

        let i0 = read.floor() as usize % len;
        let i1 = (i0 + 1) % len;
        let frac = read - read.floor();
        let wet = self.buf[i0] * (1.0 - frac) + self.buf[i1] * frac;

        self.write = (self.write + 1) % len;
        dry * x + (1.0 - dry) * wet
    }
}

/// The full organ engine.
pub struct Engine {
    sample_rate: f32,
    wheels: WheelBank,
    voices: Vec<Voice>,
    held: usize,
    // Percussion
    perc_env: f32,
    perc_note: u8,
    // Key click
    click_env: f32,
    rng: Rng,
    // Effects
    vibrato: Vibrato,
    leslie: Leslie,
}

impl Engine {
    pub fn new(sample_rate: f32) -> Self {
        Self {
            sample_rate,
            wheels: WheelBank::new(sample_rate),
            voices: Vec::with_capacity(32),
            held: 0,
            perc_env: 0.0,
            perc_note: 60,
            click_env: 0.0,
            rng: Rng(0x1234_5678),
            vibrato: Vibrato::new(sample_rate),
            leslie: Leslie::new(sample_rate),
        }
    }

    pub fn sample_rate(&self) -> f32 {
        self.sample_rate
    }

    pub fn set_sample_rate(&mut self, sample_rate: f32) {
        self.sample_rate = sample_rate;
        self.wheels.set_sample_rate(sample_rate);
        self.vibrato.set_sample_rate(sample_rate);
        self.leslie.set_sample_rate(sample_rate);
    }

    pub fn note_on(&mut self, note: u8, velocity: u8) {
        let was_silent = self.held == 0;
        let vel = (velocity as f32 / 127.0).max(0.05);

        if let Some(v) = self.voices.iter_mut().find(|v| v.note == note) {
            v.gate = true;
            v.velocity = vel;
        } else {
            self.voices.push(Voice {
                note,
                env: 0.0,
                gate: true,
                velocity: vel,
            });
        }
        self.held += 1;

        // Key click: a short contact transient on every key press.
        self.click_env = 1.0;

        // Percussion is single-triggered: it only re-fires from silence.
        if was_silent {
            self.perc_env = 1.0;
            self.perc_note = note;
        }
    }

    pub fn note_off(&mut self, note: u8) {
        if let Some(v) = self.voices.iter_mut().find(|v| v.note == note && v.gate) {
            v.gate = false;
            self.held = self.held.saturating_sub(1);
        }
    }

    pub fn all_notes_off(&mut self) {
        for v in &mut self.voices {
            v.gate = false;
        }
        self.held = 0;
    }

    /// Render one stereo sample.
    #[inline]
    pub fn process(&mut self, patch: &Patch) -> (f32, f32) {
        self.wheels.tick();

        // Envelope slew coefficients (~4 ms attack, ~8 ms release).
        let atk = 1.0 - (-1.0 / (0.004 * self.sample_rate)).exp();
        let rel = 1.0 - (-1.0 / (0.008 * self.sample_rate)).exp();

        let mut dry = 0.0f32;

        // --- Tonewheels gated by drawbars, per held voice ---
        let mut i = 0;
        while i < self.voices.len() {
            let v = &mut self.voices[i];
            let target = if v.gate { 1.0 } else { 0.0 };
            let coeff = if target > v.env { atk } else { rel };
            v.env += (target - v.env) * coeff;

            if !v.gate && v.env < 1.0e-4 {
                self.voices.swap_remove(i);
                continue;
            }

            let note = v.note as i32;
            let mut voice_sum = 0.0f32;
            for (d, &off) in FOOTAGE_OFFSETS.iter().enumerate() {
                let g = patch.drawbar_gain(d);
                if g > 0.0 {
                    let id = foldback(note + off);
                    voice_sum += self.wheels.sample(id) * g;
                }
            }
            dry += voice_sum * v.env * v.velocity;
            i += 1;
        }
        // Normalize for the 9 drawbars so a full registration stays in range.
        dry *= 0.11;

        // --- Percussion ---
        if patch.percussion.on {
            let tau = if patch.percussion.fast { 0.18 } else { 0.55 };
            let decay = (-1.0 / (tau * self.sample_rate)).exp();
            let off = if patch.percussion.third { 19 } else { 12 };
            let id = foldback(self.perc_note as i32 + off);
            let level = if patch.percussion.soft { 0.35 } else { 0.8 };
            dry += self.wheels.sample(id) * self.perc_env * level;
            self.perc_env *= decay;
        }

        // --- Key click (filtered noise burst) ---
        if self.click_env > 1.0e-4 {
            let click_decay = (-1.0 / (0.003 * self.sample_rate)).exp();
            dry += self.rng.next_bipolar() * self.click_env * 0.12;
            self.click_env *= click_decay;
        }

        // --- Tonewheel leakage / hum (all wheels bleed a little) ---
        // Cheap approximation: a touch of the fundamental bus is always present.
        dry += 0.0008 * self.wheels.sample(foldback(self.perc_note as i32));

        // --- Vibrato / chorus scanner ---
        let mono = self.vibrato.process(dry, patch.vibrato);

        // --- Overdrive (tube-style soft clip) ---
        let mono = overdrive(mono, patch.overdrive);

        // --- Leslie rotary speaker → stereo ---
        let (mut l, mut r) = self.leslie.process(mono, patch.leslie);

        l = soft_clip(l * patch.volume);
        r = soft_clip(r * patch.volume);
        (l, r)
    }
}

/// A soft output limiter: fully transparent below ±0.9, smoothly bounded to ±1.
#[inline]
pub fn soft_clip(x: f32) -> f32 {
    const T: f32 = 0.9;
    if x.abs() <= T {
        x
    } else {
        let over = (x.abs() - T) / (1.0 - T);
        x.signum() * (T + (1.0 - T) * over.tanh())
    }
}

/// Tube-style overdrive. `amount` in `0.0..=1.0`.
#[inline]
pub fn overdrive(x: f32, amount: f32) -> f32 {
    if amount <= 0.0 {
        return x;
    }
    let drive = 1.0 + amount * 12.0;
    let shaped = (x * drive).tanh();
    // Keep unity-ish output so raising drive doesn't just get quieter.
    let comp = 1.0 / (drive.tanh().max(0.05));
    // Blend a bit of dry to preserve dynamics.
    let wet = shaped * comp;
    x * (1.0 - amount) + wet * amount
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::preset::Patch;

    #[test]
    fn a4_is_440() {
        assert!((midi_to_freq(69.0) - 440.0).abs() < 1e-3);
    }

    #[test]
    fn foldback_keeps_ids_in_range() {
        for id in -40..200 {
            let f = foldback(id);
            assert!(
                (WHEEL_MIN..=WHEEL_MAX).contains(&f),
                "id {id} folded to {f}"
            );
        }
    }

    #[test]
    fn foldback_preserves_pitch_class() {
        for id in -40..200 {
            assert_eq!((foldback(id) - id).rem_euclid(12), 0);
        }
    }

    fn rms(engine: &mut Engine, patch: &Patch, n: usize) -> f32 {
        let mut acc = 0.0f64;
        for _ in 0..n {
            let (l, r) = engine.process(patch);
            assert!(l.is_finite() && r.is_finite(), "non-finite sample");
            acc += ((l + r) * 0.5) as f64 * ((l + r) * 0.5) as f64;
        }
        (acc / n as f64).sqrt() as f32
    }

    #[test]
    fn silence_when_nothing_pressed() {
        let mut e = Engine::new(48_000.0);
        let patch = Patch::default();
        // Warm up the free-running wheels, then measure with no keys.
        let level = rms(&mut e, &patch, 4_800);
        assert!(level < 1e-3, "expected near silence, got rms {level}");
    }

    #[test]
    fn produces_sound_when_note_held() {
        let mut e = Engine::new(48_000.0);
        let patch = Patch::default();
        e.note_on(69, 100);
        let level = rms(&mut e, &patch, 9_600);
        assert!(level > 1e-2, "expected audible signal, got rms {level}");
    }

    #[test]
    fn all_drawbars_off_is_quiet() {
        let mut e = Engine::new(48_000.0);
        let mut patch = Patch {
            drawbars: [0; 9],
            ..Patch::default()
        };
        patch.percussion.on = false;
        e.note_on(60, 100);
        // Skip the initial key-click transient.
        rms(&mut e, &patch, 2_000);
        let level = rms(&mut e, &patch, 4_800);
        assert!(
            level < 5e-3,
            "expected quiet with no drawbars, got rms {level}"
        );
    }

    #[test]
    fn output_never_clips_hard() {
        let mut e = Engine::new(48_000.0);
        let patch = Patch {
            drawbars: [8; 9],
            overdrive: 1.0,
            volume: 1.0,
            ..Patch::default()
        };
        for n in 60..72 {
            e.note_on(n, 127);
        }
        for _ in 0..48_000 {
            let (l, r) = e.process(&patch);
            assert!(
                l.abs() <= 1.001 && r.abs() <= 1.001,
                "runaway output: {l}, {r}"
            );
        }
    }
}
