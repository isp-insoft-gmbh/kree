use std::collections::HashMap;

use chrono::{DateTime, Local};
use eframe::egui;

use crate::parser::Reminder;

pub const WIDTH: f32 = 720.0;
pub const HEIGHT: f32 = 440.0;

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
    last_fired: &HashMap<String, DateTime<Local>>,
) -> Vec<MainWindowAction> {
    let mut actions = Vec::new();

    ui.add_space(8.0);
    ui.heading("kree");
    ui.label(egui::RichText::new("Cron-scheduled reminders.").weak());
    ui.add_space(8.0);

    ui.horizontal(|ui| {
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
        if ui
            .checkbox(&mut state.autostart, "Start with Windows")
            .changed()
        {
            actions.push(MainWindowAction::SetAutostart(state.autostart));
        }
    });

    ui.add_space(8.0);
    ui.separator();
    ui.add_space(8.0);

    if reminders.is_empty() {
        ui.label(egui::RichText::new("No reminders loaded.").italics().weak());
        ui.label("Click Edit Reminders to add some.");
        return actions;
    }

    let now = Local::now();

    egui::ScrollArea::vertical().show(ui, |ui| {
        egui::Grid::new("reminders-grid")
            .num_columns(5)
            .striped(true)
            .spacing([16.0, 6.0])
            .show(ui, |ui| {
                ui.label(egui::RichText::new("Schedule").strong());
                ui.label(egui::RichText::new("Icon").strong());
                ui.label(egui::RichText::new("Message").strong());
                ui.label(egui::RichText::new("Next fire").strong());
                ui.label(egui::RichText::new("Last fired").strong());
                ui.end_row();

                for reminder in reminders {
                    ui.label(egui::RichText::new(&reminder.schedule).monospace());
                    ui.label(egui::RichText::new(&reminder.icon).size(20.0));
                    ui.label(&reminder.body);

                    let next = reminder
                        .cron
                        .find_next_occurrence(&now, false)
                        .map(|t| t.format("%Y-%m-%d %H:%M").to_string())
                        .unwrap_or_else(|_| "—".into());
                    ui.label(next);

                    let last = match last_fired.get(&reminder.schedule) {
                        Some(t) => t.format("%Y-%m-%d %H:%M:%S").to_string(),
                        None => "—".into(),
                    };
                    ui.label(last);
                    ui.end_row();
                }
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
}
