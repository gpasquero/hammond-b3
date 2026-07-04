//! egui front-end for the Hammond B3.
//!
//! The wood-and-metal aesthetic — the oak side cheeks (real photo texture), the
//! amber-on-charcoal palette, the glossy 3D keyboard and the pitch/mod wheels —
//! is adapted from the sibling `rusted-moog` synth so the two instruments share
//! a family look. The controls themselves are pure Hammond: nine drawbars, the
//! percussion and vibrato/chorus sections, and the Leslie speed switch.

use crate::params::{MidiMsg, SharedState};
use crate::preset::{factory_presets, LeslieSpeed, Patch, Preset, VibratoMode};

use eframe::egui;
use egui::{
    pos2, vec2, Align2, Color32, Context, FontId, Pos2, Rect, RichText, Rounding, Sense, Stroke, Ui,
};
use std::collections::HashSet;

// --- Palette (borrowed from rusted-moog) ---
const BG: Color32 = Color32::from_rgb(0x10, 0x11, 0x12);
const WOOD: Color32 = Color32::from_rgb(0x7a, 0x52, 0x30);
const WOOD_HI: Color32 = Color32::from_rgb(0x9a, 0x6a, 0x40);
const WOOD_LO: Color32 = Color32::from_rgb(0x5c, 0x3d, 0x22);
const PANEL_TOP: Color32 = Color32::from_rgb(0x22, 0x26, 0x2a);
const PANEL_BOT: Color32 = Color32::from_rgb(0x17, 0x1a, 0x1e);
const PANEL_BG: Color32 = Color32::from_rgb(0x1e, 0x22, 0x26);
const TROUGH: Color32 = Color32::from_rgb(0x14, 0x16, 0x18);
const CREAM: Color32 = Color32::from_rgb(0xe0, 0xe0, 0xdc);
const CREAM_DIM: Color32 = Color32::from_rgb(0x9a, 0x9c, 0x9a);
const ACCENT: Color32 = Color32::from_rgb(0xe8, 0xa0, 0x25);
const ACCENT_DIM: Color32 = Color32::from_rgb(0xb3, 0x7a, 0x1a);
const WHITE_KEY: Color32 = Color32::from_rgb(0xec, 0xe7, 0xd8);
const BLACK_KEY: Color32 = Color32::from_rgb(0x14, 0x14, 0x14);

/// Classic drawbar colours: brown (16', 5⅓'), white (8', 4', 2') and black.
fn drawbar_color(i: usize) -> (Color32, Color32) {
    match i {
        0 | 1 => (
            Color32::from_rgb(0x8a, 0x50, 0x28),
            Color32::from_rgb(0x54, 0x2c, 0x14),
        ),
        2 | 3 | 5 => (WHITE_KEY, Color32::from_rgb(0xc2, 0xba, 0xa2)),
        _ => (
            Color32::from_rgb(0x34, 0x34, 0x36),
            Color32::from_rgb(0x12, 0x12, 0x13),
        ),
    }
}

const DRAWBAR_LABELS: [&str; 9] = ["16'", "5⅓'", "8'", "4'", "2⅔'", "2'", "1⅗'", "1⅓'", "1'"];

// Computer-keyboard → semitone mapping ("A W S E D F T G Y H U J K O L P").
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
    (egui::Key::O, 13),
    (egui::Key::L, 14),
    (egui::Key::P, 15),
];

// ── Gradient + texture helpers (borrowed from rusted-moog) ──────────────────

fn gradient_rect(
    painter: &egui::Painter,
    rect: Rect,
    tl: Color32,
    tr: Color32,
    br: Color32,
    bl: Color32,
) {
    let mut mesh = egui::Mesh::default();
    mesh.colored_vertex(rect.left_top(), tl);
    mesh.colored_vertex(rect.right_top(), tr);
    mesh.colored_vertex(rect.right_bottom(), br);
    mesh.colored_vertex(rect.left_bottom(), bl);
    mesh.add_triangle(0, 1, 2);
    mesh.add_triangle(0, 2, 3);
    painter.add(mesh);
}

fn vgrad(painter: &egui::Painter, rect: Rect, top: Color32, bot: Color32) {
    gradient_rect(painter, rect, top, top, bot, bot);
}

fn hgrad(painter: &egui::Painter, rect: Rect, left: Color32, right: Color32) {
    gradient_rect(painter, rect, left, right, right, left);
}

