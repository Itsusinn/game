use super::{App, Command};

impl App {
    pub(super) fn ui(&mut self, ui: &mut egui::Ui, commands: &mut Vec<Command>) -> egui::Rect {
        egui::Panel::top("title").exact_size(44.0).show(ui, |ui| {
            ui.horizontal_centered(|ui| {
                ui.heading(
                    egui::RichText::new("D U N G E O N  /  3 D")
                        .color(egui::Color32::from_rgb(190, 208, 170)),
                );
                ui.separator();
                ui.label(if self.session.demo {
                    "Offline walkthrough"
                } else {
                    "Voxel frontier"
                });
                if self.session.model.ready {
                    ui.separator();
                    ui.label(format!("Health  {} / 100", self.session.model.hp));
                }
            });
        });
        egui::Panel::bottom("footer").exact_size(48.0).show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.small(&self.session.status);
                ui.separator();
                ui.small(format!("{} pending", self.session.model.pending.len()));
            });
            let help = if self.captured {
                "WASD Move   Space Jump   Click / F Attack   G Pick up   Tab Inventory   Esc Release mouse"
            } else {
                "Click the world to look around   WASD Move   Space Jump   Tab Inventory   Esc Release mouse"
            };
            ui.small(help);
        });
        if !self.session.model.ready || self.inventory {
            egui::Panel::right("sidebar")
                .exact_size(300.0)
                .show(ui, |ui| {
                    egui::ScrollArea::vertical().show(ui, |ui| self.sidebar(ui, commands));
                });
        }
        let rect = ui.available_rect_before_wrap();
        if self.session.model.ready {
            let painter = ui.painter();
            let center = rect.center();
            let stroke = egui::Stroke::new(
                1.4,
                egui::Color32::from_rgba_unmultiplied(245, 245, 230, 210),
            );
            painter.line_segment(
                [
                    center + egui::vec2(-6.0, 0.0),
                    center + egui::vec2(6.0, 0.0),
                ],
                stroke,
            );
            painter.line_segment(
                [
                    center + egui::vec2(0.0, -6.0),
                    center + egui::vec2(0.0, 6.0),
                ],
                stroke,
            );
            if !self.captured {
                painter.text(
                    center + egui::vec2(0.0, 35.0),
                    egui::Align2::CENTER_CENTER,
                    "Click to explore",
                    egui::FontId::proportional(16.0),
                    egui::Color32::WHITE,
                );
            }
        }
        rect
    }

    fn sidebar(&mut self, ui: &mut egui::Ui, commands: &mut Vec<Command>) {
        ui.add_space(12.0);
        if self.session.transport.is_none() {
            ui.heading("Explore the frontier");
            ui.add_space(10.0);
            ui.label("Server");
            ui.text_edit_singleline(&mut self.session.address);
            ui.label("Name");
            ui.text_edit_singleline(&mut self.session.name);
            ui.label("Server certificate (.der)");
            ui.text_edit_singleline(&mut self.session.certificate);
            ui.horizontal(|ui| {
                if ui.button("Connect").clicked() {
                    commands.push(Command::Connect);
                }
                if ui.button("3D walkthrough").clicked() {
                    commands.push(Command::Demo);
                }
            });
        } else {
            ui.heading(&self.session.name);
            if ui.button("Disconnect").clicked() {
                commands.push(Command::Disconnect);
            }
        }
        ui.add_space(16.0);
        ui.separator();
        ui.heading("Inventory");
        if self.session.model.inventory.is_empty() {
            ui.weak("Your pack is empty.");
        }
        for item in &self.session.model.inventory {
            ui.label(format!("{}  x{}", item.name, item.quantity));
        }
        ui.add_space(16.0);
        ui.separator();
        ui.heading("Journal");
        for entry in &self.session.model.log {
            ui.label(&entry.text);
        }
        let response = ui.add_enabled(
            self.session.model.ready && !self.session.demo,
            egui::TextEdit::singleline(&mut self.chat).hint_text("Chat - Enter to send"),
        );
        if response.lost_focus()
            && ui.input(|i| i.key_pressed(egui::Key::Enter))
            && !self.chat.trim().is_empty()
        {
            commands.push(Command::Chat(self.chat.clone()));
        }
        ui.add_space(16.0);
        ui.add(egui::Slider::new(&mut self.fov, 55.0..=100.0).text("Field of view"));
        let p = self.session.model.body.position;
        ui.monospace(format!("X {:.1}  Y {:.1}  Z {:.1}", p[0], p[1], p[2]));
        ui.weak("WASD / arrows: move\nMouse: look\nSpace: jump\nClick / F: melee\nG: pick up\nTab: inventory\nEsc: release mouse");
    }
}
