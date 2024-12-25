use crate::music::MusicPlayer;
use crate::timer::Timer;
use dirs::home_dir;
use eframe::egui;
use std::time::Duration;

const WORK_TIME: u64 = 25 * 60;
const BREAK_TIME: u64 = 5 * 60;

pub struct Window {
    in_break: bool,
    timer: Timer,
    break_timer: Timer,
    main_music: MusicPlayer,
    break_music: MusicPlayer,
    fx_music: MusicPlayer,
}

impl eframe::App for Window {
    fn update(&mut self, ctx: &egui::Context, _: &mut eframe::Frame) {
        let (current_timer, other_timer) = if self.in_break {
            (&mut self.break_timer, &mut self.timer)
        } else {
            (&mut self.timer, &mut self.break_timer)
        };
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Pomodoro Timer");
            ui.separator();
            ui.heading(if self.in_break {
                "Break Time"
            } else {
                "Work Time"
            });

            // Circular timer display with arc
            let total_time = current_timer.total_time() as f64;
            let remaining_time = current_timer.check_time();
            let progress: f32 = (remaining_time / total_time) as f32;
            let (rect, response) =
                ui.allocate_exact_size(egui::vec2(200.0, 200.0), egui::Sense::hover());
            let center = rect.center();
            let radius = rect.width() / 2.0;
            let start_angle = std::f32::consts::PI * 1.5;
            let end_angle = start_angle + progress * std::f32::consts::PI * 2.0;

            ui.painter().add(egui::Shape::circle_stroke(
                center,
                radius,
                egui::Stroke::new(10.0, ui.visuals().widgets.active.bg_fill),
            ));

            // Draw the arc
            let points: Vec<egui::Pos2> = (0..=100)
                .map(|i| {
                    let t = i as f32 / 100.0;
                    let angle = start_angle + t * (end_angle - start_angle);
                    egui::pos2(
                        center.x + radius * angle.cos(),
                        center.y + radius * angle.sin(),
                    )
                })
                .collect();
            ui.painter().add(egui::Shape::line(
                points,
                egui::Stroke::new(10.0, ui.visuals().widgets.active.fg_stroke.color),
            ));

            ui.painter().text(
                center,
                egui::Align2::CENTER_CENTER,
                format!(
                    "{:02}:{:02}",
                    remaining_time as u64 / 60,
                    remaining_time as u64 % 60
                ),
                egui::FontId::proportional(32.0),
                ui.visuals().text_color(),
            );

            ui.spacing();
            ui.separator();
            ui.spacing();
            ui.horizontal(|ui| {
                if ui.button("Start").clicked() {
                    current_timer.start();
                    if self.in_break {
                        self.break_music.play();
                    } else {
                        self.main_music.play();
                    }
                }
                if ui.button("Pause").clicked() {
                    current_timer.pause();
                    if self.in_break {
                        self.break_music.pause();
                    } else {
                        self.main_music.pause();
                    }
                }
                if ui.button("Reset").clicked() {
                    other_timer.reset();
                    current_timer.reset();
                    self.in_break = false;
                    self.break_music.stop();
                    self.main_music.stop();
                }
            });
        });
        if current_timer.check_time() <= 0.0 {
            if self.in_break {
                self.break_music.pause();
                self.timer.reset();
                self.main_music.play();
                self.timer.start();
            } else {
                self.main_music.pause();
                self.break_timer.reset();
                self.break_music.play();
                self.break_timer.start();
            }
            self.in_break = !self.in_break;
        }
        ctx.request_repaint();
    }
}

impl Default for Window {
    fn default() -> Self {
        Self::new()
    }
}

impl Window {
    pub fn new() -> Self {
        Window {
            in_break: false,
            timer: Timer::new(WORK_TIME),
            break_timer: Timer::new(BREAK_TIME),
            main_music: MusicPlayer::new(),
            break_music: MusicPlayer::new(),
            fx_music: MusicPlayer::new(),
        }
    }
    pub fn load_music(&mut self) {
        let music_dir = home_dir().unwrap().join(".timer/music");
        let main_music_dir = music_dir.join("main");
        let break_music_dir = music_dir.join("break");
        let fx_music_dir = music_dir.join("fx");
        self.main_music.load_dir(&main_music_dir);
        self.break_music.load_dir(&break_music_dir);
        self.fx_music.load_dir(&fx_music_dir);
    }
}
