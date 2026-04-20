use iced::{
    alignment::{Horizontal, Vertical},
    time,
    widget::{
        button, canvas, column, container, image, mouse_area, radio, row, stack, text,
        text_input, Space,
    },
    Background, Color, ContentFit, Element, Length::*, Pixels, Rectangle, Subscription,
    Task, Theme,
};
use iced::widget::canvas::{Frame, Geometry, Path, Program, Stroke};
use iced_core::text::Alignment as TextAlign;
use tracing::{debug, info};
use std::time::Duration;
use url::Url;

use crate::{
    config::{Config, WallpaperType},
    music::MusicPlayer,
    timer::Timer,
};

const CANVAS_SIZE: f32 = 220.0;

pub enum AppView {
    Main,
    Settings,
}

pub struct PomodoroApp {
    view: AppView,
    in_break: bool,
    timer: Timer,
    break_timer: Timer,
    config: Config,

    main_music: MusicPlayer,
    break_music: MusicPlayer,
    music_enabled: bool,
    music_hovered: bool,

    blurred_wallpaper: Option<image::Handle>,
    video_wallpaper: Option<iced_video_player::Video>,

    // Settings form
    s_work_min: String,
    s_break_min: String,
    s_wallpaper_path: String,
    s_wallpaper_type: WallpaperType,
    s_music_enabled: bool,
}

#[derive(Debug, Clone)]
pub enum Message {
    TimerStart,
    TimerPause,
    TimerReset,
    Tick,

    MusicAreaEntered,
    MusicAreaExited,
    PrevTrack,
    NextTrack,

    OpenSettings,
    CloseSettings,
    WorkTimeInput(String),
    BreakTimeInput(String),
    WallpaperPathInput(String),
    WallpaperTypeSelected(WallpaperType),
    MusicEnabledToggled(bool),
    SettingsSave,

    VideoNewFrame,
    VideoEndOfStream,
}

impl PomodoroApp {
    pub fn new() -> (Self, Task<Message>) {
        let config = Config::load();
        let music_enabled = config.music_enabled;

        let mut main_music = MusicPlayer::new();
        let mut break_music = MusicPlayer::new();
        let base = dirs::home_dir().unwrap().join(".timer/music");
        main_music.load_dir(&base.join("main"));
        break_music.load_dir(&base.join("break"));

        let blurred_wallpaper = if config.wallpaper_type == WallpaperType::Static {
            load_blurred_image(&config.wallpaper_path)
        } else {
            None
        };

        let s_work_min = (config.work_time / 60).to_string();
        let s_break_min = (config.break_time / 60).to_string();
        let s_wallpaper_path = config.wallpaper_path.clone();
        let s_wallpaper_type = config.wallpaper_type;
        let s_music_enabled = music_enabled;
        let work_time = config.work_time;
        let break_time = config.break_time;

        let app = PomodoroApp {
            view: AppView::Main,
            in_break: false,
            timer: Timer::new(work_time),
            break_timer: Timer::new(break_time),
            config,
            main_music,
            break_music,
            music_enabled,
            music_hovered: false,
            blurred_wallpaper,
            video_wallpaper: None,
            s_work_min,
            s_break_min,
            s_wallpaper_path,
            s_wallpaper_type,
            s_music_enabled,
        };

        info!("PomodoroApp initialized");
        (app, Task::none())
    }

    pub fn title(&self) -> String {
        String::from("Pomodoro Timer")
    }

