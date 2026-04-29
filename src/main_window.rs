use std::collections::HashMap;

use chrono::{DateTime, Local};
use eframe::egui;

use crate::config::{Config, ConfigPatch, PopupPosition};
use crate::parser::Reminder;

pub const WIDTH: f32 = 980.0;
pub const HEIGHT: f32 = 620.0;

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
) -> Vec<MainWindowAction> {
    let mut actions = Vec::new();

    egui::Frame::default()
        .inner_margin(egui::Margin::symmetric(20, 16))
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing = egui::vec2(10.0, 10.0);
            ui.spacing_mut().button_padding = egui::vec2(10.0, 6.0);

            // Explicit heading colour — egui's default heading color
            // resolves to `widgets.noninteractive.fg_stroke.color`, which
            // is `ui.normal` (#dbdbdb) and reads as muted on the dark
            // chrome. Use `ui.important_global` (#f3effb) for headings.
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new("kree")
                        .size(30.0)
                        .strong()
                        .color(egui::Color32::from_rgb(0xf3, 0xef, 0xfb)),
                );
                ui.add_space(8.0);
                ui.label(
                    egui::RichText::new("⟁")
                        .size(22.0)
                        .color(egui::Color32::from_rgb(0xd8, 0x6d, 0xd8)),
                );
            });
            ui.label(
                egui::RichText::new("Jaffa, kree! Cron-scheduled reminders.")
                    .size(13.0)
                    .italics()
                    .color(egui::Color32::from_rgb(0xc9, 0xb6, 0xeb)),
            );
            ui.label(
                egui::RichText::new("\"Loosely translated: attention, listen up.\"  — D. Jackson")
                    .size(11.0)
                    .color(egui::Color32::from_rgb(0x9d, 0x9d, 0x9d)),
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

            // Settings panel — bidirectional with config.toml. Edits
            // here are persisted via `ConfigPatch` actions; manual file
            // edits hot-reload through the watcher.
            ui.label(
                egui::RichText::new("Settings")
                    .strong()
                    .size(13.0)
                    .color(egui::Color32::from_rgb(0xc9, 0xb6, 0xeb)),
            );
            ui.add_space(4.0);
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing.x = 16.0;

                let mut chime = config.chime;
                if ui.checkbox(&mut chime, "Chime").changed() {
                    actions.push(MainWindowAction::SetConfig(ConfigPatch::Chime(chime)));
                }

                let mut speak = config.speak;
                if ui.checkbox(&mut speak, "Speak").changed() {
                    actions.push(MainWindowAction::SetConfig(ConfigPatch::Speak(speak)));
                }

                ui.label("Popup:");
                let mut chosen: Option<PopupPosition> = None;
                egui::ComboBox::from_id_salt("popup_position")
                    .selected_text(config.popup_position.as_str())
                    .show_ui(ui, |ui| {
                        for &pos in PopupPosition::ALL {
                            if ui
                                .selectable_label(pos == config.popup_position, pos.as_str())
                                .clicked()
                            {
                                chosen = Some(pos);
                            }
                        }
                    });
                if let Some(p) = chosen {
                    actions.push(MainWindowAction::SetConfig(ConfigPatch::PopupPosition(p)));
                }
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

            egui::ScrollArea::vertical().show(ui, |ui| {
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

                        for reminder in reminders {
                            ui.label(egui::RichText::new(&reminder.schedule).monospace());
                            ui.label(egui::RichText::new(&reminder.icon).size(22.0));
                            ui.label(&reminder.body);

                            let next = reminder
                                .cron
                                .find_next_occurrence(&now, false)
                                .map(|t| t.format("%Y-%m-%d %H:%M").to_string())
                                .unwrap_or_else(|_| "—".into());
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