/// Paint an oak-wood side cheek from the real photo texture (procedural fallback).
fn paint_wood(painter: &egui::Painter, rect: Rect, tex: Option<&egui::TextureHandle>) {
    if let Some(tex) = tex {
        let uv = Rect::from_min_max(pos2(0.36, 0.02), pos2(0.60, 0.98));
        painter.image(tex.id(), rect, uv, Color32::WHITE);
        painter.rect_filled(rect, 0.0, Color32::from_rgba_unmultiplied(18, 10, 3, 46));
    } else {
        painter.rect_filled(rect, 0.0, WOOD);
        let mid = rect.center().x;
        let lh = Rect::from_min_max(rect.left_top(), pos2(mid, rect.bottom()));
        let rh = Rect::from_min_max(pos2(mid, rect.top()), rect.right_bottom());
        hgrad(painter, lh, WOOD_LO, WOOD_HI);
        hgrad(painter, rh, WOOD_HI, WOOD_LO);
    }
    painter.line_segment(
        [rect.left_top(), rect.left_bottom()],
        Stroke::new(2.0, Color32::from_rgba_unmultiplied(0, 0, 0, 90)),
    );
    painter.line_segment(
        [rect.right_top(), rect.right_bottom()],
        Stroke::new(2.0, Color32::from_rgba_unmultiplied(0, 0, 0, 90)),
    );
}

/// Decode the embedded oak-wood photo into a GPU texture (once).
fn load_wood(ctx: &Context) -> Option<egui::TextureHandle> {
    let bytes = include_bytes!("../assets/wood.jpg");
    let img = image::load_from_memory(bytes).ok()?.to_rgba8();
    let (w, h) = img.dimensions();
    let color = egui::ColorImage::from_rgba_unmultiplied([w as usize, h as usize], img.as_raw());
    Some(ctx.load_texture("oak-wood", color, egui::TextureOptions::LINEAR))
}

fn note_at(p: Pos2, blacks: &[(Rect, i32)], whites: &[(Rect, i32)]) -> Option<i32> {
    for (r, n) in blacks {
        if r.contains(p) {
            return Some(*n);
        }
    }
    for (r, n) in whites {
        if r.contains(p) {
            return Some(*n);
        }
    }
    None
}

// ── Application ─────────────────────────────────────────────────────────────

pub struct OrganApp {
    shared: SharedState,
    patch: Patch,
    presets: Vec<Preset>,
    selected: usize,
    preset_name: String,
    kbd_octave: i32,
    audio_started: bool,
    status: String,
    active_notes: HashSet<i32>,
    mouse_note: Option<i32>,
    pitch_wheel: f32,
    mod_wheel: f32,
    wood_tex: Option<egui::TextureHandle>,

    #[cfg(feature = "audio")]
    _audio: Option<crate::audio::AudioHandle>,
    #[cfg(all(feature = "desktop", not(target_arch = "wasm32")))]
    _midi: Option<crate::midi::MidiHandle>,
}

