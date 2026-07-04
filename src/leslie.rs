//! Leslie rotary-speaker simulation.
//!
//! The signal is split by a crossover into a **horn** (highs) and a **drum**
//! (lows). Each rotor is modelled with three effects driven by its rotation:
//!
//! * **Doppler** — a small modulated delay shifts the pitch as the mouth of the
//!   rotor moves toward / away from the listener.
//! * **Amplitude modulation** — the level swells as the rotor faces the mic.
//! * **Stereo panning** — two virtual mics see the rotor from opposite sides.
//!
//! Horn and drum spin at slightly different speeds and have different inertia,
//! so speed changes (slow ↔ fast ↔ brake) ramp independently, just like the
//! real cabinet spinning up and down.

use crate::preset::LeslieSpeed;
use std::f32::consts::TAU;

/// One rotating element (horn or drum) with its own Doppler delay line.
struct Rotor {
    angle: f32,
    /// Current angular speed in Hz.
    speed: f32,
    /// How quickly the speed approaches its target (per-sample smoothing).
    accel: f32,
    // Doppler delay line.
    buf: Vec<f32>,
    write: usize,
    base_delay: f32,
    depth: f32,
    inc_scale: f32,
}

impl Rotor {
    fn new(sample_rate: f32, initial_hz: f32, spinup_s: f32, depth_ms: f32) -> Self {
        let len = (sample_rate * 0.01) as usize + 4;
        Self {
            angle: 0.0,
            speed: initial_hz,
            accel: 1.0 - (-1.0 / (spinup_s * sample_rate)).exp(),
            buf: vec![0.0; len],
            write: 0,
            base_delay: sample_rate * 0.0015,
            depth: sample_rate * depth_ms * 0.001,
            inc_scale: 1.0 / sample_rate,
        }
    }

    fn set_sample_rate(&mut self, sample_rate: f32, spinup_s: f32, depth_ms: f32) {
        let len = (sample_rate * 0.01) as usize + 4;
        self.buf = vec![0.0; len];
        self.write = 0;
        self.accel = 1.0 - (-1.0 / (spinup_s * sample_rate)).exp();
        self.base_delay = sample_rate * 0.0015;
        self.depth = sample_rate * depth_ms * 0.001;
        self.inc_scale = 1.0 / sample_rate;
    }

    /// Advance the rotor and return `(doppler_sample, sin, cos)` of its angle.
    #[inline]
    fn process(&mut self, x: f32, target_hz: f32) -> (f32, f32, f32) {
        self.speed += (target_hz - self.speed) * self.accel;
        self.angle += self.speed * self.inc_scale * TAU;
        if self.angle >= TAU {
            self.angle -= TAU;
        }
        let (s, c) = self.angle.sin_cos();

        // Doppler via a delay modulated by the rotor angle.
        let len = self.buf.len();
        self.buf[self.write] = x;
        let delay = self.base_delay + self.depth * (0.5 + 0.5 * s);
        let read = self.write as f32 - delay;
        let read = if read < 0.0 { read + len as f32 } else { read };
        let i0 = read.floor() as usize % len;
        let i1 = (i0 + 1) % len;
        let frac = read - read.floor();
        let out = self.buf[i0] * (1.0 - frac) + self.buf[i1] * frac;

        self.write = (self.write + 1) % len;
        (out, s, c)
    }
}

/// The full two-rotor Leslie cabinet.
pub struct Leslie {
    horn: Rotor,
    drum: Rotor,
    // One-pole crossover state.
    lp: f32,
    xover: f32,
}

impl Leslie {
    pub fn new(sample_rate: f32) -> Self {
        Self {
            horn: Rotor::new(sample_rate, 0.8, 0.8, 0.7),
            drum: Rotor::new(sample_rate, 0.7, 2.2, 1.6),
            lp: 0.0,
            xover: crossover_coeff(sample_rate, 800.0),
        }
    }

    pub fn set_sample_rate(&mut self, sample_rate: f32) {
        self.horn.set_sample_rate(sample_rate, 0.8, 0.7);
        self.drum.set_sample_rate(sample_rate, 2.2, 1.6);
        self.xover = crossover_coeff(sample_rate, 800.0);
        self.lp = 0.0;
    }

    #[inline]
    fn targets(speed: LeslieSpeed) -> (f32, f32) {
        match speed {
            LeslieSpeed::Brake => (0.0, 0.0),
            LeslieSpeed::Slow => (0.8, 0.7),
            LeslieSpeed::Fast => (6.8, 6.0),
        }
    }

    /// Process one mono sample into a stereo pair.
    #[inline]
    pub fn process(&mut self, x: f32, speed: LeslieSpeed) -> (f32, f32) {
        let (horn_hz, drum_hz) = Self::targets(speed);

        // Crossover: one-pole low-pass for the drum, remainder for the horn.
        self.lp += self.xover * (x - self.lp);
        let low = self.lp;
        let high = x - low;

        let (hd, hs, _hc) = self.horn.process(high, horn_hz);
        let (dd, ds, _dc) = self.drum.process(low, drum_hz);

        // Amplitude modulation (level swells toward the mic).
        let horn_amp = 0.7 + 0.3 * hs;
        let drum_amp = 0.8 + 0.2 * ds;

        // Stereo: the two rotors pan in anti-phase across the mics.
        let l = hd * horn_amp * (0.5 - 0.5 * hs) + dd * drum_amp * (0.5 - 0.5 * ds);
        let r = hd * horn_amp * (0.5 + 0.5 * hs) + dd * drum_amp * (0.5 + 0.5 * ds);

        (l, r)
    }
}

#[inline]
fn crossover_coeff(sample_rate: f32, cutoff_hz: f32) -> f32 {
    let x = (-TAU * cutoff_hz / sample_rate).exp();
    1.0 - x
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn leslie_is_finite_and_bounded() {
        let mut l = Leslie::new(48_000.0);
        for i in 0..48_000 {
            let x = ((i as f32) * 0.01).sin();
            let (a, b) = l.process(x, LeslieSpeed::Fast);
            assert!(a.is_finite() && b.is_finite());
            assert!(a.abs() < 4.0 && b.abs() < 4.0);
        }
    }

    #[test]
    fn brake_still_processes() {
        let mut l = Leslie::new(44_100.0);
        let (a, b) = l.process(1.0, LeslieSpeed::Brake);
        assert!(a.is_finite() && b.is_finite());
    }
}
