//! A narrow-friendly single input and vertically ordered conversation.
use crate::{ChatRole, ChatState};
use bevy_egui::egui;

/// Draw the flow and return an utterance to be sent through `chat.send` by the caller.
pub fn chat_panel(ui: &mut egui::Ui, state: &ChatState, input: &mut String) -> Option<String> {
    let mut sent = None;
    let ctx = ui.ctx().clone();
    egui::CentralPanel::default().show(ui, |ui| {
        ui.vertical(|ui| {
            egui::ScrollArea::vertical()
                .stick_to_bottom(true)
                .max_height((ui.available_height() - 40.0).max(0.0))
                .show(ui, |ui| {
                    for entry in &state.0 {
                        let role = match entry.role {
                            ChatRole::User => "chat-user",
                            ChatRole::Agent => "chat-agent",
                            ChatRole::System => "chat-system",
                        };
                        ui.label(format!(
                            "{}: {}",
                            corundum_framework::gui::i18n::tr(&ctx, role),
                            entry.text
                        ));
                    }
                });
            ui.horizontal(|ui| {
                let send_label = corundum_framework::gui::i18n::tr(&ctx, "chat-send");
                let width = (ui.available_width() - 65.0).max(0.0);
                let response = ui.add_sized([width, 24.0], egui::TextEdit::singleline(input));
                let enter = response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                if (ui.button(send_label).clicked() || enter) && !input.trim().is_empty() {
                    sent = Some(std::mem::take(input));
                }
            });
        });
    });
    sent
}