impl OrganApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        Self::install_theme(&cc.egui_ctx);
        let wood_tex = load_wood(&cc.egui_ctx);
        let presets = factory_presets();
        let patch = presets.first().map(|p| p.patch.clone()).unwrap_or_default();
        let shared = SharedState::new(patch.clone());

        Self {
            shared,
            patch,
            presets,
            selected: 0,
            preset_name: "My Preset".to_string(),
            kbd_octave: 0,
            audio_started: false,
            status: "Click a key or press A–P to start the sound".to_string(),
            active_notes: HashSet::new(),
            mouse_note: None,
            pitch_wheel: 0.5,
            mod_wheel: 0.0,
            wood_tex,
            #[cfg(feature = "audio")]
            _audio: None,
            #[cfg(all(feature = "desktop", not(target_arch = "wasm32")))]
            _midi: None,
        }
    }

    fn install_theme(ctx: &Context) {
        let mut v = egui::Visuals::dark();
        v.override_text_color = Some(CREAM);
        v.panel_fill = BG;
        v.window_fill = BG;
        v.faint_bg_color = PANEL_BG;
        v.extreme_bg_color = TROUGH;
        v.widgets.noninteractive.bg_fill = PANEL_BG;
        v.widgets.inactive.bg_fill = Color32::from_rgb(0x33, 0x36, 0x38);
        v.widgets.inactive.weak_bg_fill = Color32::from_rgb(0x33, 0x36, 0x38);
        v.widgets.hovered.bg_fill = Color32::from_rgb(0x4a, 0x4e, 0x50);
        v.widgets.active.bg_fill = ACCENT_DIM;
        v.selection.bg_fill = ACCENT_DIM;
        v.selection.stroke = Stroke::new(1.0, ACCENT);
        for w in [
            &mut v.widgets.noninteractive,
            &mut v.widgets.inactive,
            &mut v.widgets.hovered,
            &mut v.widgets.active,
            &mut v.widgets.open,
        ] {
            w.rounding = Rounding::same(4.0);
        }
        ctx.set_visuals(v);
        let mut style = (*ctx.style()).clone();
        style.spacing.item_spacing = vec2(6.0, 6.0);
        style.spacing.button_padding = vec2(8.0, 4.0);
        ctx.set_style(style);
    }

    #[cfg(feature = "audio")]
    pub fn with_audio(mut self, handle: crate::audio::AudioHandle) -> Self {
        self.audio_started = true;
        self.status = format!("Audio @ {} Hz — ready", handle.sample_rate as u32);
        self._audio = Some(handle);
        self
    }

    #[cfg(all(feature = "desktop", not(target_arch = "wasm32")))]
    pub fn with_midi(mut self, handle: Option<crate::midi::MidiHandle>) -> Self {
        if let Some(h) = &handle {
            self.status = format!("{}  ·  MIDI: {}", self.status, h.port_name);
        }
        self._midi = handle;
        self
    }

    pub fn shared(&self) -> SharedState {
        self.shared.clone()
    }

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

    /// Browsers start the `AudioContext` suspended and only let it resume from a
    /// user gesture. So on every pointer/key press we (lazily) create the stream
    /// and re-`play()` it until sound actually flows. Harmless on native.
    fn kick_audio(&mut self, ctx: &Context) {
        let interacted = ctx.input(|i| {
            i.pointer.any_pressed()
                || i.events
                    .iter()
                    .any(|e| matches!(e, egui::Event::Key { pressed: true, .. }))
        });
        if !interacted {
            return;
        }
        self.ensure_audio();
        #[cfg(feature = "audio")]
        if let Some(handle) = &self._audio {
            handle.resume();
        }
    }

    fn note_on(&mut self, note: i32) {
        self.ensure_audio();
        if (0..=127).contains(&note) {
            self.active_notes.insert(note);
            self.shared.push_event(MidiMsg::NoteOn {
                note: note as u8,
                velocity: 100,
            });
        }
    }

    fn note_off(&mut self, note: i32) {
        self.active_notes.remove(&note);
        if (0..=127).contains(&note) {
            self.shared
                .push_event(MidiMsg::NoteOff { note: note as u8 });
        }
    }

    fn process_keyboard(&mut self, ctx: &Context) {
        let base = 60 + self.kbd_octave * 12;
        let mut desired: HashSet<i32> = HashSet::new();
        ctx.input(|i| {
            for (key, semis) in KEY_MAP {
                if i.key_down(*key) {
                    desired.insert(base + *semis);
                }
            }
        });
        // Only diff computer-keyboard notes; mouse note is tracked separately.
        let pc_current: HashSet<i32> = self
            .active_notes
            .iter()
            .copied()
            .filter(|n| self.mouse_note != Some(*n))
            .collect();
        for n in desired.difference(&pc_current) {
            if self.mouse_note != Some(*n) {
                self.note_on(*n);
            }
        }
        for n in pc_current.difference(&desired) {
            if self.mouse_note != Some(*n) {
                self.note_off(*n);
            }
        }
    }
}

