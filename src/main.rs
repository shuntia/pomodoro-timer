// This is the entry point of the application. It initializes the Pomodoro timer and manages the main application loop.

use std::fs::{create_dir_all, exists};
use std::os;
use std::path::Path;

use eframe::egui::*;
use eframe::*;

use dirs::home_dir;

mod music;
mod timer;
mod window;

fn main() {
    check_env();
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

fn check_env(){
    let music_dir = home_dir().unwrap().join(".timer/music");
    let main_music_dir = music_dir.join("main");
    let break_music_dir = music_dir.join("break");
    let fx_music_dir = music_dir.join("fx");
    if !exists(music_dir).expect("Failed to read if music dir exists!"){
        create_dir_all(main_music_dir).unwrap();
        create_dir_all(break_music_dir).unwrap();
        create_dir_all(fx_music_dir).unwrap();
    }
}