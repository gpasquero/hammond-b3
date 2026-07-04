//! Native desktop entry point: sets up logging, starts real-time audio and MIDI
//! input, then runs the egui GUI.

use hammond_b3::app::OrganApp;

fn main() -> eframe::Result<()> {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    let native_options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_inner_size([920.0, 580.0])
            .with_min_inner_size([720.0, 480.0])
            .with_title("Hammond B3"),
        ..Default::default()
    };

    eframe::run_native(
        "Hammond B3",
        native_options,
        Box::new(|cc| {
            let app = OrganApp::new(cc);
            let shared = app.shared();

            let app = match hammond_b3::audio::start(shared.clone()) {
                Ok(handle) => app.with_audio(handle),
                Err(e) => {
                    tracing::error!("could not start audio: {e}");
                    app
                }
            };

            let midi = hammond_b3::midi::start(shared).ok();
            let app = app.with_midi(midi);

            Ok(Box::new(app))
        }),
    )
}