impl eframe::App for OrganApp {
    fn update(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        // Cap repaint at ~30 fps: keeps the web audio callback (main thread) smooth.
        ctx.request_repaint_after(std::time::Duration::from_millis(33));
        self.kick_audio(ctx);
        self.process_keyboard(ctx);

        egui::TopBottomPanel::top("header")
            .frame(egui::Frame::none().fill(PANEL_TOP).inner_margin(10.0))
            .show(ctx, |ui| self.header(ui));

        egui::TopBottomPanel::bottom("keyboard")
            .frame(
                egui::Frame::none()
                    .fill(Color32::from_rgb(0x0e, 0x0e, 0x0f))
                    .inner_margin(10.0),
            )
            .show(ctx, |ui| self.keyboard(ui));

        let wood = self.wood_tex.clone();
        egui::SidePanel::left("wood_left")
            .exact_width(26.0)
            .resizable(false)
            .frame(egui::Frame::none())
            .show(ctx, |ui| {
                paint_wood(ui.painter(), ui.max_rect(), wood.as_ref())
            });
        egui::SidePanel::right("wood_right")
            .exact_width(26.0)
            .resizable(false)
            .frame(egui::Frame::none())
            .show(ctx, |ui| {
                paint_wood(ui.painter(), ui.max_rect(), wood.as_ref())
            });

        egui::CentralPanel::default()
            .frame(egui::Frame::none().fill(PANEL_BOT).inner_margin(10.0))
            .show(ctx, |ui| {
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| self.body(ui));
            });
    }
}

