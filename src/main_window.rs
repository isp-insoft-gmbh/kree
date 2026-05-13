use std::collections::HashMap;

use chrono::{DateTime, Local};
use eframe::egui;

use crate::config::{Config, ConfigPatch, PopupPosition, ThemeChoice};
use crate::parser::Reminder;
use crate::theme::{ThemeMode, heading_color, muted_color, subtitle_color};

pub const WIDTH: f32 = 1180.0;
pub const HEIGHT: f32 = 720.0;

#[derive(Default)]
pub struct MainWindowState {
    pub paused: bool,
    pub autostart: bool,
}

/// Render the main window into the given Ui. Buttons emit
/// [`MainWindowAction`]s; the caller wires those to the rest of the app
/// (editor launch in step 12, hot reload in step 13, autostart in step
/// 14). Step 11 just gives them a UI surface.
pub fn render(
    ui: &mut egui::Ui,
    state: &mut MainWindowState,
    reminders: &[Reminder],
    config: &Config,
    last_fired: &HashMap<String, DateTime<Local>>,
    mode: ThemeMode,
) -> Vec<MainWindowAction> {
    let mut actions = Vec::new();

    // Explicit `fill` here so the main window's background tracks
    // whichever palette is active. Without it, the outer Frame is
    // transparent over eframe's own clear, which can lag the theme
    // by a frame on toggle.
    let panel_fill = ui.visuals().panel_fill;
    egui::Frame::default()
        .fill(panel_fill)
        .inner_margin(egui::Margin::symmetric(20, 16))
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing = egui::vec2(10.0, 10.0);
            ui.spacing_mut().button_padding = egui::vec2(10.0, 6.0);

            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new("kree")
                        .size(30.0)
                        .strong()
                        .color(heading_color(mode)),
                );
                ui.add_space(8.0);
                ui.label(
                    egui::RichText::new("⟁")
                        .size(22.0)
                        .color(subtitle_color(mode)),
                );
            });
            ui.label(
                egui::RichText::new("Jaffa, kree! Cron-scheduled reminders.")
                    .size(13.0)
                    .italics()
                    .color(subtitle_color(mode)),
            );
            ui.label(
                egui::RichText::new("\"Loosely translated: attention, listen up.\"  — D. Jackson")
                    .size(11.0)
                    .color(muted_color(mode)),
            );
            ui.add_space(14.0);

            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing.x = 8.0;
                if ui.button("Edit Reminders").clicked() {
                    actions.push(MainWindowAction::EditReminders);
                }
                if ui.button("Reload").clicked() {
                    actions.push(MainWindowAction::Reload);
                }
                let pause_label = if state.paused { "Resume" } else { "Pause All" };
                if ui.button(pause_label).clicked() {
                    state.paused = !state.paused;
                    actions.push(MainWindowAction::SetPaused(state.paused));
                }
                ui.add_space(8.0);
                if ui
                    .checkbox(&mut state.autostart, "Start with Windows")
                    .changed()
                {
                    actions.push(MainWindowAction::SetAutostart(state.autostart));
                }
            });

            ui.add_space(14.0);
            ui.separator();
            ui.add_space(10.0);

            ui.label(
                egui::RichText::new("Settings")
                    .strong()
                    .size(13.0)
                    .color(subtitle_color(mode)),
            );
            ui.add_space(6.0);

            egui::Grid::new("settings-grid")
                .num_columns(2)
                .spacing([16.0, 8.0])
                .min_col_width(80.0)
                .show(ui, |ui| {
                    let mut chime = config.chime;
                    ui.label("Chime");
                    if ui.checkbox(&mut chime, "play on fire").changed() {
                        actions.push(MainWindowAction::SetConfig(ConfigPatch::Chime(chime)));
                    }
                    ui.end_row();

                    let mut speak = config.speak;
                    ui.label("Speak");
                    if ui.checkbox(&mut speak, "TTS body 2s after fire").changed() {
                        actions.push(MainWindowAction::SetConfig(ConfigPatch::Speak(speak)));
                    }
                    ui.end_row();

                    ui.label("Popup");
                    let mut chosen_pos: Option<PopupPosition> = None;
                    egui::ComboBox::from_id_salt("popup_position")
                        .selected_text(config.popup_position.as_str())
                        .show_ui(ui, |ui| {
                            for &pos in PopupPosition::ALL {
                                if ui
                                    .selectable_label(pos == config.popup_position, pos.as_str())
                                    .clicked()
                                {
                                    chosen_pos = Some(pos);
                                }
                            }
                        });
                    if let Some(p) = chosen_pos {
                        actions.push(MainWindowAction::SetConfig(ConfigPatch::PopupPosition(p)));
                    }
                    ui.end_row();

                    ui.label("Theme");
                    let mut chosen_theme: Option<ThemeChoice> = None;
                    egui::ComboBox::from_id_salt("theme")
                        .selected_text(config.theme.as_str())
                        .show_ui(ui, |ui| {
                            for &theme in ThemeChoice::ALL {
                                if ui
                                    .selectable_label(theme == config.theme, theme.as_str())
                                    .clicked()
                                {
                                    chosen_theme = Some(theme);
                                }
                            }
                        });
                    if let Some(t) = chosen_theme {
                        actions.push(MainWindowAction::SetConfig(ConfigPatch::Theme(t)));
                    }
                    ui.end_row();
                });

            ui.add_space(14.0);
            ui.separator();
            ui.add_space(14.0);

            if reminders.is_empty() {
                ui.label(egui::RichText::new("No reminders loaded.").italics().weak());
                ui.add_space(4.0);
                ui.label("Click Edit Reminders to add some.");
                return;
            }

            let now = Local::now();
            let mut sorted_reminders: Vec<_> = reminders
                .iter()
                .map(|reminder| {
                    let next = reminder
                        .cron
                        .find_next_occurrence(&now, false)
                        .expect("parsed reminder cron should have a next occurrence");
                    (reminder, next)
                })
                .collect();
            sorted_reminders.sort_by(|(a_reminder, a_next), (b_reminder, b_next)| {
                a_next
                    .cmp(b_next)
                    .then_with(|| a_reminder.schedule.cmp(&b_reminder.schedule))
                    .then_with(|| a_reminder.body.cmp(&b_reminder.body))
            });

            egui::ScrollArea::both()
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    egui::Grid::new("reminders-grid")
                        .num_columns(5)
                        .striped(true)
                        .spacing([22.0, 10.0])
                        .min_col_width(80.0)
                        .show(ui, |ui| {
                            ui.label(egui::RichText::new("Schedule").strong());
                            ui.label(egui::RichText::new("Icon").strong());
                            ui.label(egui::RichText::new("Message").strong());
                            ui.label(egui::RichText::new("Next fire").strong());
                            ui.label(egui::RichText::new("Last fired").strong());
                            ui.end_row();

                            for (reminder, next) in sorted_reminders {
                                ui.label(egui::RichText::new(&reminder.schedule).monospace());
                                ui.label(egui::RichText::new(&reminder.icon).size(22.0));
                                ui.label(&reminder.body);

                                let next = next.format("%Y-%m-%d %H:%M").to_string();
                                ui.label(egui::RichText::new(next).monospace());

                                let last = match last_fired.get(&reminder.schedule) {
                                    Some(t) => t.format("%Y-%m-%d %H:%M:%S").to_string(),
                                    None => "—".into(),
                                };
                                ui.label(egui::RichText::new(last).monospace());
                                ui.end_row();
                            }
                        });
                });
        });

    actions
}

#[derive(Debug)]
pub enum MainWindowAction {
    EditReminders,
    Reload,
    SetPaused(bool),
    SetAutostart(bool),
    SetConfig(ConfigPatch),
}