    pub fn subscription(&self) -> Subscription<Message> {
        time::every(Duration::from_millis(100)).map(|_| Message::Tick)
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Tick => {
                let done = if self.in_break {
                    self.break_timer.check_time() <= 0.0
                } else {
                    self.timer.check_time() <= 0.0
                };
                if done {
                    debug!("Tick: timer done, switching session");
                    self.switch_session();
                }
            }

            Message::TimerStart => {
                if self.in_break {
                    self.break_timer.start();
                } else {
                    self.timer.start();
                }
                if self.music_enabled {
                    self.play_current_music();
                }
            }

            Message::TimerPause => {
                if self.in_break {
                    self.break_timer.pause();
                } else {
                    self.timer.pause();
                }
                self.pause_current_music();
            }

            Message::TimerReset => {
                self.timer.reset();
                self.break_timer.reset();
                self.in_break = false;
                self.main_music.stop();
                self.break_music.stop();
                self.video_wallpaper = None;
            }

            Message::MusicAreaEntered => self.music_hovered = true,
            Message::MusicAreaExited => self.music_hovered = false,

            Message::PrevTrack => {
                if self.in_break {
                    self.break_music.prev_track();
                    if self.break_music.current_file_is_video() {
                        self.update_video_wallpaper();
                    }
                } else {
                    self.main_music.prev_track();
                    if self.main_music.current_file_is_video() {
                        self.update_video_wallpaper();
                    }
                }
            }

            Message::NextTrack => {
                if self.in_break {
                    self.break_music.next_track();
                    if self.break_music.current_file_is_video() {
                        self.update_video_wallpaper();
                    }
                } else {
                    self.main_music.next_track();
                    if self.main_music.current_file_is_video() {
                        self.update_video_wallpaper();
                    }
                }
            }

            Message::OpenSettings => {
                self.s_work_min = (self.config.work_time / 60).to_string();
                self.s_break_min = (self.config.break_time / 60).to_string();
                self.s_wallpaper_path = self.config.wallpaper_path.clone();
                self.s_wallpaper_type = self.config.wallpaper_type;
                self.s_music_enabled = self.config.music_enabled;
                self.view = AppView::Settings;
            }

            Message::CloseSettings => {
                self.view = AppView::Main;
            }

            Message::WorkTimeInput(s) => self.s_work_min = s,
            Message::BreakTimeInput(s) => self.s_break_min = s,
            Message::WallpaperPathInput(s) => self.s_wallpaper_path = s,
            Message::WallpaperTypeSelected(t) => self.s_wallpaper_type = t,
            Message::MusicEnabledToggled(v) => self.s_music_enabled = v,

            Message::SettingsSave => {
                let work_time = self.s_work_min.parse::<u64>().unwrap_or(25).max(1) * 60;
                let break_time = self.s_break_min.parse::<u64>().unwrap_or(5).max(1) * 60;

                self.config.work_time = work_time;
                self.config.break_time = break_time;
                self.config.wallpaper_path = self.s_wallpaper_path.clone();
                self.config.wallpaper_type = self.s_wallpaper_type;
                self.config.music_enabled = self.s_music_enabled;
                self.music_enabled = self.s_music_enabled;

                self.timer = Timer::new(work_time);
                self.break_timer = Timer::new(break_time);
                self.in_break = false;
                self.main_music.stop();
                self.break_music.stop();
                self.video_wallpaper = None;

                self.blurred_wallpaper = if self.config.wallpaper_type == WallpaperType::Static {
                    load_blurred_image(&self.config.wallpaper_path)
                } else {
                    None
                };

                self.config.save();
                self.view = AppView::Main;
            }

            Message::VideoNewFrame => {}

            Message::VideoEndOfStream => {
                if let Some(ref mut video) = self.video_wallpaper {
                    let _ = video.seek(Duration::ZERO, false);
                    video.set_paused(false);
                }
            }
        }

