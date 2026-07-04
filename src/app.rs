//! egui front-end with a skeuomorphic Hammond look (wood cabinet, coloured
//! drawbars, tab switches, Leslie control) plus an on-screen keyboard so the
//! web demo is playable without any MIDI hardware.

use crate::params::{MidiMsg, SharedState};
use crate::preset::{factory_presets, LeslieSpeed, Patch, Preset, VibratoMode};

use egui::{Color32, FontId, Pos2, Rect, Rounding, Sense, Stroke, Vec2};
use std::collections::HashSet;

// --- Palette (from the classic wood-and-brass aesthetic) ---
const WOOD_DARK: Color32 = Color32::from_rgb(0x4a, 0x2c, 0x14);
const WOOD: Color32 = Color32::from_rgb(0x6b, 0x44, 0x23);
const PANEL: Color32 = Color32::from_rgb(0x1a, 0x15, 0x12);
const CREAM: Color32 = Color32::from_rgb(0xef, 0xe6, 0xc8);
const BRASS: Color32 = Color32::from_rgb(0xb8, 0x93, 0x3f);
const KEY_BLACK: Color32 = Color32::from_rgb(0x18, 0x14, 0x10);

/// Classic drawbar colours: white (8'), brown (16', 5⅓') and black (mutations).
fn drawbar_color(i: usize) -> Color32 {
    match i {
        0 | 1 => Color32::from_rgb(0x6a, 0x3c, 0x1e), // brown
        2 | 3 | 5 => CREAM,                           // white
        _ => Color32::from_rgb(0x20, 0x20, 0x20),     // black mutations
    }
}

const DRAWBAR_LABELS: [&str; 9] = ["16'", "5⅓'", "8'", "4'", "2⅔'", "2'", "1⅗'", "1⅓'", "1'"];

// Computer-keyboard → semitone mapping (one octave, piano layout).
const KEY_MAP: &[(egui::Key, i32)] = &[
    (egui::Key::A, 0),
    (egui::Key::W, 1),
    (egui::Key::S, 2),
    (egui::Key::E, 3),
    (egui::Key::D, 4),
    (egui::Key::F, 5),
    (egui::Key::T, 6),
    (egui::Key::G, 7),
    (egui::Key::Y, 8),
    (egui::Key::H, 9),
    (egui::Key::U, 10),
    (egui::Key::J, 11),
    (egui::Key::K, 12),
];

pub struct OrganApp {
    shared: SharedState,
    patch: Patch,
    presets: Vec<Preset>,
    selected: usize,
    preset_name: String,
    octave: i32,
    audio_started: bool,
    status: String,
    // Currently sounding notes, so we send clean note-on/off edges.
    active_keyboard: HashSet<u8>,
    mouse_note: Option<u8>,

    #[cfg(feature = "audio")]
    _audio: Option<crate::audio::AudioHandle>,
    #[cfg(all(feature = "desktop", not(target_arch = "wasm32")))]
    _midi: Option<crate::midi::MidiHandle>,
}

impl OrganApp {
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        let presets = factory_presets();
        let patch = presets.first().map(|p| p.patch.clone()).unwrap_or_default();
        let shared = SharedState::new(patch.clone());