impl OrganApp {
    fn header(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            ui.label(
                RichText::new("HAMMOND B3")
                    .color(ACCENT)
                    .size(26.0)
                    .strong(),
            );
            ui.add_space(8.0);
            ui.label(
                RichText::new("tonewheel organ · gpasquero")
                    .color(CREAM_DIM)
                    .size(12.0),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(RichText::new(&self.status).color(CREAM_DIM).size(12.0));
            });
        });
    }

    fn body(&mut self, ui: &mut Ui) {
        let mut changed = false;
        ui.add_space(2.0);
        ui.horizontal_top(|ui| {
            // --- Drawbars ---
            panel(ui, "DRAWBARS", |ui| {
                ui.horizontal(|ui| {
                    for i in 0..9 {
                        changed |= self.drawbar(ui, i);
                    }
                });
            });

            // --- Percussion + Vibrato ---
            panel(ui, "PERCUSSION · VIBRATO", |ui| {
                changed |= ui
                    .checkbox(&mut self.patch.percussion.on, "Percussion On")
                    .changed();
                ui.horizontal(|ui| {
                    changed |= tab(ui, &mut self.patch.percussion.third, false, "2nd");
                    changed |= tab(ui, &mut self.patch.percussion.third, true, "3rd");
                    changed |= tab(ui, &mut self.patch.percussion.soft, false, "Norm");
                    changed |= tab(ui, &mut self.patch.percussion.soft, true, "Soft");
                });
                ui.horizontal(|ui| {
                    changed |= tab(ui, &mut self.patch.percussion.fast, true, "Fast Decay");
                    changed |= tab(ui, &mut self.patch.percussion.fast, false, "Slow Decay");
                });
                ui.add_space(8.0);
                ui.label(
                    RichText::new("VIBRATO / CHORUS")
                        .color(ACCENT)
                        .size(11.0)
                        .strong(),
                );
                ui.horizontal(|ui| {
                    for m in VibratoMode::ALL {
                        changed |= tab(ui, &mut self.patch.vibrato, m, m.label());
                    }
                });
            });
        });

        ui.add_space(8.0);
        ui.horizontal_top(|ui| {
            // --- Leslie ---
            panel(ui, "LESLIE", |ui| {
                ui.horizontal(|ui| {
                    changed |= tab(ui, &mut self.patch.leslie, LeslieSpeed::Brake, "Brake");
                    changed |= tab(ui, &mut self.patch.leslie, LeslieSpeed::Slow, "Slow");
                    changed |= tab(ui, &mut self.patch.leslie, LeslieSpeed::Fast, "Fast");
                });
                ui.add_space(4.0);
                let (rect, _) = ui.allocate_exact_size(vec2(170.0, 34.0), Sense::hover());
                self.leslie_indicator(ui, rect);
            });

            // --- Amp ---
            panel(ui, "AMPLIFIER", |ui| {
                ui.label(RichText::new("DRIVE").color(CREAM_DIM).size(11.0));
                changed |= ui
                    .add(egui::Slider::new(&mut self.patch.overdrive, 0.0..=1.0).show_value(false))
                    .changed();
                ui.label(RichText::new("VOLUME").color(CREAM_DIM).size(11.0));
                changed |= ui
                    .add(egui::Slider::new(&mut self.patch.volume, 0.0..=1.0).show_value(false))
                    .changed();
            });

            // --- Presets ---
            panel(ui, "PRESETS", |ui| {
                ui.horizontal(|ui| {
                    egui::ComboBox::from_id_salt("preset")
                        .selected_text(
                            self.presets
                                .get(self.selected)
                                .map(|p| p.name.as_str())
                                .unwrap_or("—"),
                        )
                        .width(140.0)
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
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    ui.add(egui::TextEdit::singleline(&mut self.preset_name).desired_width(140.0));
                    if ui.button("Save").clicked() {
                        self.presets
                            .push(Preset::new(self.preset_name.clone(), self.patch.clone()));
                        self.selected = self.presets.len() - 1;
                    }
                });
            });
        });

        if changed {
            self.sync_patch();
        }
    }

    fn leslie_indicator(&self, ui: &Ui, rect: Rect) {
        let p = ui.painter();
        p.rect_filled(
            rect,
            Rounding::same(5.0),
            Color32::from_rgb(0x0a, 0x0b, 0x0c),
        );
        let (label, col) = match self.patch.leslie {
            LeslieSpeed::Brake => ("BRAKE", Color32::from_rgb(0x88, 0x36, 0x30)),
            LeslieSpeed::Slow => ("CHORALE — slow", Color32::from_rgb(0x3a, 0x8a, 0x50)),
            LeslieSpeed::Fast => ("TREMOLO — fast", ACCENT),
        };
        p.circle_filled(pos2(rect.left() + 16.0, rect.center().y), 6.0, col);
        p.text(
            pos2(rect.left() + 32.0, rect.center().y),
            Align2::LEFT_CENTER,
            label,
            FontId::monospace(13.0),
            col,
        );
    }

    /// A single vertical drawbar. Returns `true` if its value changed.
    fn drawbar(&mut self, ui: &mut Ui, index: usize) -> bool {
        let (rect, resp) = ui.allocate_exact_size(vec2(30.0, 172.0), Sense::click_and_drag());
        let track = Rect::from_min_max(
            pos2(rect.left() + 4.0, rect.top() + 4.0),
            pos2(rect.right() - 4.0, rect.bottom() - 16.0),
        );
        if let Some(pos) = resp.interact_pointer_pos() {
            let t = ((track.bottom() - pos.y) / track.height()).clamp(0.0, 1.0);
            self.patch.drawbars[index] = (t * 8.0).round() as u8;
        }
        let value = self.patch.drawbars[index];
        let changed = resp.dragged() || resp.clicked();

        let p = ui.painter();
        p.rect_filled(
            track,
            Rounding::same(3.0),
            Color32::from_rgb(0x0a, 0x08, 0x06),
        );
        let pull = value as f32 / 8.0;
        let cap_h = 20.0;
        let cap_y = track.bottom() - cap_h - pull * (track.height() - cap_h);
        let bar = Rect::from_min_max(
            pos2(track.left(), cap_y),
            pos2(track.right(), track.bottom()),
        );
        let (c_top, c_bot) = drawbar_color(index);
        vgrad(p, bar, c_top, c_bot);
        p.rect_stroke(
            bar,
            Rounding::same(2.0),
            Stroke::new(1.0, Color32::from_black_alpha(120)),
        );
        // Gloss line on the cap.
        p.line_segment(
            [
                pos2(bar.left() + 2.0, cap_y + 2.0),
                pos2(bar.right() - 2.0, cap_y + 2.0),
            ],
            Stroke::new(1.0, Color32::from_white_alpha(60)),
        );
        let num_col = if index >= 6 {
            CREAM
        } else {
            Color32::from_rgb(0x1a, 0x10, 0x08)
        };
        p.text(
            pos2(bar.center().x, cap_y + cap_h * 0.5),
            Align2::CENTER_CENTER,
            format!("{value}"),
            FontId::monospace(12.0),
            num_col,
        );
        p.text(
            pos2(rect.center().x, rect.bottom() - 7.0),
            Align2::CENTER_CENTER,
            DRAWBAR_LABELS[index],
            FontId::monospace(10.0),
            ACCENT,
        );
        changed
    }

    // ── Keyboard (adapted from rusted-moog) ──────────────────────────────────

    fn keyboard(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            if ui.button("OCT −").clicked() {
                self.kbd_octave = (self.kbd_octave - 1).max(-3);
            }
            ui.label(
                RichText::new(format!("OCTAVE {}", self.kbd_octave + 4))
                    .color(ACCENT)
                    .strong(),
            );
            if ui.button("OCT +").clicked() {
                self.kbd_octave = (self.kbd_octave + 1).min(3);
            }
            ui.add_space(10.0);
            ui.label(
                RichText::new("keys:  A W S E D F T G Y H U J K O L P")
                    .color(CREAM_DIM)
                    .size(9.0),
            );
        });
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            self.wheels(ui);
            ui.add_space(10.0);
            self.draw_keys(ui);
        });
    }

    fn wheels(&mut self, ui: &mut Ui) {
        egui::Frame::none()
            .fill(Color32::from_rgb(0x0b, 0x0b, 0x0c))
            .rounding(4.0)
            .inner_margin(6.0)
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    let mut p = self.pitch_wheel;
                    let dragging = draw_wheel(ui, "PITCH", &mut p);
                    self.pitch_wheel = if dragging {
                        p
                    } else {
                        self.pitch_wheel + (0.5 - self.pitch_wheel) * 0.25
                    };
                    ui.add_space(2.0);
                    let mut m = self.mod_wheel;
                    if draw_wheel(ui, "MOD", &mut m) {
                        self.mod_wheel = m;
                    }
                });
            });
    }

    fn draw_keys(&mut self, ui: &mut Ui) {
        let avail_w = ui.available_width();
        let (resp, painter) = ui.allocate_painter(vec2(avail_w, 130.0), Sense::click_and_drag());
        let rect = resp.rect;

        let num_oct = 3;
        let whites = [0, 2, 4, 5, 7, 9, 11];
        let total_white = num_oct * 7 + 1;
        let wk = rect.width() / total_white as f32;
        let bk_w = wk * 0.6;
        let bk_h = rect.height() * 0.62;
        let base = 48 + self.kbd_octave * 12;

        let mut white_notes: Vec<(Rect, i32)> = Vec::new();
        let mut wi = 0;
        for o in 0..num_oct {
            for &semi in &whites {
                let note = base + o * 12 + semi;
                let x1 = rect.left() + wi as f32 * wk;
                white_notes.push((
                    Rect::from_min_max(pos2(x1, rect.top()), pos2(x1 + wk, rect.bottom())),
                    note,
                ));
                wi += 1;
            }
        }
        let x1 = rect.left() + wi as f32 * wk;
        white_notes.push((
            Rect::from_min_max(pos2(x1, rect.top()), pos2(x1 + wk, rect.bottom())),
            base + num_oct * 12,
        ));

        let blacks = [(0usize, 1i32), (1, 3), (3, 6), (4, 8), (5, 10)];
        let mut black_notes: Vec<(Rect, i32)> = Vec::new();
        for o in 0..num_oct {
            for &(wn, semi) in &blacks {
                let note = base + o * 12 + semi;
                let cx = rect.left() + ((o as usize * 7 + wn + 1) as f32) * wk;
                black_notes.push((
                    Rect::from_min_max(
                        pos2(cx - bk_w / 2.0, rect.top()),
                        pos2(cx + bk_w / 2.0, rect.top() + bk_h),
                    ),
                    note,
                ));
            }
        }

        let names = ["C", "", "D", "", "E", "F", "", "G", "", "A", "", "B"];
        for (r, note) in &white_notes {
            let on = self.active_notes.contains(note);
            let body = if on { ACCENT } else { WHITE_KEY };
            painter.rect(
                *r,
                3.0,
                body,
                Stroke::new(1.0, Color32::from_rgb(0x9a, 0x92, 0x7c)),
            );
            let shadow = Rect::from_min_max(pos2(r.left(), r.bottom() - 14.0), r.max);
            vgrad(
                &painter,
                shadow,
                Color32::from_rgba_unmultiplied(0, 0, 0, 0),
                Color32::from_rgba_unmultiplied(0, 0, 0, if on { 30 } else { 55 }),
            );
            painter.line_segment(
                [
                    pos2(r.left() + 2.0, r.top() + 1.5),
                    pos2(r.right() - 2.0, r.top() + 1.5),
                ],
                Stroke::new(1.0, Color32::from_rgba_unmultiplied(0xff, 0xff, 0xff, 90)),
            );
            let semi = note.rem_euclid(12) as usize;
            let label = if semi == 0 {
                format!("C{}", note / 12 - 1)
            } else {
                names[semi].to_string()
            };
            painter.text(
                pos2(r.center().x, r.bottom() - 10.0),
                Align2::CENTER_CENTER,
                label,
                FontId::proportional(8.0),
                if on {
                    Color32::from_rgb(0x40, 0x30, 0x10)
                } else {
                    CREAM_DIM
                },
            );
        }
        for (r, note) in &black_notes {
            let on = self.active_notes.contains(note);
            let body = if on { ACCENT_DIM } else { BLACK_KEY };
            painter.rect(*r, 2.0, body, Stroke::new(1.0, Color32::BLACK));
            let gloss = Rect::from_min_max(
                pos2(r.left() + 1.5, r.top() + 1.5),
                pos2(r.right() - 1.5, r.top() + r.height() * 0.55),
            );
            vgrad(
                &painter,
                gloss,
                Color32::from_rgba_unmultiplied(0xff, 0xff, 0xff, if on { 40 } else { 55 }),
                Color32::from_rgba_unmultiplied(0xff, 0xff, 0xff, 0),
            );
        }

        if resp.is_pointer_button_down_on() {
            if let Some(pt) = resp.interact_pointer_pos() {
                let note = note_at(pt, &black_notes, &white_notes);
                if note != self.mouse_note {
                    if let Some(old) = self.mouse_note {
                        self.note_off(old);
                    }
                    if let Some(n) = note {
                        self.note_on(n);
                    }
                    self.mouse_note = note;
                }
            }
        } else if let Some(old) = self.mouse_note.take() {
            self.note_off(old);
        }
    }
}

