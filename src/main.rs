// This is the entry point of the application. It initializes the Pomodoro timer and manages the main application loop.

use eframe::egui::*;
use eframe::*;

mod music;
mod timer;
mod window;

fn main() {
    let mut window = window::Window::new();
    window.load_music();
    let options = NativeOptions::default();

    eframe::run_native(
        "Timer",
        options,
        Box::new(|cc| {
            let mut style = (*cc.egui_ctx.style()).clone();
            style.text_styles = [
                (
                    TextStyle::Heading,
                    FontId::new(40.0, FontFamily::Proportional),
                ),
                (TextStyle::Body, FontId::new(30.0, FontFamily::Proportional)),
                (
                    TextStyle::Monospace,
                    FontId::new(30.0, FontFamily::Monospace),
                ),
                (
                    TextStyle::Button,
                    FontId::new(30.0, FontFamily::Proportional),
                ),
                (
                    TextStyle::Small,
                    FontId::new(20.0, FontFamily::Proportional),
                ),
            ]
            .into();
            style.spacing.item_spacing = Vec2::new(10.0, 10.0);
            cc.egui_ctx.set_style(style);
            Ok(Box::new(window))
        }),
    )
    .unwrap();
}