        Self {
            shared,
            patch,
            presets,
            selected: 0,
            preset_name: "My Preset".to_string(),
            octave: 0,
            audio_started: false,
            status: "Click any key or press A–K to start".to_string(),
            active_keyboard: HashSet::new(),
            mouse_note: None,
            #[cfg(feature = "audio")]
            _audio: None,
            #[cfg(all(feature = "desktop", not(target_arch = "wasm32")))]
            _midi: None,
        }
    }

    /// Attach an already-running audio handle (used by the native binary).
    #[cfg(feature = "audio")]
    pub fn with_audio(mut self, handle: crate::audio::AudioHandle) -> Self {
        self.audio_started = true;
        self._audio = Some(handle);
        self
    }

    #[cfg(all(feature = "desktop", not(target_arch = "wasm32")))]
    pub fn with_midi(mut self, handle: Option<crate::midi::MidiHandle>) -> Self {
        if let Some(h) = &handle {
            self.status = format!("MIDI: {}", h.port_name);
        }
        self._midi = handle;
        self
    }

    pub fn shared(&self) -> SharedState {
        self.shared.clone()
    }

    /// Lazily start audio on the first user gesture (required by browsers).
    fn ensure_audio(&mut self) {
        if self.audio_started {
            return;
        }
        self.audio_started = true;
        #[cfg(feature = "audio")]
        {
            match crate::audio::start(self.shared.clone()) {
                Ok(handle) => {
                    self.status = format!("Audio @ {} Hz", handle.sample_rate as u32);
                    self._audio = Some(handle);
                }
                Err(e) => self.status = format!("Audio error: {e}"),
            }
        }
    }

    fn sync_patch(&self) {
        self.shared.set_patch(self.patch.clone());
    }

    fn note_on(&mut self, note: u8) {
        self.ensure_audio();
        self.shared.push_event(MidiMsg::NoteOn {
            note,
            velocity: 100,
        });
    }

    fn note_off(&mut self, note: u8) {
        self.shared.push_event(MidiMsg::NoteOff { note });
    }

    fn handle_computer_keyboard(&mut self, ctx: &egui::Context) {
        let base = 60 + self.octave * 12;
        let mut desired: HashSet<u8> = HashSet::new();
        ctx.input(|i| {
            for (key, semis) in KEY_MAP {
                if i.key_down(*key) {
                    let n = base + *semis;
                    if (0..=127).contains(&n) {
                        desired.insert(n as u8);
                    }
                }
            }
        });
        let newly: Vec<u8> = desired.difference(&self.active_keyboard).copied().collect();
        let released: Vec<u8> = self.active_keyboard.difference(&desired).copied().collect();
        for n in newly {
            self.note_on(n);
        }
        for n in released {
            self.note_off(n);
        }
        self.active_keyboard = desired;
    }
}

impl eframe::App for OrganApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Repaint at ~30 fps: enough for the keyboard highlight while leaving
        // the main thread free for the audio callback. On the web build the
        // audio runs on the main thread, so an uncapped repaint rate is the
        // main cause of choppy sound — throttling it keeps the output smooth.
        ctx.request_repaint_after(std::time::Duration::from_millis(33));
        self.handle_computer_keyboard(ctx);

        egui::CentralPanel::default()
            .frame(egui::Frame::none().fill(WOOD_DARK))
            .show(ctx, |ui| {
                header(ui);
                ui.add_space(6.0);
                self.control_panel(ui);
                ui.add_space(10.0);
                self.keyboard(ui);
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new(&self.status).color(CREAM).small());
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("Octave +").clicked() {
                            self.octave = (self.octave + 1).min(3);
                        }
                        if ui.button("Octave −").clicked() {
                            self.octave = (self.octave - 1).max(-3);
                        }
                        ui.label(egui::RichText::new(format!("Oct {}", self.octave)).color(CREAM));
                    });
                });
            });
    }
}

fn header(ui: &mut egui::Ui) {
    let rect = ui.allocate_space(Vec2::new(ui.available_width(), 46.0)).1;
    let p = ui.painter();
    p.rect_filled(rect, Rounding::same(6.0), PANEL);
    p.text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        "HAMMOND B3  ·  gpasquero",
        FontId::proportional(26.0),
        BRASS,
    );
}