// ── Free-standing widgets ────────────────────────────────────────────────────

/// A framed metal sub-panel with a title (mirrors rusted-moog's `panel`).
fn panel(ui: &mut Ui, title: &str, add: impl FnOnce(&mut Ui)) {
    egui::Frame::none()
        .fill(PANEL_BG)
        .rounding(6.0)
        .stroke(Stroke::new(1.0, Color32::from_rgb(0x3a, 0x40, 0x46)))
        .inner_margin(10.0)
        .show(ui, |ui| {
            let r = ui.max_rect();
            vgrad(
                ui.painter(),
                Rect::from_min_max(r.min, pos2(r.max.x, r.min.y + 24.0)),
                PANEL_TOP,
                PANEL_BG,
            );
            ui.label(RichText::new(title).color(ACCENT).size(11.0).strong());
            ui.add_space(4.0);
            add(ui);
        });
}

/// A tab-style toggle bound to `*field == value`.
fn tab<T: PartialEq + Copy>(ui: &mut Ui, field: &mut T, value: T, label: &str) -> bool {
    if ui.selectable_label(*field == value, label).clicked() {
        *field = value;
        return true;
    }
    false
}

/// A pitch/mod wheel: a dark slot with a draggable brass thumb. Returns `true`
/// while it is being dragged. `value` is `0.0..=1.0`.
fn draw_wheel(ui: &mut Ui, label: &str, value: &mut f32) -> bool {
    let (resp, painter) = ui.allocate_painter(vec2(26.0, 116.0), Sense::click_and_drag());
    let rect = resp.rect;
    let slot = Rect::from_center_size(rect.center(), vec2(16.0, rect.height() - 20.0));
    vgrad(
        &painter,
        slot,
        Color32::from_rgb(0x28, 0x22, 0x1c),
        Color32::from_rgb(0x0e, 0x0c, 0x0a),
    );
    painter.rect_stroke(
        slot,
        Rounding::same(6.0),
        Stroke::new(1.0, Color32::from_black_alpha(160)),
    );

    if let Some(pos) = resp.interact_pointer_pos() {
        *value = ((slot.bottom() - pos.y) / slot.height()).clamp(0.0, 1.0);
    }
    let ty = slot.bottom() - *value * slot.height();
    let thumb = Rect::from_center_size(pos2(slot.center().x, ty), vec2(20.0, 12.0));
    vgrad(&painter, thumb, ACCENT, ACCENT_DIM);
    painter.rect_stroke(
        thumb,
        Rounding::same(3.0),
        Stroke::new(1.0, Color32::from_black_alpha(120)),
    );
    painter.text(
        pos2(rect.center().x, rect.bottom() - 4.0),
        Align2::CENTER_CENTER,
        label,
        FontId::monospace(8.0),
        CREAM_DIM,
    );
    resp.dragged()
}