        Task::none()
    }

    // ── Private helpers ──────────────────────────────────────────────────────

    fn switch_session(&mut self) {
        self.video_wallpaper = None;
        if self.in_break {
            self.break_music.stop();
            self.timer.reset();
            self.timer.start();
            self.in_break = false;
        } else {
            self.main_music.stop();
            self.break_timer.reset();
            self.break_timer.start();
            self.in_break = true;
        }
        if self.music_enabled {
            self.play_current_music();
        }
    }

    fn play_current_music(&mut self) {
        let is_video = if self.in_break {
            self.break_music.current_file_is_video()
        } else {
            self.main_music.current_file_is_video()
        };

        if is_video {
            self.update_video_wallpaper();
        } else {
            self.video_wallpaper = None;
            if self.in_break {
                self.break_music.play();
            } else {
                self.main_music.play();
            }
        }
    }

    fn pause_current_music(&mut self) {
        if let Some(ref mut video) = self.video_wallpaper {
            video.set_paused(true);
        } else if self.in_break {
            self.break_music.pause();
        } else {
            self.main_music.pause();
        }
    }

    fn update_video_wallpaper(&mut self) {
        let path = if self.in_break {
            self.break_music.current_file_path()
        } else {
            self.main_music.current_file_path()
        };

        if let Some(p) = path {
            if let Ok(url) = Url::from_file_path(p) {
                if let Ok(mut video) = iced_video_player::Video::new(&url) {
                    video.set_looping(true);
                    video.set_paused(false);
                    self.video_wallpaper = Some(video);
                    return;
                }
            }
        }
        self.video_wallpaper = None;
    }

    // ── Views ────────────────────────────────────────────────────────────────

    pub fn view(&self) -> Element<'_, Message> {
        match self.view {
            AppView::Main => self.main_view(),
            AppView::Settings => self.settings_view(),
        }
    }

    fn main_view(&self) -> Element<'_, Message> {
        let current = if self.in_break {
            &self.break_timer
        } else {
            &self.timer
        };
        let total = current.total_time() as f64;
        let remaining = current.check_time();
        let progress = if total > 0.0 { (remaining / total) as f32 } else { 0.0 };
        debug!(total, remaining, progress, "main_view: timer state");

        // ── Timer canvas ──────────────────────────────────────────────────
        let timer_canvas = canvas(TimerCanvas {
            progress,
            remaining,
            is_break: self.in_break,
        })
        .width(CANVAS_SIZE)
        .height(CANVAS_SIZE);

        // ── Controls ──────────────────────────────────────────────────────
        let controls = row![
            button("Start").on_press(Message::TimerStart).padding([10, 22]),
            button("Pause").on_press(Message::TimerPause).padding([10, 22]),
            button("Reset").on_press(Message::TimerReset).padding([10, 22]),
        ]
        .spacing(10);

        // ── Music hover area ──────────────────────────────────────────────
        let music_inner: Element<Message> = if self.music_hovered {
            row![
                button("⏮").on_press(Message::PrevTrack).padding([6, 10]),
                text(if self.music_enabled { "♪ ON" } else { "♪ OFF" }).size(15),
                button("⏭").on_press(Message::NextTrack).padding([6, 10]),
            ]
            .spacing(8)
            .align_y(Vertical::Center)
            .into()
        } else {
            text("♪")
                .size(18)
                .color(Color::from_rgba(1.0, 1.0, 1.0, 0.35))
                .into()
        };

        let music_area = mouse_area(
            container(music_inner)
                .padding(10)
                .align_x(Horizontal::Center),
        )
        .on_enter(Message::MusicAreaEntered)
        .on_exit(Message::MusicAreaExited);

        // ── Mode label ────────────────────────────────────────────────────
        let mode_label = text(if self.in_break { "Break Time" } else { "Work Time" })
            .size(20)
            .color(Color::from_rgb(0.75, 0.75, 0.75));

        // ── Settings button ───────────────────────────────────────────────
        let settings_btn = button("⚙  Settings")
            .on_press(Message::OpenSettings)
            .padding([6, 16]);

        // ── Assemble centered UI column ───────────────────────────────────
        let ui_col = column![mode_label, timer_canvas, controls, music_area, settings_btn]
            .spacing(18)
            .align_x(Horizontal::Center);

        let centered = container(ui_col)
            .width(Fill)
            .height(Fill)
            .align_x(Horizontal::Center)
            .align_y(Vertical::Center);

        // ── Background layers ─────────────────────────────────────────────
        if let Some(ref video) = self.video_wallpaper {
            let video_bg = iced_video_player::VideoPlayer::new(video)
                .on_end_of_stream(Message::VideoEndOfStream)
                .on_new_frame(Message::VideoNewFrame);

            // Wrap video in a full-fill container so it scales to the window
            let video_container = container(video_bg).width(Fill).height(Fill);

            let overlay = container(Space::new().width(Fill).height(Fill))
                .width(Fill)
                .height(Fill)
                .style(|_: &Theme| container::Style {
                    background: Some(Background::Color(Color::from_rgba(0.0, 0.0, 0.0, 0.62))),
                    ..Default::default()
                });

            stack![video_container, overlay, centered]
                .width(Fill)
                .height(Fill)
                .into()
        } else if let Some(ref handle) = self.blurred_wallpaper {
            let bg_img = image(handle.clone())
                .width(Fill)
                .height(Fill)
                .content_fit(ContentFit::Cover);

            let overlay = container(Space::new().width(Fill).height(Fill))
                .width(Fill)
                .height(Fill)
                .style(|_: &Theme| container::Style {
                    background: Some(Background::Color(Color::from_rgba(0.0, 0.0, 0.0, 0.50))),
                    ..Default::default()
                });

            stack![bg_img, overlay, centered]
                .width(Fill)
                .height(Fill)
                .into()
        } else {
            centered.into()
        }
    }

    fn settings_view(&self) -> Element<'_, Message> {
        let title = text("Settings").size(30);

        let work_row = row![
            text("Work duration (minutes)").width(240),
            text_input("25", &self.s_work_min)
                .on_input(Message::WorkTimeInput)
                .width(80),
        ]
        .spacing(10)
        .align_y(Vertical::Center);

        let break_row = row![
            text("Break duration (minutes)").width(240),
            text_input("5", &self.s_break_min)
                .on_input(Message::BreakTimeInput)
                .width(80),
        ]
        .spacing(10)
        .align_y(Vertical::Center);

        let music_row = row![
            text("Music enabled").width(240),
            button(if self.s_music_enabled { "ON" } else { "OFF" })
                .on_press(Message::MusicEnabledToggled(!self.s_music_enabled))
                .padding([6, 16]),
        ]
        .spacing(10)
        .align_y(Vertical::Center);

        let wallpaper_header = text("Wallpaper").size(16);

        let wp_none = radio(
            "None",
            WallpaperType::None,
            Some(self.s_wallpaper_type),
            Message::WallpaperTypeSelected,
        );
        let wp_static = radio(
            "Static image",
            WallpaperType::Static,
            Some(self.s_wallpaper_type),
            Message::WallpaperTypeSelected,
        );

        let wp_path_row: Element<Message> = if self.s_wallpaper_type == WallpaperType::Static {
            column![
                text("Image path").size(14),
                text_input("/path/to/image.jpg", &self.s_wallpaper_path)
                    .on_input(Message::WallpaperPathInput)
                    .width(Fill),
            ]
            .spacing(4)
            .into()
        } else {
            Space::new().into()
        };

        let note = text(
            "Video wallpaper: place a video file (mp4, mkv, …) in ~/.timer/music/main or /break.\n\
             It will play as wallpaper (with audio) when the timer starts.",
        )
        .size(12)
        .color(Color::from_rgb(0.6, 0.6, 0.6));

        let save_row = row![
            button("Save").on_press(Message::SettingsSave).padding([10, 24]),
            button("Cancel").on_press(Message::CloseSettings).padding([10, 24]),
        ]
        .spacing(12);

        let form = column![
            title,
            work_row,
            break_row,
            music_row,
            wallpaper_header,
            wp_none,
            wp_static,
            wp_path_row,
            note,
            save_row,
        ]
        .spacing(16)
        .padding(36)
        .max_width(480);

        container(form)
            .width(Fill)
            .height(Fill)
            .align_x(Horizontal::Center)
            .align_y(Vertical::Center)
            .into()
    }
}