impl OrganApp {
    fn control_panel(&mut self, ui: &mut egui::Ui) {
        let rect = ui.allocate_space(Vec2::new(ui.available_width(), 190.0)).1;
        ui.painter().rect_filled(rect, Rounding::same(8.0), PANEL);
        ui.painter()
            .rect_stroke(rect, Rounding::same(8.0), Stroke::new(2.0, WOOD));

        let inner = rect.shrink(12.0);
        let mut changed = false;

        // --- Drawbars ---
        let bar_w = 26.0;
        let gap = 8.0;
        for i in 0..9 {
            let x = inner.left() + i as f32 * (bar_w + gap);
            let col = Rect::from_min_size(Pos2::new(x, inner.top()), Vec2::new(bar_w, 150.0));
            if self.drawbar(ui, col, i) {
                changed = true;
            }
        }

        // --- Switches / knobs to the right ---
        let sw_left = inner.left() + 9.0 * (bar_w + gap) + 16.0;
        let mut y = inner.top();
        let sw = Rect::from_min_size(Pos2::new(sw_left, y), Vec2::new(150.0, 22.0));
        ui.scope_builder(
            egui::UiBuilder::new().max_rect(sw.expand2(Vec2::new(0.0, 170.0))),
            |ui| {
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("PERC").color(CREAM).small());
                    changed |= ui.checkbox(&mut self.patch.percussion.on, "On").changed();
                });
                ui.horizontal(|ui| {
                    changed |= ui
                        .selectable_value(&mut self.patch.percussion.third, false, "2nd")
                        .changed();
                    changed |= ui
                        .selectable_value(&mut self.patch.percussion.third, true, "3rd")
                        .changed();
                    changed |= ui
                        .selectable_value(&mut self.patch.percussion.fast, true, "Fast")
                        .changed();
                    changed |= ui
                        .selectable_value(&mut self.patch.percussion.fast, false, "Slow")
                        .changed();
                });
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("VIB/CHO").color(CREAM).small());
                    egui::ComboBox::from_id_salt("vibrato")
                        .selected_text(self.patch.vibrato.label())
                        .show_ui(ui, |ui| {
                            for m in VibratoMode::ALL {
                                changed |= ui
                                    .selectable_value(&mut self.patch.vibrato, m, m.label())
                                    .changed();
                            }
                        });
                });
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("LESLIE").color(CREAM).small());
                    for (label, sp) in [
                        ("Brake", LeslieSpeed::Brake),
                        ("Slow", LeslieSpeed::Slow),
                        ("Fast", LeslieSpeed::Fast),
                    ] {
                        changed |= ui
                            .selectable_value(&mut self.patch.leslie, sp, label)
                            .changed();
                    }
                });
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("DRIVE").color(CREAM).small());
                    changed |= ui
                        .add(
                            egui::Slider::new(&mut self.patch.overdrive, 0.0..=1.0)
                                .show_value(false),
                        )
                        .changed();
                });
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("VOL").color(CREAM).small());
                    changed |= ui
                        .add(egui::Slider::new(&mut self.patch.volume, 0.0..=1.0).show_value(false))
                        .changed();
                });

                // --- Presets ---
                ui.separator();
                ui.horizontal(|ui| {
                    egui::ComboBox::from_id_salt("preset")
                        .selected_text(
                            self.presets
                                .get(self.selected)
                                .map(|p| p.name.as_str())
                                .unwrap_or("—"),
                        )
                        .show_ui(ui, |ui| {
                            for (i, p) in self.presets.iter().enumerate() {
                                ui.selectable_value(&mut self.selected, i, &p.name);
                            }
                        });
                    if ui.button("Load").clicked() {
                        if let Some(p) = self.presets.get(self.selected) {
                            self.patch = p.patch.clone();
                            changed = true;
                        }
                    }
                });
                ui.horizontal(|ui| {
                    ui.add(egui::TextEdit::singleline(&mut self.preset_name).desired_width(90.0));
                    if ui.button("Save").clicked() {
                        self.presets
                            .push(Preset::new(self.preset_name.clone(), self.patch.clone()));
                        self.selected = self.presets.len() - 1;
                    }
                });
            },
        );
        y += 190.0;
        let _ = y;

        if changed {
            self.sync_patch();
        }
    }

    /// A single vertical drawbar. Returns `true` if its value changed.
    fn drawbar(&mut self, ui: &mut egui::Ui, rect: Rect, index: usize) -> bool {
        let resp = ui.allocate_rect(rect, Sense::click_and_drag());
        let value = self.patch.drawbars[index];
        let mut changed = false;

        if let Some(pos) = resp.interact_pointer_pos() {
            let t = ((rect.bottom() - pos.y) / rect.height()).clamp(0.0, 1.0);
            let new_v = (t * 8.0).round() as u8;
            if new_v != value {
                self.patch.drawbars[index] = new_v;
                changed = true;
            }
        }
        let value = self.patch.drawbars[index];

        let p = ui.painter();
        // Track.
        p.rect_filled(
            rect,
            Rounding::same(3.0),
            Color32::from_rgb(0x0d, 0x0a, 0x08),
        );
        // Pulled-out portion.
        let pull = value as f32 / 8.0;
        let top = rect.bottom() - pull * rect.height();
        let bar = Rect::from_min_max(Pos2::new(rect.left(), top), rect.right_bottom());
        p.rect_filled(bar, Rounding::same(3.0), drawbar_color(index));
        // Number cap.
        p.text(
            Pos2::new(rect.center().x, top - 8.0),
            egui::Align2::CENTER_CENTER,
            format!("{value}"),
            FontId::monospace(12.0),
            CREAM,
        );
        // Footage label at the bottom.
        p.text(
            Pos2::new(rect.center().x, rect.bottom() + 10.0),
            egui::Align2::CENTER_CENTER,
            DRAWBAR_LABELS[index],
            FontId::monospace(10.0),
            BRASS,
        );
        changed
    }

    fn keyboard(&mut self, ui: &mut egui::Ui) {
        let octaves = 2;
        let white_per_oct = 7;
        let n_white = octaves * white_per_oct;
        let rect = ui.allocate_space(Vec2::new(ui.available_width(), 140.0)).1;
        let ww = rect.width() / n_white as f32;
        let base = 48 + self.octave * 12; // C below middle C

        // White-key note offsets within an octave.
        let white_offsets = [0, 2, 4, 5, 7, 9, 11];
        let black_offsets = [1, 3, 6, 8, 10];

        let mut pressed: Option<u8> = None;
        let pointer = ui.input(|i| i.pointer.interact_pos());
        let down = ui.input(|i| i.pointer.primary_down());

        // White keys.
        for w in 0..n_white {
            let oct = w / white_per_oct;
            let idx = w % white_per_oct;
            let note = (base + oct as i32 * 12 + white_offsets[idx]) as u8;
            let kr = Rect::from_min_size(
                Pos2::new(rect.left() + w as f32 * ww, rect.top()),
                Vec2::new(ww - 2.0, rect.height()),
            );
            let held = self.active_keyboard.contains(&note) || self.mouse_note == Some(note);
            let fill = if held { BRASS } else { CREAM };
            ui.painter().rect_filled(kr, Rounding::same(3.0), fill);
            ui.painter()
                .rect_stroke(kr, Rounding::same(3.0), Stroke::new(1.0, KEY_BLACK));
            if down {
                if let Some(p) = pointer {
                    if kr.contains(p) {
                        pressed = Some(note);
                    }
                }
            }
        }

        // Black keys (drawn on top).
        for w in 0..n_white {
            let oct = w / white_per_oct;
            let idx = w % white_per_oct;
            if !black_offsets.contains(&(white_offsets[idx] + 1)) {
                continue;
            }
            let note = (base + oct as i32 * 12 + white_offsets[idx] + 1) as u8;
            let x = rect.left() + (w as f32 + 0.7) * ww;
            let kr = Rect::from_min_size(
                Pos2::new(x, rect.top()),
                Vec2::new(ww * 0.6, rect.height() * 0.62),
            );
            let held = self.active_keyboard.contains(&note) || self.mouse_note == Some(note);
            let fill = if held { BRASS } else { KEY_BLACK };
            ui.painter().rect_filled(kr, Rounding::same(2.0), fill);
            if down {
                if let Some(p) = pointer {
                    if kr.contains(p) {
                        pressed = Some(note);
                    }
                }
            }
        }

        // Mouse edge detection.
        match (self.mouse_note, pressed) {
            (None, Some(n)) => {
                self.mouse_note = Some(n);
                self.note_on(n);
            }
            (Some(old), Some(n)) if old != n => {
                self.note_off(old);
                self.mouse_note = Some(n);
                self.note_on(n);
            }
            (Some(old), None) if !down => {
                self.note_off(old);
                self.mouse_note = None;
            }
            _ => {}
        }
    }
}
