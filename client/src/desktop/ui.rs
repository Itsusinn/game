use super::{App, Command};
use protocol::ActionType;

impl App {
    pub(super) fn ui(
        &mut self,
        ui: &mut egui::Ui,
        adapter: &str,
        commands: &mut Vec<Command>,
    ) -> egui::Rect {
        egui::Panel::top("title").exact_size(52.0).show(ui, |ui| {
            ui.horizontal_centered(|ui| {
                ui.heading(
                    egui::RichText::new("D U N G E O N")
                        .color(egui::Color32::from_rgb(137, 218, 190)),
                );
                ui.separator();
                ui.label("Multiplayer dungeon");
            });
        });
        egui::Panel::bottom("footer")
            .exact_size(28.0)
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.small(&self.session.status);
                    ui.separator();
                    ui.small(format!(
                        "{} pending",
                        self.session.model.pending.pending_count()
                    ));
                });
            });
        egui::Panel::right("sidebar").exact_size(292.0).resizable(false).show(ui, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.add_space(12.0);
                if self.session.transport.is_none() {
                    ui.heading("Enter the dungeon"); ui.add_space(8.0);
                    ui.label("Server address"); ui.text_edit_singleline(&mut self.session.address);
                    ui.label("Player name"); ui.text_edit_singleline(&mut self.session.name);
                    ui.label("Server certificate (.der)"); ui.text_edit_singleline(&mut self.session.certificate);
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        if ui.button("Connect").clicked() { commands.push(Command::Connect); }
                        if ui.button("Renderer demo").clicked() { commands.push(Command::Demo); }
                    });
                    ui.small("Run cargo run -p server first.");
                } else {
                    ui.heading(&self.session.name);
                    if ui.button("Disconnect").clicked() { commands.push(Command::Disconnect); }
                }
                ui.add_space(14.0); ui.separator(); ui.add_space(8.0);
                let s = &self.session.model.state;
                ui.label(egui::RichText::new("EXPLORER").strong());
                ui.add(egui::ProgressBar::new(s.hp.max(0) as f32/s.max_hp.max(1) as f32).text(format!("Health  {} / {}",s.hp,s.max_hp)).fill(egui::Color32::from_rgb(72,153,117)));
                ui.add(egui::ProgressBar::new(s.stamina as f32/100.0).text(format!("Stamina  {}",s.stamina)));
                ui.label(format!("Hunger {}    Thirst {}",s.hunger,s.thirst));
                ui.monospace(format!("({}, {})  /  region {:?}",s.player_pos.x,s.player_pos.y,s.sub_world_id));
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if ui.add_enabled(self.session.model.logged_in,egui::Button::new("Wait")).clicked() { commands.push(Command::Action(ActionType::Wait)); }
                    if ui.add_enabled(self.session.model.logged_in,egui::Button::new("Attack")).clicked() { commands.push(Command::Action(ActionType::MeleeAttack)); }
                    if ui.add_enabled(self.session.model.logged_in,egui::Button::new("Pick up")).clicked() { commands.push(Command::Action(ActionType::Pickup)); }
                });
                ui.add_space(10.0); ui.separator();
                ui.checkbox(&mut self.inventory,"Inventory [Tab]");
                if self.inventory {
                    if s.inventory.is_empty() { ui.weak("Your pack is empty."); }
                    for item in &s.inventory { ui.label(format!("{}   x{}",item.name,item.quantity)); }
                }
                ui.add_space(10.0); ui.separator(); ui.label(egui::RichText::new("JOURNAL").strong());
                for entry in s.message_log.iter().rev().take(10).rev() {
                    let c = entry.color;
                    ui.label(egui::RichText::new(&entry.text).color(egui::Color32::from_rgb((c>>16) as u8,(c>>8) as u8,c as u8)));
                }
                let chat_response = ui.add_enabled(self.session.model.logged_in,egui::TextEdit::singleline(&mut self.chat).hint_text("Chat - Enter to send"));
                if chat_response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) && !self.chat.trim().is_empty() {
                    commands.push(Command::Action(ActionType::Chat { text: self.chat.clone() }));
                }
                ui.add_space(10.0); ui.separator();
                ui.label("WASD / arrows: move\nQ E Z C: diagonal\nF: attack    G: pick up\nSpace / .: wait    Wheel: zoom");
                ui.add(egui::Slider::new(&mut self.zoom,12.0..=48.0).text("Tile size"));
                ui.add_space(8.0); ui.small(adapter);
            });
        });
        ui.available_rect_before_wrap()
    }
}
