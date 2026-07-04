<div align="center">

# 🎹 Hammond B3 — a tonewheel organ in Rust

**A virtual Hammond B3 organ with drawbars, a Leslie rotary speaker, percussion, vibrato/chorus and tube overdrive — written in pure Rust, playable natively *and* in your browser via WebAssembly.**

[![CI](https://github.com/gpasquero/hammond-b3/actions/workflows/ci.yml/badge.svg)](https://github.com/gpasquero/hammond-b3/actions/workflows/ci.yml)
[![Deploy web demo](https://github.com/gpasquero/hammond-b3/actions/workflows/pages.yml/badge.svg)](https://github.com/gpasquero/hammond-b3/actions/workflows/pages.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-stable-orange.svg?logo=rust)](https://www.rust-lang.org)
[![WebAssembly](https://img.shields.io/badge/WebAssembly-demo-654ff0.svg?logo=webassembly&logoColor=white)](https://gpasquero.github.io/hammond-b3/)
[![egui](https://img.shields.io/badge/GUI-egui-blue.svg)](https://github.com/emilk/egui)

### ▶️ **[Play the live demo](https://gpasquero.github.io/hammond-b3/)** — no install, runs in your browser

</div>

---

## ✨ Features

- 🎚️ **Nine drawbars** (`16' 5⅓' 8' 4' 2⅔' 2' 1⅗' 1⅓' 1'`) with authentic tonewheel foldback
- 🌀 **Leslie rotary speaker** — independent horn & drum rotors, Doppler pitch shift, amplitude swell, stereo micing, and inertial **slow / fast / brake** ramps
- 🥁 **Percussion** — single-triggered 2nd / 3rd harmonic, soft / normal, fast / slow decay
- 〰️ **Vibrato / Chorus scanner** — the classic `V1–V3` / `C1–C3` selector
- 🎛️ **Tube overdrive** with a musical soft-limiter on the output
- 🔑 **Key click** and tonewheel **leakage** for that vintage grit
- 💾 **Presets** in human-editable **TOML** (four factory patches included)
- 🎹 **MIDI input** (desktop) + an **on-screen / computer-keyboard** so the web demo needs no hardware
- 🦀 **One pure-Rust DSP engine** shared by the native app and the WebAssembly build

## 🏛️ The look

A skeuomorphic wood-and-brass cabinet inspired by the classic B3 console: coloured drawbars, tab switches, a Leslie control and a playable manual — rendered with [`egui`](https://github.com/emilk/egui).

## 🚀 Quick start

### Desktop (native audio + MIDI)

```bash
cargo run --release
```

Plug in a MIDI controller (auto-connects to the first port) or play with your computer keyboard.

### Web demo (WebAssembly)

```bash
cargo install trunk
rustup target add wasm32-unknown-unknown
trunk serve            # open http://127.0.0.1:8080
```

Every push to `main` also auto-deploys the demo to **GitHub Pages** via `.github/workflows/pages.yml`.

## 🎮 Controls

| Action            | How |
|-------------------|-----|
| Play notes        | On-screen keyboard, computer keys `A W S E D F T G Y H U J K`, or MIDI |
| Change octave     | **Octave −/+** buttons |
| Drawbars          | Drag each bar up/down (0–8) |
| Leslie            | **Brake / Slow / Fast** |
| Percussion        | On, 2nd/3rd, Fast/Slow |
| Vibrato/Chorus    | `OFF · V-1 · V-2 · V-3 · C-1 · C-2 · C-3` |
| Presets           | Pick + **Load**, or name + **Save** |

## 🎼 Presets

Factory presets live in [`presets/`](presets/) as editable TOML:

```toml
name = "Full Jazz"
drawbars = [8, 8, 8, 0, 0, 0, 0, 0, 0]
vibrato = "c3"
leslie = "slow"
overdrive = 0.1
volume = 0.8

[percussion]
on = true
soft = false
fast = true
third = true
```

Regenerate them from code with `cargo run --example export_presets --no-default-features`.

## 🧱 Architecture

```
tonewheels × drawbars ─┐
percussion ────────────┤→ vibrato/chorus → overdrive → Leslie → soft-limit → stereo out
key click + leakage ───┘
```

| Module | Responsibility |
|--------|----------------|
| `engine` | Free-running tonewheel bank, drawbars, voices, percussion, key click, vibrato, overdrive |
| `leslie` | Two-rotor rotary-speaker simulation |
| `preset` | Patch data model + TOML (de)serialization + factory presets |
| `params` | Lock-free-ish state shared between UI/MIDI and the real-time audio thread |
| `audio`  | [`cpal`] output stream (native + WebAudio) |
| `midi`   | [`midir`] MIDI input (desktop) |
| `app`    | [`egui`]/[`eframe`] GUI + on-screen keyboard |

The audio callback never allocates or blocks: it *tries* to read the latest patch and drains a small event queue each block.

## 🧪 Development

```bash
cargo test --no-default-features        # fast engine tests (no system deps)
cargo clippy --all-targets -- -D warnings
cargo fmt --all
```

## 📜 License

MIT © gpasquero

---

<div align="center">

**Topics:** `rust` · `hammond` · `hammond-b3` · `organ` · `tonewheel` · `synthesizer` · `synth` · `virtual-instrument` · `leslie` · `rotary-speaker` · `drawbars` · `audio` · `dsp` · `audio-synthesis` · `music` · `midi` · `webassembly` · `wasm` · `egui` · `cpal` · `real-time-audio` · `vst-alternative` · `music-production`

*Built with 🦀 Rust, 🎚️ cpal, 🎹 midir and 🖼️ egui.*

</div>