// ── Blurred static image loader ──────────────────────────────────────────────

fn load_blurred_image(path: &str) -> Option<image::Handle> {
    if path.is_empty() {
        return None;
    }
    let img = ::image::open(path).ok()?;
    let blurred = img.blur(18.0);
    let rgba = blurred.to_rgba8();
    let (w, h) = rgba.dimensions();
    Some(image::Handle::from_rgba(w, h, rgba.into_raw()))
}

// ── Timer canvas ──────────────────────────────────────────────────────────────

struct TimerCanvas {
    progress: f32,
    remaining: f64,
    is_break: bool,
}

impl Program<Message> for TimerCanvas {
    type State = ();

    fn draw(
        &self,
        _state: &(),
        renderer: &iced::Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: iced::mouse::Cursor,
    ) -> Vec<Geometry<iced::Renderer>> {
        use std::f32::consts::{FRAC_PI_2, TAU};
        use iced::Radians;
        use iced::widget::canvas::path::Arc;

        debug!(
            bounds_w = bounds.width,
            bounds_h = bounds.height,
            progress = self.progress,
            remaining = self.remaining,
            "TimerCanvas::draw called"
        );

        let mut frame = Frame::new(renderer, bounds.size());
        let center = iced::Point::new(bounds.width / 2.0, bounds.height / 2.0);
        let radius = (bounds.width.min(bounds.height) / 2.0) - 10.0;

        // Background ring
        frame.stroke(
            &Path::circle(center, radius),
            Stroke::default()
                .with_width(9.0)
                .with_color(Color::from_rgba(1.0, 1.0, 1.0, 0.12)),
        );

        // Progress arc
        if self.progress > 0.001 {
            let start = Radians(-FRAC_PI_2);
            let end = Radians(-FRAC_PI_2 + self.progress * TAU);
            let arc = Path::new(|b| {
                b.arc(Arc {
                    center,
                    radius,
                    start_angle: start,
                    end_angle: end,
                });
            });
            let color = if self.is_break {
                Color::from_rgb(0.25, 0.85, 0.45)
            } else {
                Color::from_rgb(0.95, 0.48, 0.12)
            };
            frame.stroke(&arc, Stroke::default().with_width(9.0).with_color(color));
        }

        // Time text
        let mins = self.remaining as u64 / 60;
        let secs = self.remaining as u64 % 60;
        frame.fill_text(iced::widget::canvas::Text {
            content: format!("{:02}:{:02}", mins, secs),
            position: center,
            color: Color::WHITE,
            size: Pixels(38.0),
            align_x: TextAlign::Center,
            align_y: Vertical::Center,
            ..Default::default()
        });

        vec![frame.into_geometry()]
    }
}
