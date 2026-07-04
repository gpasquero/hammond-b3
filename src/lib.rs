//! # hammond-b3
//!
//! A virtual Hammond B3 tonewheel organ in Rust: 9 drawbars, percussion,
//! vibrato/chorus scanner, tube overdrive and a Leslie rotary speaker, with
//! human-editable TOML presets. Runs as a native desktop app and compiles to
//! WebAssembly for an in-browser demo.
//!
//! The [`engine`] module is pure DSP with no I/O and is the same on every
//! platform; only the audio backend ([`audio`]), MIDI input ([`midi`]) and the
//! [`app`] GUI shell differ per target.

pub mod engine;
pub mod leslie;
pub mod params;
pub mod preset;

#[cfg(feature = "audio")]
pub mod audio;

#[cfg(all(feature = "desktop", not(target_arch = "wasm32")))]
pub mod midi;

#[cfg(feature = "gui")]
pub mod app;

/// WebAssembly entry point: mounts the egui app on the `<canvas id="organ">`.
#[cfg(all(target_arch = "wasm32", feature = "web"))]
#[wasm_bindgen::prelude::wasm_bindgen(start)]
pub fn start() {
    console_error_panic_hook::set_once();

    use eframe::wasm_bindgen::JsCast as _;

    let web_options = eframe::WebOptions::default();
    wasm_bindgen_futures::spawn_local(async {
        let document = eframe::web_sys::window()
            .and_then(|w| w.document())
            .expect("no document");
        let canvas = document
            .get_element_by_id("organ")
            .expect("missing <canvas id=\"organ\">")
            .dyn_into::<eframe::web_sys::HtmlCanvasElement>()
            .expect("#organ is not a canvas");

        eframe::WebRunner::new()
            .start(
                canvas,
                web_options,
                Box::new(|cc| Ok(Box::new(app::OrganApp::new(cc)))),
            )
            .await
            .expect("failed to start eframe");
    });
}
