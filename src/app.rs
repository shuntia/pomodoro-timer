use iced::{
    alignment::{Horizontal, Vertical},
    event, mouse as icedmouse, time,
    widget::{
        button, canvas, column, container, image, mouse_area, pick_list, radio, row, scrollable,
        stack, text, text_input, Space,
    },
    Background, Border, Color, ContentFit, Element, Event, Length::*, Padding, Pixels, Rectangle,
    Subscription, Task, Theme,
};
use iced::widget::canvas::{Frame, Geometry, Path, Program, Stroke};
use iced_core::text::Alignment as TextAlign;
use tracing::{debug, info};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::time::{Duration, Instant};
use url::Url;

use crate::{
    config::{Config, TimerFont, WallpaperType},
    music::{MusicPlayer, SoundPlayer},
    timer::Timer,
};

// Vertical space reserved for the top (cog) and bottom (controls + music) overlays.
// The canvas uses these to compute a circle that never overlaps the controls.
const OVERLAY_TOP: f32 = 60.0;
const OVERLAY_BOTTOM: f32 = 130.0;
const OVERLAY_SIDE: f32 = 40.0;

// Fixed width box for the track name label (≈24 chars @ size 14).
const TRACK_NAME_MAX_W: f32 = 180.0;
// Height of the progress bar strip at the very bottom of the screen.
const TRACK_BAR_H: f32 = 5.0;

// Seconds of mouse inactivity before the timer controls fully fade out.
const CONTROLS_IDLE_SECS: f64 = 2.0;

// Resting opacity for the fading icon buttons.
const MUSIC_REST: f32 = 0.18;
const COG_REST: f32 = 0.22;
// Per-tick lerp speed (100ms tick): asymmetric — fast in, slow out.
const FADE_IN: f32 = 0.35;
const FADE_OUT: f32 = 0.16;

// ── Color helpers ─────────────────────────────────────────────────────────────

fn parse_hex_color(s: &str) -> Option<[f32; 3]> {
    let s = s.trim().trim_start_matches('#');
    if s.len() != 6 {
        return None;
    }
    let r = u8::from_str_radix(&s[0..2], 16).ok()?;
    let g = u8::from_str_radix(&s[2..4], 16).ok()?;
    let b = u8::from_str_radix(&s[4..6], 16).ok()?;
    Some([r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0])
}

fn to_hex_color(c: [f32; 3]) -> String {
    let [r, g, b] = c;
    format!(
        "{:02x}{:02x}{:02x}",
        (r * 255.0).round() as u8,
        (g * 255.0).round() as u8,
        (b * 255.0).round() as u8
    )
}

fn arr_to_color(c: [f32; 3]) -> Color {
    Color::from_rgb(c[0], c[1], c[2])
}

fn timer_font_to_iced(f: TimerFont, custom_name: Option<&'static str>) -> iced::Font {
    use iced::font::Family;
    match f {
        TimerFont::Default => iced::Font::DEFAULT,
        TimerFont::Monospace => iced::Font::MONOSPACE,
        TimerFont::Serif => iced::Font { family: Family::Serif, ..iced::Font::DEFAULT },
        TimerFont::SansSerif => iced::Font { family: Family::SansSerif, ..iced::Font::DEFAULT },
        TimerFont::Cursive => iced::Font { family: Family::Cursive, ..iced::Font::DEFAULT },
        TimerFont::Fantasy => iced::Font { family: Family::Fantasy, ..iced::Font::DEFAULT },
        TimerFont::Custom => match custom_name {
            Some(name) => iced::Font { family: Family::Name(name), ..iced::Font::DEFAULT },
            None => iced::Font::DEFAULT,
        },
    }
}

fn font_family_name(bytes: &[u8]) -> Option<String> {
    let face = ttf_parser::Face::parse(bytes, 0).ok()?;
    face.names()
        .into_iter()
        .filter(|n| n.name_id == ttf_parser::name_id::FAMILY)
        .find_map(|n| {
            if n.is_unicode() {
                let s: String = char::decode_utf16(
                    n.name.chunks_exact(2).map(|c| u16::from_be_bytes([c[0], c[1]])),
                )
                .filter_map(Result::ok)
                .collect();
                if s.is_empty() { None } else { Some(s) }
            } else {
                std::str::from_utf8(n.name).ok().map(|s| s.to_string())
            }
        })
}

/// Read the family name from a TTF/OTF file without loading its bytes into iced.
/// Used only to pre-fill the font-name text field when the user browses for a file.
fn extract_font_name_from_file(path: &str) -> Option<String> {
    let bytes = std::fs::read(path).ok()?;
    font_family_name(&bytes)
}

fn leak_font_name(name: &str) -> &'static str {
    Box::leak(name.to_owned().into_boxed_str())
}

// ── Button style ──────────────────────────────────────────────────────────────

fn ghost_button(theme: &Theme, status: button::Status) -> button::Style {
    let palette = theme.extended_palette();
    let is_dark = palette.background.base.color.r < 0.5;
    let tint: f32 = if is_dark { 1.0 } else { 0.0 };
    let bg_alpha = match status {
        button::Status::Hovered => 0.12,
        button::Status::Pressed => 0.22,
        _ => 0.0,
    };
    let border_alpha = match status {
        button::Status::Hovered | button::Status::Pressed => 0.25,
        _ => 0.10,
    };
    button::Style {
        background: Some(Background::Color(Color::from_rgba(tint, tint, tint, bg_alpha))),
        text_color: palette.background.base.text,
        border: Border {
            color: Color::from_rgba(tint, tint, tint, border_alpha),
            width: 1.0,
            radius: 8.0.into(),
        },
        ..Default::default()
    }
}

// Text-label button style with opacity applied to every colour channel.
fn ghost_button_faded(alpha: f32) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |theme, status| {
        let mut s = ghost_button(theme, status);
        s.text_color.a *= alpha;
        if let Some(Background::Color(ref mut c)) = s.background {
            c.a *= alpha;
        }
        s.border.color.a *= alpha;
        s
    }
}

// Returns a button style closure with the given opacity baked in.
fn ghost_icon_button(opacity: f32) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |theme, status| {
        let palette = theme.extended_palette();
        let is_dark = palette.background.base.color.r < 0.5;
        let tint: f32 = if is_dark { 1.0 } else { 0.0 };
        let bg_alpha = match status {
            button::Status::Hovered => (opacity + 0.10).min(0.28),
            button::Status::Pressed => (opacity + 0.18).min(0.38),
            _ => 0.0,
        };
        button::Style {
            background: Some(Background::Color(Color::from_rgba(tint, tint, tint, bg_alpha))),
            text_color: Color::from_rgba(tint, tint, tint, opacity),
            border: Border {
                color: Color::from_rgba(tint, tint, tint, opacity * 0.30),
                width: 1.0,
                radius: 8.0.into(),
            },
            ..Default::default()
        }
    }
}

// ── Overlay helpers ───────────────────────────────────────────────────────────

fn tint_overlay(tint: Color, strength: f32) -> Element<'static, Message> {
    let color = Color { a: strength.clamp(0.0, 1.0), ..tint };
    container(Space::new().width(Fill).height(Fill))
        .width(Fill)
        .height(Fill)
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(color)),
            ..Default::default()
        })
        .into()
}

// ── App state ─────────────────────────────────────────────────────────────────

pub enum AppView {
    Main,
    Settings,
}

pub struct PomodoroApp {
    pub detected_theme: Theme,
    view: AppView,
    in_break: bool,
    timer: Timer,
    break_timer: Timer,
    config: Config,

    main_music: MusicPlayer,
    break_music: MusicPlayer,
    arc_fill: f32,    // CCW tip — fill animation (0 → 1 on start)
    arc_elapsed: f32, // 12-o'clock end — tracks elapsed/total directly
    custom_font_name: Option<&'static str>,

    music_enabled: bool,
    music_hovered: bool,
    music_progress: f32,
    cog_hovered: bool,
    cog_progress: f32,

    blurred_wallpaper: Option<image::Handle>,
    thumbnail_wallpaper: Option<image::Handle>,
    // None value = "checked, no art"; absent = not yet checked
    thumb_cache: HashMap<PathBuf, Option<image::Handle>>,
    thumb_pending: HashSet<PathBuf>,
    #[cfg(feature = "video")]
    video_wallpaper: Option<iced_video_player::Video>,
    // Pre-buffered video: (path, video) — validated against current track before use.
    #[cfg(feature = "video")]
    video_preroll: Option<(PathBuf, iced_video_player::Video)>,

    // Controls fade: 0 = invisible, 1 = fully visible; driven by mouse activity.
    controls_opacity: f32,
    last_mouse_move: Instant,
    window_size: iced::Size,

    // Countdown + bell one-shot players.
    countdown_player: SoundPlayer,
    bell_player: SoundPlayer,
    // True while we're waiting for the countdown sound to finish before switching.
    awaiting_countdown: bool,

    // Settings form state
    s_work_min: String,
    s_break_min: String,
    s_wallpaper_path: String,
    s_wallpaper_type: WallpaperType,
    s_music_enabled: bool,
    s_main_music_dir: String,
    s_break_music_dir: String,
    s_accent_hex: String,
    s_break_hex: String,
    s_timer_font: TimerFont,
    s_custom_font_name: String,
    s_mode_font_size_scale: String,
    s_mode_font_color_hex: String,
    s_mode_font_opacity: String,
    s_blur_intensity: String,
    s_font_size_scale: String,
    s_ring_thickness_scale: String,
    s_ring_bg_opacity: String,
    s_bg_tint_hex: String,
    s_bg_tint_strength: String,
    s_countdown_path: String,
    s_bell_path: String,
}

#[derive(Debug, Clone)]
pub enum Message {
    TimerStart,
    TimerPause,
    TimerReset,
    Tick,

    MouseMoved,
    MusicAreaEntered,
    MusicAreaExited,
    CogEntered,
    CogExited,
    ThumbnailCached(PathBuf, Option<image::Handle>),
    PrevTrack,
    NextTrack,

    OpenSettings,
    CloseSettings,
    WorkTimeInput(String),
    BreakTimeInput(String),
    WallpaperPathInput(String),
    WallpaperTypeSelected(WallpaperType),
    MusicEnabledToggled(bool),
    MainMusicDirInput(String),
    BreakMusicDirInput(String),
    AccentColorInput(String),
    BreakColorInput(String),
    TimerFontSelected(TimerFont),
    PickMainMusicDir,
    PickBreakMusicDir,
    PickWallpaperFile,
    PickFontFile,
    MainMusicDirPicked(Option<String>),
    BreakMusicDirPicked(Option<String>),
    WallpaperFilePicked(Option<String>),
    FontFilePicked(Option<String>),
    CustomFontNameInput(String),
    ModeFontSizeScaleInput(String),
    ModeFontColorInput(String),
    ModeFontOpacityInput(String),
    SkipSession,
    WindowResized(iced::Size),
    SettingsSave,

    #[cfg(feature = "video")]
    VideoNewFrame,
    #[cfg(feature = "video")]
    VideoEndOfStream,
    BlurIntensityInput(String),
    FontSizeScaleInput(String),
    RingThicknessInput(String),
    RingBgOpacityInput(String),
    BgTintHexInput(String),
    BgTintStrengthInput(String),
    CountdownPathInput(String),
    BellPathInput(String),
    PickCountdownFile,
    PickBellFile,
    CountdownFilePicked(Option<String>),
    BellFilePicked(Option<String>),
}

impl PomodoroApp {
    pub fn new() -> (Self, Task<Message>) {
        let config = Config::load();
        let music_enabled = config.music_enabled;

        let mut main_music = MusicPlayer::new();
        let mut break_music = MusicPlayer::new();
        main_music.load_dir(std::path::Path::new(&config.main_music_dir));
        break_music.load_dir(std::path::Path::new(&config.break_music_dir));

        let blurred_wallpaper = if config.wallpaper_type == WallpaperType::Static {
            load_blurred_image(&config.wallpaper_path, config.blur_intensity)
        } else {
            None
        };

        let detected_theme = match dark_light::detect() {
            Ok(dark_light::Mode::Light) => Theme::Light,
            _ => Theme::Dark,
        };

        let s_work_min = (config.work_time / 60).to_string();
        let s_break_min = (config.break_time / 60).to_string();
        let s_wallpaper_path = config.wallpaper_path.clone();
        let s_wallpaper_type = config.wallpaper_type;
        let s_music_enabled = music_enabled;
        let s_main_music_dir = config.main_music_dir.clone();
        let s_break_music_dir = config.break_music_dir.clone();
        let s_accent_hex = to_hex_color(config.accent_color);
        let s_break_hex = to_hex_color(config.break_color);
        let s_timer_font = config.timer_font;
        let s_custom_font_name = config.custom_font_name.clone();
        let s_mode_font_size_scale = config.mode_font_size_scale.to_string();
        let s_mode_font_color_hex = to_hex_color(config.mode_font_color);
        let s_mode_font_opacity = config.mode_font_opacity.to_string();
        let s_blur_intensity = config.blur_intensity.to_string();
        let s_font_size_scale = config.font_size_scale.to_string();
        let s_ring_thickness_scale = config.ring_thickness_scale.to_string();
        let s_ring_bg_opacity = config.ring_bg_opacity.to_string();
        let s_bg_tint_hex = to_hex_color(config.bg_tint);
        let s_bg_tint_strength = config.bg_tint_strength.to_string();
        let s_countdown_path = config.countdown_sound_path.clone();
        let s_bell_path = config.bell_sound_path.clone();
        let work_time = config.work_time;
        let break_time = config.break_time;

        let custom_font_name =
            if config.timer_font == TimerFont::Custom && !config.custom_font_name.is_empty() {
                Some(leak_font_name(&config.custom_font_name))
            } else {
                None
            };

        let mut app = PomodoroApp {
            detected_theme,
            view: AppView::Main,
            in_break: false,
            timer: Timer::new(work_time),
            break_timer: Timer::new(break_time),
            config,
            main_music,
            break_music,
            arc_fill: 0.0,
            arc_elapsed: 0.0,
            custom_font_name,
            music_enabled,
            music_hovered: false,
            music_progress: MUSIC_REST,
            cog_hovered: false,
            cog_progress: COG_REST,
            blurred_wallpaper,
            thumbnail_wallpaper: None,
            thumb_cache: HashMap::new(),
            thumb_pending: HashSet::new(),
            #[cfg(feature = "video")]
            video_wallpaper: None,
            #[cfg(feature = "video")]
            video_preroll: None,
            controls_opacity: 0.0,
            last_mouse_move: Instant::now(),
            window_size: iced::Size::new(1280.0, 720.0),
            countdown_player: SoundPlayer::new(),
            bell_player: SoundPlayer::new(),
            awaiting_countdown: false,
            s_work_min,
            s_break_min,
            s_wallpaper_path,
            s_wallpaper_type,
            s_music_enabled,
            s_main_music_dir,
            s_break_music_dir,
            s_accent_hex,
            s_break_hex,
            s_timer_font,
            s_custom_font_name,
            s_mode_font_size_scale,
            s_mode_font_color_hex,
            s_mode_font_opacity,
            s_blur_intensity,
            s_font_size_scale,
            s_ring_thickness_scale,
            s_ring_bg_opacity,
            s_bg_tint_hex,
            s_bg_tint_strength,
            s_countdown_path,
            s_bell_path,
        };

        #[cfg(feature = "video")]
        app.preroll_video();
        info!("PomodoroApp initialized, theme={:?}", app.detected_theme);
        (app, Task::none())
    }

    pub fn title(&self) -> String {
        String::from("Pomodoro Timer")
    }

    pub fn subscription(&self) -> Subscription<Message> {
        let tick = time::every(Duration::from_millis(16)).map(|_| Message::Tick);
        let mouse = event::listen_with(|ev, _, _| match ev {
            Event::Mouse(icedmouse::Event::CursorMoved { .. }) => Some(Message::MouseMoved),
            _ => None,
        });
        let resize = iced::window::resize_events()
            .map(|(_, size)| Message::WindowResized(size));
        Subscription::batch([tick, mouse, resize])
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Tick => {
                // Countdown phase: wait for the countdown sound to finish, then switch.
                if self.awaiting_countdown {
                    if self.countdown_player.is_done() {
                        self.awaiting_countdown = false;
                        return self.finish_session();
                    }
                    return Task::none();
                }

                let current = if self.in_break { &self.break_timer } else { &self.timer };
                let done = current.check_time() <= 0.0 && current.is_running();
                if done {
                    debug!("Tick: timer done");
                    if !self.config.countdown_sound_path.is_empty() {
                        return self.start_countdown_phase();
                    }
                    return self.finish_session();
                }

                // Smoothly animate the arc progress.
                let current = if self.in_break { &self.break_timer } else { &self.timer };
                if current.is_running() {
                    let total = current.total_time() as f32;
                    let remaining = current.check_time() as f32;
                    // arc_fill: CCW tip lerps toward 1.0 (fast fill-in animation)
                    self.arc_fill = (self.arc_fill + (1.0 - self.arc_fill) * 0.067)
                        .clamp(0.0, 1.0);
                    // arc_elapsed: direct elapsed fraction (no lerp — tracks real time)
                    self.arc_elapsed = if total > 0.0 { (total - remaining) / total } else { 0.0 };
                }

                // Animate music controls opacity (fast in, slow out).
                let mt = if self.music_hovered { 1.0_f32 } else { MUSIC_REST };
                let ms = if mt > self.music_progress { FADE_IN } else { FADE_OUT };
                self.music_progress = (self.music_progress + (mt - self.music_progress) * ms)
                    .clamp(0.0, 1.0);

                // Animate cog opacity.
                let ct = if self.cog_hovered { 1.0_f32 } else { COG_REST };
                let cs = if ct > self.cog_progress { FADE_IN } else { FADE_OUT };
                self.cog_progress = (self.cog_progress + (ct - self.cog_progress) * cs)
                    .clamp(0.0, 1.0);

                // Animate controls opacity: show on mouse activity, fade after idle.
                let idle = self.last_mouse_move.elapsed().as_secs_f64();
                let ctrl_target = if idle < CONTROLS_IDLE_SECS { 1.0_f32 } else { 0.0 };
                let ctrl_speed = if ctrl_target > self.controls_opacity { FADE_IN } else { FADE_OUT };
                self.controls_opacity = (self.controls_opacity
                    + (ctrl_target - self.controls_opacity) * ctrl_speed)
                    .clamp(0.0, 1.0);
            }

            Message::TimerStart => {
                // Only snap to 0 on a fresh start; resuming from pause keeps the arc where it is.
                let is_fresh = if self.in_break {
                    self.break_timer.is_idle()
                } else {
                    self.timer.is_idle()
                };
                if is_fresh {
                    self.arc_fill = 0.0;
                    self.arc_elapsed = 0.0;
                }
                if self.in_break {
                    self.break_timer.start();
                } else {
                    self.timer.start();
                }
                let is_video = if self.in_break {
                    self.break_music.current_file_is_video()
                } else {
                    self.main_music.current_file_is_video()
                };
                #[cfg(feature = "video")]
                if is_video {
                    let cur_path = if self.in_break {
                        self.break_music.current_file_path()
                    } else {
                        self.main_music.current_file_path()
                    }.map(|p| p.to_path_buf());
                    let preroll_valid = self.video_preroll.as_ref()
                        .zip(cur_path.as_ref())
                        .map(|((stored_path, _), cur)| stored_path == cur)
                        .unwrap_or(false);
                    if preroll_valid {
                        if let Some((_, mut video)) = self.video_preroll.take() {
                            debug!("using pre-buffered video");
                            video.set_paused(false);
                            self.video_wallpaper = Some(video);
                        }
                    } else {
                        self.video_preroll = None;
                        self.update_video_wallpaper();
                    }
                    return Task::none();
                }
                #[cfg(not(feature = "video"))]
                let _ = is_video;
                if self.music_enabled {
                    self.play_current_music();
                }
                return self.update_thumbnail_wallpaper();
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
                self.arc_fill = 0.0;
                self.arc_elapsed = 0.0;
                self.awaiting_countdown = false;
                self.countdown_player.stop();
                self.timer.reset();
                self.break_timer.reset();
                self.in_break = false;
                self.main_music.stop();
                self.break_music.stop();
                #[cfg(feature = "video")]
                { self.video_wallpaper = None; self.video_preroll = None; }
                self.thumbnail_wallpaper = None;
                #[cfg(feature = "video")]
                self.preroll_video();
            }

            Message::MouseMoved => {
                self.last_mouse_move = Instant::now();
            }

            Message::MusicAreaEntered => self.music_hovered = true,
            Message::MusicAreaExited => self.music_hovered = false,
            Message::CogEntered => self.cog_hovered = true,
            Message::CogExited => self.cog_hovered = false,

            Message::PrevTrack => {
                if self.in_break {
                    self.break_music.prev_track();
                } else {
                    self.main_music.prev_track();
                }
                let timer_running = if self.in_break { self.break_timer.is_running() } else { self.timer.is_running() };
                #[cfg(feature = "video")]
                if !timer_running { self.preroll_video(); }
                return self.refresh_media_background();
            }

            Message::NextTrack => {
                if self.in_break {
                    self.break_music.next_track();
                } else {
                    self.main_music.next_track();
                }
                let timer_running = if self.in_break { self.break_timer.is_running() } else { self.timer.is_running() };
                #[cfg(feature = "video")]
                if !timer_running { self.preroll_video(); }
                return self.refresh_media_background();
            }

            Message::OpenSettings => {
                self.s_work_min = (self.config.work_time / 60).to_string();
                self.s_break_min = (self.config.break_time / 60).to_string();
                self.s_wallpaper_path = self.config.wallpaper_path.clone();
                self.s_wallpaper_type = self.config.wallpaper_type;
                self.s_music_enabled = self.config.music_enabled;
                self.s_main_music_dir = self.config.main_music_dir.clone();
                self.s_break_music_dir = self.config.break_music_dir.clone();
                self.s_accent_hex = to_hex_color(self.config.accent_color);
                self.s_break_hex = to_hex_color(self.config.break_color);
                self.s_timer_font = self.config.timer_font;
                self.s_custom_font_name = self.config.custom_font_name.clone();
                self.s_mode_font_size_scale = self.config.mode_font_size_scale.to_string();
                self.s_mode_font_color_hex = to_hex_color(self.config.mode_font_color);
                self.s_mode_font_opacity = self.config.mode_font_opacity.to_string();
                self.s_blur_intensity = self.config.blur_intensity.to_string();
                self.s_font_size_scale = self.config.font_size_scale.to_string();
                self.s_ring_thickness_scale = self.config.ring_thickness_scale.to_string();
                self.s_ring_bg_opacity = self.config.ring_bg_opacity.to_string();
                self.s_bg_tint_hex = to_hex_color(self.config.bg_tint);
                self.s_bg_tint_strength = self.config.bg_tint_strength.to_string();
                self.s_countdown_path = self.config.countdown_sound_path.clone();
                self.s_bell_path = self.config.bell_sound_path.clone();
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
            Message::MainMusicDirInput(s) => self.s_main_music_dir = s,
            Message::BreakMusicDirInput(s) => self.s_break_music_dir = s,
            Message::AccentColorInput(s) => self.s_accent_hex = s,
            Message::BreakColorInput(s) => self.s_break_hex = s,
            Message::TimerFontSelected(f) => self.s_timer_font = f,

            Message::PickMainMusicDir => {
                return Task::perform(
                    async {
                        rfd::AsyncFileDialog::new()
                            .set_title("Pick Work Music Folder")
                            .pick_folder()
                            .await
                            .map(|h| h.path().to_string_lossy().into_owned())
                    },
                    Message::MainMusicDirPicked,
                );
            }

            Message::PickBreakMusicDir => {
                return Task::perform(
                    async {
                        rfd::AsyncFileDialog::new()
                            .set_title("Pick Break Music Folder")
                            .pick_folder()
                            .await
                            .map(|h| h.path().to_string_lossy().into_owned())
                    },
                    Message::BreakMusicDirPicked,
                );
            }

            Message::PickWallpaperFile => {
                return Task::perform(
                    async {
                        rfd::AsyncFileDialog::new()
                            .set_title("Pick Wallpaper Image")
                            .add_filter("Images", &["jpg", "jpeg", "png", "webp", "bmp"])
                            .pick_file()
                            .await
                            .map(|h| h.path().to_string_lossy().into_owned())
                    },
                    Message::WallpaperFilePicked,
                );
            }

            Message::MainMusicDirPicked(p) => {
                if let Some(path) = p {
                    self.s_main_music_dir = path;
                }
            }
            Message::BreakMusicDirPicked(p) => {
                if let Some(path) = p {
                    self.s_break_music_dir = path;
                }
            }
            Message::WallpaperFilePicked(p) => {
                if let Some(path) = p {
                    self.s_wallpaper_path = path;
                }
            }

            Message::PickFontFile => {
                return Task::perform(
                    async {
                        rfd::AsyncFileDialog::new()
                            .set_title("Pick Timer Font")
                            .add_filter("Fonts", &["ttf", "otf"])
                            .pick_file()
                            .await
                            .map(|h| h.path().to_string_lossy().into_owned())
                    },
                    Message::FontFilePicked,
                );
            }

            Message::FontFilePicked(p) => {
                if let Some(path) = p {
                    // Extract the family name from the TTF/OTF to pre-fill the name field.
                    if let Some(name) = extract_font_name_from_file(&path) {
                        self.s_custom_font_name = name;
                    }
                }
            }

            Message::CustomFontNameInput(s) => self.s_custom_font_name = s,
            Message::ModeFontSizeScaleInput(s) => self.s_mode_font_size_scale = s,
            Message::ModeFontColorInput(s) => self.s_mode_font_color_hex = s,
            Message::ModeFontOpacityInput(s) => self.s_mode_font_opacity = s,
            Message::SkipSession => return self.finish_session(),
            Message::WindowResized(size) => self.window_size = size,
            Message::BlurIntensityInput(s) => self.s_blur_intensity = s,
            Message::FontSizeScaleInput(s) => self.s_font_size_scale = s,
            Message::RingThicknessInput(s) => self.s_ring_thickness_scale = s,
            Message::RingBgOpacityInput(s) => self.s_ring_bg_opacity = s,
            Message::BgTintHexInput(s) => self.s_bg_tint_hex = s,
            Message::BgTintStrengthInput(s) => self.s_bg_tint_strength = s,
            Message::CountdownPathInput(s) => self.s_countdown_path = s,
            Message::BellPathInput(s) => self.s_bell_path = s,

            Message::PickCountdownFile => {
                return Task::perform(
                    async {
                        rfd::AsyncFileDialog::new()
                            .set_title("Pick Countdown Sound")
                            .add_filter("Audio", &["mp3", "wav", "ogg", "flac", "aac", "m4a"])
                            .pick_file()
                            .await
                            .map(|h| h.path().to_string_lossy().into_owned())
                    },
                    Message::CountdownFilePicked,
                );
            }
            Message::PickBellFile => {
                return Task::perform(
                    async {
                        rfd::AsyncFileDialog::new()
                            .set_title("Pick Bell Sound")
                            .add_filter("Audio", &["mp3", "wav", "ogg", "flac", "aac", "m4a"])
                            .pick_file()
                            .await
                            .map(|h| h.path().to_string_lossy().into_owned())
                    },
                    Message::BellFilePicked,
                );
            }
            Message::CountdownFilePicked(p) => {
                if let Some(path) = p { self.s_countdown_path = path; }
            }
            Message::BellFilePicked(p) => {
                if let Some(path) = p { self.s_bell_path = path; }
            }

            Message::SettingsSave => {
                let work_time = self.s_work_min.parse::<u64>().unwrap_or(25).max(1) * 60;
                let break_time = self.s_break_min.parse::<u64>().unwrap_or(5).max(1) * 60;

                self.config.work_time = work_time;
                self.config.break_time = break_time;
                self.config.wallpaper_path = self.s_wallpaper_path.clone();
                self.config.wallpaper_type = self.s_wallpaper_type;
                self.config.music_enabled = self.s_music_enabled;
                self.music_enabled = self.s_music_enabled;

                if !self.s_main_music_dir.is_empty() {
                    self.config.main_music_dir = self.s_main_music_dir.clone();
                }
                if !self.s_break_music_dir.is_empty() {
                    self.config.break_music_dir = self.s_break_music_dir.clone();
                }
                if let Some(c) = parse_hex_color(&self.s_accent_hex) {
                    self.config.accent_color = c;
                }
                if let Some(c) = parse_hex_color(&self.s_break_hex) {
                    self.config.break_color = c;
                }
                self.config.timer_font = self.s_timer_font;
                self.config.custom_font_name = self.s_custom_font_name.clone();
                if let Ok(v) = self.s_mode_font_size_scale.parse::<f32>() {
                    self.config.mode_font_size_scale = v.clamp(0.3, 3.0);
                }
                if let Some(c) = parse_hex_color(&self.s_mode_font_color_hex) {
                    self.config.mode_font_color = c;
                }
                if let Ok(v) = self.s_mode_font_opacity.parse::<f32>() {
                    self.config.mode_font_opacity = v.clamp(0.0, 1.0);
                }
                if let Ok(v) = self.s_blur_intensity.parse::<f32>() {
                    self.config.blur_intensity = v.clamp(1.0, 100.0);
                }
                if let Ok(v) = self.s_font_size_scale.parse::<f32>() {
                    self.config.font_size_scale = v.clamp(0.3, 3.0);
                }
                if let Ok(v) = self.s_ring_thickness_scale.parse::<f32>() {
                    self.config.ring_thickness_scale = v.clamp(0.1, 5.0);
                }
                if let Ok(v) = self.s_ring_bg_opacity.parse::<f32>() {
                    self.config.ring_bg_opacity = v.clamp(0.0, 1.0);
                }
                if let Some(c) = parse_hex_color(&self.s_bg_tint_hex) {
                    self.config.bg_tint = c;
                }
                if let Ok(v) = self.s_bg_tint_strength.parse::<f32>() {
                    self.config.bg_tint_strength = v.clamp(0.0, 1.0);
                }
                self.config.countdown_sound_path = self.s_countdown_path.clone();
                self.config.bell_sound_path = self.s_bell_path.clone();

                // Validate and apply custom font family name.
                if self.s_timer_font == TimerFont::Custom && !self.s_custom_font_name.is_empty() {
                    self.custom_font_name = Some(leak_font_name(&self.s_custom_font_name));
                } else if self.s_timer_font != TimerFont::Custom {
                    self.custom_font_name = None;
                }

                self.main_music = MusicPlayer::new();
                self.break_music = MusicPlayer::new();
                self.main_music
                    .load_dir(std::path::Path::new(&self.config.main_music_dir));
                self.break_music
                    .load_dir(std::path::Path::new(&self.config.break_music_dir));

                self.timer = Timer::new(work_time);
                self.break_timer = Timer::new(break_time);
                self.in_break = false;
                #[cfg(feature = "video")]
                { self.video_wallpaper = None; self.video_preroll = None; }
                self.thumbnail_wallpaper = None;
                self.thumb_cache.clear();
                self.thumb_pending.clear();
                #[cfg(feature = "video")]
                self.preroll_video();

                self.blurred_wallpaper = if self.config.wallpaper_type == WallpaperType::Static {
                    load_blurred_image(&self.config.wallpaper_path, self.config.blur_intensity)
                } else {
                    None
                };

                self.config.save();
                self.view = AppView::Main;
            }

            Message::ThumbnailCached(path, handle) => {
                self.thumb_pending.remove(&path);
                let cur = if self.in_break {
                    self.break_music.current_file_path()
                } else {
                    self.main_music.current_file_path()
                };
                if cur == Some(path.as_path()) {
                    self.thumbnail_wallpaper = handle.clone();
                }
                // Always cache — including None (no art) to avoid re-decoding.
                self.thumb_cache.insert(path, handle);
            }

            #[cfg(feature = "video")]
            Message::VideoNewFrame => {}

            #[cfg(feature = "video")]
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

    fn switch_session(&mut self) -> Task<Message> {
        self.arc_fill = 0.0;
        self.arc_elapsed = 0.0;
        #[cfg(feature = "video")]
        { self.video_wallpaper = None; }
        self.thumbnail_wallpaper = None;
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
        let is_video = if self.in_break {
            self.break_music.current_file_is_video()
        } else {
            self.main_music.current_file_is_video()
        };
        #[cfg(feature = "video")]
        if is_video {
            let cur_path = if self.in_break {
                self.break_music.current_file_path()
            } else {
                self.main_music.current_file_path()
            }.map(|p| p.to_path_buf());
            let preroll_valid = self.video_preroll.as_ref()
                .zip(cur_path.as_ref())
                .map(|((stored, _), cur)| stored == cur)
                .unwrap_or(false);
            if preroll_valid {
                if let Some((_, mut video)) = self.video_preroll.take() {
                    debug!("switch_session: using pre-buffered video");
                    video.set_paused(false);
                    self.video_wallpaper = Some(video);
                }
            } else {
                self.video_preroll = None;
                self.update_video_wallpaper();
            }
            return Task::none();
        }
        #[cfg(not(feature = "video"))]
        let _ = is_video;
        if self.music_enabled {
            self.play_current_music();
        }
        self.update_thumbnail_wallpaper()
    }

    fn start_countdown_phase(&mut self) -> Task<Message> {
        debug!("starting countdown phase");
        self.pause_current_music();
        self.countdown_player.play(&self.config.countdown_sound_path.clone());
        self.awaiting_countdown = true;
        Task::none()
    }

    fn finish_session(&mut self) -> Task<Message> {
        let task = self.switch_session();
        self.bell_player.play(&self.config.bell_sound_path.clone());
        task
    }

    fn play_current_music(&mut self) {
        let is_video = if self.in_break {
            self.break_music.current_file_is_video()
        } else {
            self.main_music.current_file_is_video()
        };

        if is_video {
            #[cfg(feature = "video")]
            self.update_video_wallpaper();
        } else {
            #[cfg(feature = "video")]
            { self.video_wallpaper = None; }
            if self.in_break {
                self.break_music.play();
            } else {
                self.main_music.play();
            }
        }
    }

    fn pause_current_music(&mut self) {
        #[cfg(feature = "video")]
        if let Some(ref mut video) = self.video_wallpaper {
            video.set_paused(true);
            return;
        }
        if self.in_break {
            self.break_music.pause();
        } else {
            self.main_music.pause();
        }
    }

    #[cfg(feature = "video")]
    fn update_video_wallpaper(&mut self) {
        let path = if self.in_break {
            self.break_music.current_file_path()
        } else {
            self.main_music.current_file_path()
        };

        debug!("update_video_wallpaper: path={:?}", path);
        if let Some(p) = path {
            match Url::from_file_path(p) {
                Ok(url) => {
                    debug!("opening video: {}", url);
                    if let Some(mut video) = open_video_blurred(&url, self.config.blur_intensity) {
                        video.set_looping(true);
                        video.set_paused(false);
                        self.video_wallpaper = Some(video);
                        return;
                    }
                    debug!("open_video_blurred returned None");
                }
                Err(_) => debug!("Url::from_file_path failed for {:?}", p),
            }
        }
        self.video_wallpaper = None;
    }

    // Open the current video track in PAUSED state so GStreamer pre-rolls it.
    // Stores (path, video) so callers can validate the track hasn't changed.
    #[cfg(feature = "video")]
    fn preroll_video(&mut self) {
        let is_video = if self.in_break {
            self.break_music.current_file_is_video()
        } else {
            self.main_music.current_file_is_video()
        };
        if !is_video {
            self.video_preroll = None;
            return;
        }
        let path = if self.in_break {
            self.break_music.current_file_path()
        } else {
            self.main_music.current_file_path()
        };
        let Some(p) = path else {
            self.video_preroll = None;
            return;
        };
        let p = p.to_path_buf();
        debug!("preroll_video: opening {:?}", p);
        if let Ok(url) = Url::from_file_path(&p) {
            if let Some(mut video) = open_video_blurred(&url, self.config.blur_intensity) {
                video.set_looping(true);
                video.set_paused(true);
                self.video_preroll = Some((p, video));
                return;
            }
        }
        self.video_preroll = None;
    }

    // Check cache for current track; if missing spawn decode + preemptively decode next.
    fn update_thumbnail_wallpaper(&mut self) -> Task<Message> {
        if self.in_break {
            if self.break_music.current_file_is_video() {
                self.thumbnail_wallpaper = None;
                return Task::none();
            }
        } else if self.main_music.current_file_is_video() {
            self.thumbnail_wallpaper = None;
            return Task::none();
        }

        let player = if self.in_break { &self.break_music } else { &self.main_music };
        let cur = player.current_file_path().map(|p| p.to_path_buf());
        let nxt = player.next_file_path().map(|p| p.to_path_buf());

        let Some(cur_path) = cur else {
            self.thumbnail_wallpaper = None;
            return Task::none();
        };

        // Cache hit (including None = confirmed-no-art).
        if let Some(entry) = self.thumb_cache.get(&cur_path) {
            self.thumbnail_wallpaper = entry.clone();
            return self.prewarm_next(nxt);
        }

        // Decode already in flight — nothing to do.
        if self.thumb_pending.contains(&cur_path) {
            return Task::none();
        }

        // Cache miss — dispatch decode for current and optionally pre-warm next.
        debug!("cache miss, decoding {:?}", cur_path);
        self.thumb_pending.insert(cur_path.clone());
        let bi = self.config.blur_intensity;
        let mut tasks = vec![spawn_thumb_task(cur_path, bi)];

        if let Some(next_path) = nxt {
            if !self.thumb_cache.contains_key(&next_path)
                && !self.thumb_pending.contains(&next_path)
                && !crate::music::is_video_path(&next_path)
            {
                debug!("pre-warming next thumbnail {:?}", next_path);
                self.thumb_pending.insert(next_path.clone());
                tasks.push(spawn_thumb_task(next_path, bi));
            }
        }
        Task::batch(tasks)
    }

    fn prewarm_next(&mut self, nxt: Option<PathBuf>) -> Task<Message> {
        let Some(next_path) = nxt else { return Task::none(); };
        if self.thumb_cache.contains_key(&next_path)
            || self.thumb_pending.contains(&next_path)
            || crate::music::is_video_path(&next_path)
        {
            return Task::none();
        }
        debug!("pre-warming next thumbnail {:?}", next_path);
        self.thumb_pending.insert(next_path.clone());
        spawn_thumb_task(next_path, self.config.blur_intensity)
    }

    // Update visual background after a track change (video or audio).
    fn refresh_media_background(&mut self) -> Task<Message> {
        let is_video = if self.in_break {
            self.break_music.current_file_is_video()
        } else {
            self.main_music.current_file_is_video()
        };
        if is_video {
            self.thumbnail_wallpaper = None;
            #[cfg(feature = "video")]
            self.update_video_wallpaper();
            Task::none()
        } else {
            #[cfg(feature = "video")]
            { self.video_wallpaper = None; }
            self.update_thumbnail_wallpaper()
        }
    }

    // ── Views ────────────────────────────────────────────────────────────────

    pub fn view(&self) -> Element<'_, Message> {
        match self.view {
            AppView::Main => self.main_view(),
            AppView::Settings => self.settings_view(),
        }
    }

    fn main_view(&self) -> Element<'_, Message> {
        let remaining = if self.in_break {
            self.break_timer.check_time()
        } else {
            self.timer.check_time()
        };

        // Canvas fills the whole window; circle scales to fit
        let [mr, mg, mb] = self.config.mode_font_color;
        let timer_canvas: Element<Message> = canvas(TimerCanvas {
            arc_fill: self.arc_fill,
            arc_elapsed: self.arc_elapsed,
            remaining,
            is_break: self.in_break,
            accent_color: arr_to_color(self.config.accent_color),
            break_color: arr_to_color(self.config.break_color),
            font: timer_font_to_iced(self.config.timer_font, self.custom_font_name),
            font_size_scale: self.config.font_size_scale,
            mode_font_size_scale: self.config.mode_font_size_scale,
            mode_font_color: Color::from_rgba(mr, mg, mb, self.config.mode_font_opacity),
            ring_thickness_scale: self.config.ring_thickness_scale,
            ring_bg_opacity: self.config.ring_bg_opacity,
        })
        .width(Fill)
        .height(Fill)
        .into();

        // Controls — centered below the time display, fade in/out on mouse activity.
        // Compute the exact same radius the canvas uses so ctrl_pad_top is precise.
        // With align_y(Center)+padding P, controls center = (H+P)/2.
        // Timer center = OVERLAY_TOP + avail_h/2 = (H - OVERLAY_BOTTOM + OVERLAY_TOP)/2.
        // ctrl_distance = (P + OVERLAY_BOTTOM - OVERLAY_TOP) / 2.
        // Setting ctrl_distance = label_gap: P = 2*label_gap - (OVERLAY_BOTTOM - OVERLAY_TOP).
        let ca = self.controls_opacity;
        let mode_scale = self.config.mode_font_size_scale.clamp(0.3, 3.0);
        let font_scale = self.config.font_size_scale.clamp(0.3, 3.0);
        let avail_h = (self.window_size.height - OVERLAY_TOP - OVERLAY_BOTTOM).max(1.0);
        let avail_w = (self.window_size.width - 2.0 * OVERLAY_SIDE).max(1.0);
        let radius = (avail_h.min(avail_w) / 2.0 - 8.0).max(10.0);
        let label_gap = radius * (0.10 * font_scale + 0.10 * mode_scale + 0.08);
        let ctrl_pad_top = (2.0 * label_gap - (OVERLAY_BOTTOM - OVERLAY_TOP)).max(20.0);
        let btn_w = 80.0_f32;

        let current_running = if self.in_break {
            self.break_timer.is_running()
        } else {
            self.timer.is_running()
        };
        let (toggle_label, toggle_msg) = if current_running {
            ("Pause", Message::TimerPause)
        } else {
            ("Start", Message::TimerStart)
        };

        let centered_label = |s: &'static str| text(s).width(Fill).align_x(Horizontal::Center);
        let controls_overlay: Element<Message> = if ca > 0.01 {
            container(
                row![
                    button(centered_label(toggle_label))
                        .on_press(toggle_msg)
                        .style(ghost_button_faded(ca))
                        .width(btn_w)
                        .padding([10, 0]),
                    button(centered_label("Skip"))
                        .on_press(Message::SkipSession)
                        .style(ghost_button_faded(ca))
                        .width(btn_w)
                        .padding([10, 0]),
                    button(centered_label("Reset"))
                        .on_press(Message::TimerReset)
                        .style(ghost_button_faded(ca))
                        .width(btn_w)
                        .padding([10, 0]),
                ]
                .spacing(10),
            )
            .width(Fill)
            .height(Fill)
            .align_x(Horizontal::Center)
            .align_y(Vertical::Center)
            .padding(Padding { top: ctrl_pad_top, right: 0.0, bottom: 0.0, left: 0.0 })
            .into()
        } else {
            Space::new().width(Fill).height(Fill).into()
        };

        // Music controls — stable layout, opacity animated by music_progress.
        let mp = self.music_progress;
        let track_name = {
            let player = if self.in_break { &self.break_music } else { &self.main_music };
            if let Some(p) = player.current_file_path() {
                let stem = p.file_stem().and_then(|s| s.to_str()).unwrap_or("—");
                let truncated = if stem.len() > 24 {
                    format!("{}…", &stem[..24])
                } else {
                    stem.to_string()
                };
                format!("♪ {}", truncated)
            } else if self.music_enabled {
                "♪ ON".to_string()
            } else {
                "♪ OFF".to_string()
            }
        };
        let half_name = TRACK_NAME_MAX_W / 2.0;
        let music_row = row![
            button(crate::icon::skip_back().size(16))
                .on_press(Message::PrevTrack)
                .style(ghost_icon_button(mp))
                .padding([6, 10]),
            Space::new().width(half_name),
            text(track_name)
                .size(14)
                .width(TRACK_NAME_MAX_W)
                .align_x(Horizontal::Center)
                .color(Color::from_rgba(1.0, 1.0, 1.0, mp)),
            Space::new().width(half_name),
            button(crate::icon::skip_forward().size(16))
                .on_press(Message::NextTrack)
                .style(ghost_icon_button(mp))
                .padding([6, 10]),
        ]
        .align_y(Vertical::Center);

        let music_area = mouse_area(container(music_row).padding(Padding { top: 6.0, right: 8.0, bottom: 6.0, left: 8.0 }))
            .on_enter(Message::MusicAreaEntered)
            .on_exit(Message::MusicAreaExited);

        // Cog — top-right, fades to resting opacity when not hovered.
        let cog_btn = mouse_area(
            button(crate::icon::settings().size(18))
                .on_press(Message::OpenSettings)
                .style(ghost_icon_button(self.cog_progress))
                .padding([6, 10]),
        )
        .on_enter(Message::CogEntered)
        .on_exit(Message::CogExited);

        // Track progress bar — full width, flush at the very bottom.
        let bar_color = if self.in_break {
            arr_to_color(self.config.break_color)
        } else {
            arr_to_color(self.config.accent_color)
        };
        let track_bar: Element<Message> = canvas(TrackBar {
            progress: self.arc_elapsed,
            color: bar_color,
        })
        .width(Fill)
        .height(TRACK_BAR_H)
        .into();

        let ui_overlay: Element<Message> = column![
            row![Space::new().width(Fill), cog_btn]
                .padding(Padding { top: 12.0, right: 12.0, bottom: 0.0, left: 12.0 }),
            Space::new().width(Fill).height(Fill),
            container(music_area)
                .width(Fill)
                .align_x(Horizontal::Center)
                .padding(Padding { top: 0.0, right: 0.0, bottom: 10.0, left: 0.0 }),
            track_bar,
        ]
        .width(Fill)
        .height(Fill)
        .into();

        // Background priority: Video > Thumbnail > Wallpaper > plain
        let bg_image = self
            .thumbnail_wallpaper
            .as_ref()
            .or(self.blurred_wallpaper.as_ref());

        let bg_tint_color = arr_to_color(self.config.bg_tint);
        let bg_tint_strength = self.config.bg_tint_strength;

        #[cfg(feature = "video")]
        if let Some(ref video) = self.video_wallpaper {
            let video_bg = iced_video_player::VideoPlayer::new(video)
                .width(Fill)
                .height(Fill)
                .content_fit(iced::ContentFit::Cover)
                .on_end_of_stream(Message::VideoEndOfStream)
                .on_new_frame(Message::VideoNewFrame);
            return stack![
                video_bg,
                tint_overlay(bg_tint_color, bg_tint_strength),
                timer_canvas,
                controls_overlay,
                ui_overlay,
            ]
            .width(Fill)
            .height(Fill)
            .into();
        }

        if let Some(handle) = bg_image {
            let bg_img = image(handle.clone())
                .width(Fill)
                .height(Fill)
                .content_fit(ContentFit::Cover);
            stack![
                bg_img,
                tint_overlay(bg_tint_color, bg_tint_strength),
                timer_canvas,
                controls_overlay,
                ui_overlay,
            ]
            .width(Fill)
            .height(Fill)
            .into()
        } else {
            stack![timer_canvas, controls_overlay, ui_overlay]
                .width(Fill)
                .height(Fill)
                .into()
        }
    }

    fn settings_view(&self) -> Element<'_, Message> {
        let title = text("Settings").size(26);

        let duration_section = column![
            section_label("Timer"),
            labeled_row(
                "Work (minutes)",
                text_input("25", &self.s_work_min)
                    .on_input(Message::WorkTimeInput)
                    .width(80)
                    .into(),
            ),
            labeled_row(
                "Break (minutes)",
                text_input("5", &self.s_break_min)
                    .on_input(Message::BreakTimeInput)
                    .width(80)
                    .into(),
            ),
        ]
        .spacing(10);

        let music_section = column![
            section_label("Music"),
            labeled_row(
                "Enabled",
                button(if self.s_music_enabled { "ON" } else { "OFF" })
                    .on_press(Message::MusicEnabledToggled(!self.s_music_enabled))
                    .style(ghost_button)
                    .padding([5, 14])
                    .into(),
            ),
            dir_picker_row(
                "Work music folder",
                &self.s_main_music_dir,
                Message::MainMusicDirInput,
                Message::PickMainMusicDir,
            ),
            dir_picker_row(
                "Break music folder",
                &self.s_break_music_dir,
                Message::BreakMusicDirInput,
                Message::PickBreakMusicDir,
            ),
            text(
                "Video files (mp4, mkv…) in the folder play as wallpaper + audio. \
                 Audio files with embedded artwork show it as background."
            )
            .size(12)
            .color(Color::from_rgba(0.6, 0.6, 0.6, 1.0)),
        ]
        .spacing(10);

        let accent_preview = color_swatch(
            parse_hex_color(&self.s_accent_hex)
                .map(arr_to_color)
                .unwrap_or_else(|| arr_to_color(self.config.accent_color)),
        );
        let break_preview = color_swatch(
            parse_hex_color(&self.s_break_hex)
                .map(arr_to_color)
                .unwrap_or_else(|| arr_to_color(self.config.break_color)),
        );
        let bg_tint_preview = color_swatch(
            parse_hex_color(&self.s_bg_tint_hex)
                .map(arr_to_color)
                .unwrap_or_else(|| arr_to_color(self.config.bg_tint)),
        );

        let colors_section = column![
            section_label("Colors  (hex, no #)"),
            row![
                text("Work arc").size(14).width(110),
                text_input("f27a1e", &self.s_accent_hex)
                    .on_input(Message::AccentColorInput)
                    .width(90),
                accent_preview,
            ]
            .spacing(8)
            .align_y(Vertical::Center),
            row![
                text("Break arc").size(14).width(110),
                text_input("40d973", &self.s_break_hex)
                    .on_input(Message::BreakColorInput)
                    .width(90),
                break_preview,
            ]
            .spacing(8)
            .align_y(Vertical::Center),
            row![
                text("BG tint").size(14).width(110),
                text_input("000000", &self.s_bg_tint_hex)
                    .on_input(Message::BgTintHexInput)
                    .width(90),
                bg_tint_preview,
            ]
            .spacing(8)
            .align_y(Vertical::Center),
            labeled_row(
                "Tint strength  (0–1)",
                text_input("0.47", &self.s_bg_tint_strength)
                    .on_input(Message::BgTintStrengthInput)
                    .width(80)
                    .into(),
            ),
        ]
        .spacing(10);

        let font_picker = pick_list(
            TimerFont::all(),
            Some(self.s_timer_font),
            Message::TimerFontSelected,
        );
        let custom_font_row: Element<_> = if self.s_timer_font == TimerFont::Custom {
            column![
                text("Font family name (must be system-installed)").size(12)
                    .color(Color::from_rgba(0.6, 0.6, 0.6, 1.0)),
                row![
                    text_input("e.g. JetBrains Mono", &self.s_custom_font_name)
                        .on_input(Message::CustomFontNameInput)
                        .width(Fill),
                    button("Browse TTF…")
                        .on_press(Message::PickFontFile)
                        .style(ghost_button)
                        .padding([6, 12]),
                ]
                .spacing(8),
            ]
            .spacing(4)
            .into()
        } else {
            Space::new().into()
        };

        let mode_font_color_preview = color_swatch(
            parse_hex_color(&self.s_mode_font_color_hex)
                .map(arr_to_color)
                .unwrap_or_else(|| arr_to_color(self.config.mode_font_color)),
        );
        let font_section = column![
            section_label("Timer font"),
            labeled_row("Font", font_picker.into()),
            custom_font_row,
            labeled_row(
                "Font size scale  (0.3–3.0)",
                text_input("1.0", &self.s_font_size_scale)
                    .on_input(Message::FontSizeScaleInput)
                    .width(80)
                    .into(),
            ),
            labeled_row(
                "Label size scale  (0.3–3.0)",
                text_input("1.0", &self.s_mode_font_size_scale)
                    .on_input(Message::ModeFontSizeScaleInput)
                    .width(80)
                    .into(),
            ),
            row![
                text("Label color").size(14).width(180),
                text_input("ffffff", &self.s_mode_font_color_hex)
                    .on_input(Message::ModeFontColorInput)
                    .width(90),
                mode_font_color_preview,
            ]
            .spacing(8)
            .align_y(Vertical::Center),
            labeled_row(
                "Label opacity  (0–1)",
                text_input("0.55", &self.s_mode_font_opacity)
                    .on_input(Message::ModeFontOpacityInput)
                    .width(80)
                    .into(),
            ),
        ]
        .spacing(8);

        let ring_section = column![
            section_label("Ring"),
            labeled_row(
                "Thickness scale  (0.1–5)",
                text_input("1.0", &self.s_ring_thickness_scale)
                    .on_input(Message::RingThicknessInput)
                    .width(80)
                    .into(),
            ),
            labeled_row(
                "Track opacity  (0–1)",
                text_input("0.1", &self.s_ring_bg_opacity)
                    .on_input(Message::RingBgOpacityInput)
                    .width(80)
                    .into(),
            ),
        ]
        .spacing(8);

        let blur_section = column![
            section_label("Blur intensity  (Gaussian σ, 1–100)"),
            labeled_row(
                "Blur strength",
                text_input("18", &self.s_blur_intensity)
                    .on_input(Message::BlurIntensityInput)
                    .width(80)
                    .into(),
            ),
            text("Applied to static wallpaper, album art, and video background passes.")
                .size(12)
                .color(Color::from_rgba(0.6, 0.6, 0.6, 1.0)),
        ]
        .spacing(8);

        let sounds_section = column![
            section_label("Sounds"),
            dir_picker_row(
                "Countdown sound (plays before bell)",
                &self.s_countdown_path,
                Message::CountdownPathInput,
                Message::PickCountdownFile,
            ),
            dir_picker_row(
                "Bell sound (plays after mode switch)",
                &self.s_bell_path,
                Message::BellPathInput,
                Message::PickBellFile,
            ),
            text("Leave empty to disable. Countdown plays in full before switching modes.")
                .size(12)
                .color(Color::from_rgba(0.6, 0.6, 0.6, 1.0)),
        ]
        .spacing(10);

        let wp_section = column![
            section_label("Wallpaper (fallback)"),
            row![
                radio(
                    "None",
                    WallpaperType::None,
                    Some(self.s_wallpaper_type),
                    Message::WallpaperTypeSelected
                ),
                radio(
                    "Static image",
                    WallpaperType::Static,
                    Some(self.s_wallpaper_type),
                    Message::WallpaperTypeSelected
                ),
            ]
            .spacing(16),
        ]
        .spacing(8);

        let wp_path: Element<Message> = if self.s_wallpaper_type == WallpaperType::Static {
            column![
                text("Image path").size(13),
                row![
                    text_input("/path/to/image.jpg", &self.s_wallpaper_path)
                        .on_input(Message::WallpaperPathInput)
                        .width(Fill),
                    button("Browse…")
                        .on_press(Message::PickWallpaperFile)
                        .style(ghost_button)
                        .padding([6, 12]),
                ]
                .spacing(8),
            ]
            .spacing(4)
            .into()
        } else {
            Space::new().into()
        };

        let save_row = row![
            button("Save")
                .on_press(Message::SettingsSave)
                .style(ghost_button)
                .padding([10, 24]),
            button("Cancel")
                .on_press(Message::CloseSettings)
                .style(ghost_button)
                .padding([10, 24]),
        ]
        .spacing(12);

        let form = column![
            title,
            duration_section,
            music_section,
            colors_section,
            font_section,
            ring_section,
            blur_section,
            sounds_section,
            wp_section,
            wp_path,
            save_row,
        ]
        .spacing(22)
        .padding(36)
        .max_width(520);

        container(scrollable(form))
            .width(Fill)
            .height(Fill)
            .align_x(Horizontal::Center)
            .into()
    }
}

// ── Settings helpers ──────────────────────────────────────────────────────────

fn section_label(label: &str) -> text::Text<'_> {
    text(label).size(13).color(Color::from_rgba(0.5, 0.5, 0.5, 1.0))
}

fn labeled_row<'a>(label: &'a str, widget: Element<'a, Message>) -> Element<'a, Message> {
    row![text(label).size(14).width(180), widget]
        .spacing(10)
        .align_y(Vertical::Center)
        .into()
}

fn dir_picker_row<'a>(
    label: &'a str,
    value: &'a str,
    on_input: impl Fn(String) -> Message + 'a,
    on_pick: Message,
) -> Element<'a, Message> {
    column![
        text(label).size(13),
        row![
            text_input("", value).on_input(on_input).width(Fill),
            button("Browse…")
                .on_press(on_pick)
                .style(ghost_button)
                .padding([6, 12]),
        ]
        .spacing(8),
    ]
    .spacing(4)
    .into()
}

fn color_swatch(color: Color) -> Element<'static, Message> {
    container(Space::new())
        .width(24)
        .height(24)
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(color)),
            border: Border {
                radius: 4.0.into(),
                ..Default::default()
            },
            ..Default::default()
        })
        .into()
}

// ── Image helpers ─────────────────────────────────────────────────────────────

fn load_blurred_image(path: &str, intensity: f32) -> Option<image::Handle> {
    if path.is_empty() {
        return None;
    }
    let img = ::image::open(path).ok()?;
    let rgba = img.blur(intensity.max(1.0)).to_rgba8();
    let (w, h) = rgba.dimensions();
    Some(image::Handle::from_rgba(w, h, rgba.into_raw()))
}

/// Open a video with GPU Gaussian-approximation blur via chained gleffects passes.
/// `blur_intensity` controls the number of blur passes (≈ sigma² ∝ passes).
/// Falls back to the standard pipeline if the GL elements are unavailable.
#[cfg(feature = "video")]
fn open_video_blurred(url: &Url, blur_intensity: f32) -> Option<iced_video_player::Video> {
    use gstreamer as gst;
    use gstreamer_app as gst_app;
    use gst::prelude::*;

    let _ = gst::init();

    // Each gleffects pass applies a fixed-radius box-blur kernel. Multiple passes
    // approximate a Gaussian with σ ≈ √passes · kernel_σ. Clamp to a sane range.
    let passes = ((blur_intensity / 6.0).ceil() as usize).clamp(1, 12);
    let blur_chain: String = std::iter::repeat("gleffects effect=blur")
        .take(passes)
        .collect::<Vec<_>>()
        .join(" ! ");

    let blur_pipeline_str = format!(
        "playbin uri=\"{}\" \
         text-sink=\"appsink name=iced_text sync=true drop=true\" \
         video-sink=\"videoscale ! videoconvert ! glupload ! \
           {blur_chain} ! \
           gldownload ! videoconvert ! \
           appsink name=iced_video drop=true caps=video/x-raw,format=NV12,pixel-aspect-ratio=1/1\"",
        url.as_str()
    );

    let try_blurred = || -> Option<iced_video_player::Video> {
        let pipeline = match gst::parse::launch(&blur_pipeline_str) {
            Ok(p) => p,
            Err(e) => { debug!("GPU blur pipeline parse failed: {e}"); return None; }
        };
        let pipeline = match pipeline.downcast::<gst::Pipeline>() {
            Ok(p) => p,
            Err(_) => { debug!("GPU blur pipeline downcast failed"); return None; }
        };
        let video_sink_el: gst::Element = pipeline.property("video-sink");
        let pad = video_sink_el.pads().first().cloned()?;
        let pad = match pad.dynamic_cast::<gst::GhostPad>() {
            Ok(p) => p,
            Err(_) => { debug!("video-sink pad not a GhostPad"); return None; }
        };
        let bin = match pad.parent_element()?.downcast::<gst::Bin>() {
            Ok(b) => b,
            Err(_) => { debug!("video-sink parent is not a Bin"); return None; }
        };
        let video_sink = match bin.by_name("iced_video").and_then(|e| e.downcast::<gst_app::AppSink>().ok()) {
            Some(s) => s,
            None => { debug!("iced_video appsink not found in bin"); return None; }
        };
        let text_sink: gst::Element = pipeline.property("text-sink");
        let text_sink = text_sink.downcast::<gst_app::AppSink>().ok();
        match iced_video_player::Video::from_gst_pipeline(pipeline, video_sink, text_sink) {
            Ok(v) => { debug!("GPU blur video pipeline opened successfully"); Some(v) }
            Err(e) => { debug!("from_gst_pipeline failed: {e}"); None }
        }
    };

    try_blurred().or_else(|| {
        debug!("GPU blur pipeline unavailable, falling back to standard video pipeline");
        match iced_video_player::Video::new(url) {
            Ok(v) => { debug!("standard video pipeline opened successfully"); Some(v) }
            Err(e) => { debug!("standard video pipeline also failed: {e}"); None }
        }
    })
}

fn spawn_thumb_task(path: PathBuf, blur_intensity: f32) -> Task<Message> {
    let msg_path = path.clone();
    Task::perform(
        async move {
            tokio::task::spawn_blocking(move || extract_album_art(&path, blur_intensity))
                .await
                .ok()
                .flatten()
        },
        move |handle| Message::ThumbnailCached(msg_path.clone(), handle),
    )
}

fn extract_album_art(path: &std::path::Path, intensity: f32) -> Option<image::Handle> {
    use lofty::prelude::*;
    let tagged = lofty::read_from_path(path).ok()?;
    let tag = tagged.primary_tag().or_else(|| tagged.first_tag())?;
    let picture = tag.pictures().first()?;
    let img = ::image::load_from_memory(picture.data()).ok()?;
    let rgba = img.blur(intensity.max(1.0)).to_rgba8();
    let (w, h) = rgba.dimensions();
    Some(image::Handle::from_rgba(w, h, rgba.into_raw()))
}

// ── Timer canvas ──────────────────────────────────────────────────────────────

struct TimerCanvas {
    arc_fill: f32,    // CCW tip (fill-in animation, lerps to 1.0)
    arc_elapsed: f32, // elapsed fraction (direct, no lerp)
    remaining: f64,
    is_break: bool,
    accent_color: Color,
    break_color: Color,
    font: iced::Font,
    font_size_scale: f32,
    mode_font_size_scale: f32,
    mode_font_color: Color,
    ring_thickness_scale: f32,
    ring_bg_opacity: f32,
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

        let mut frame = Frame::new(renderer, bounds.size());

        // Size the circle to the space not occupied by the overlay controls.
        let avail_h = (bounds.height - OVERLAY_TOP - OVERLAY_BOTTOM).max(1.0);
        let avail_w = (bounds.width - 2.0 * OVERLAY_SIDE).max(1.0);
        let radius = (avail_h.min(avail_w) / 2.0 - 8.0).max(10.0);
        let center = iced::Point::new(
            bounds.width / 2.0,
            OVERLAY_TOP + avail_h / 2.0,
        );

        let scale = self.font_size_scale.clamp(0.3, 3.0);
        let mode_scale = self.mode_font_size_scale.clamp(0.3, 3.0);
        let ring_scale = self.ring_thickness_scale.clamp(0.1, 5.0);
        let stroke_w = (radius * 0.065 * ring_scale).max(1.0);
        let time_font = Pixels((radius * 0.28 * scale).max(12.0));
        let mode_font = Pixels((radius * 0.10 * mode_scale).max(8.0));
        // label_gap = sum of both text half-heights (cap ≈ 0.7 em) + breathing room.
        // This ensures the mode label clears the time text regardless of which scale changes.
        let label_gap = radius * (0.10 * scale + 0.10 * mode_scale + 0.08);

        // Background ring
        frame.stroke(
            &Path::circle(center, radius),
            Stroke::default()
                .with_width(stroke_w)
                .with_color(Color::from_rgba(1.0, 1.0, 1.0, self.ring_bg_opacity.clamp(0.0, 1.0))),
        );

        // Progress arc: CCW from arc_fill tip to arc_elapsed base
        // arc_fill lerps to 1.0 (start animation), arc_elapsed tracks real elapsed/total
        if self.arc_fill > self.arc_elapsed + 0.001 {
            let start = Radians(-FRAC_PI_2 - self.arc_fill * TAU);
            let end = Radians(-FRAC_PI_2 - self.arc_elapsed * TAU);
            let arc = Path::new(|b| {
                b.arc(Arc {
                    center,
                    radius,
                    start_angle: start,
                    end_angle: end,
                });
            });
            let color = if self.is_break {
                self.break_color
            } else {
                self.accent_color
            };
            frame.stroke(&arc, Stroke::default().with_width(stroke_w).with_color(color));
        }

        // Mode label above the time
        frame.fill_text(iced::widget::canvas::Text {
            content: if self.is_break { "Break".to_string() } else { "Focus".to_string() },
            position: iced::Point::new(center.x, center.y - label_gap),
            color: self.mode_font_color,
            size: mode_font,
            font: self.font,
            align_x: TextAlign::Center,
            align_y: Vertical::Center,
            ..Default::default()
        });

        // Time
        let mins = self.remaining as u64 / 60;
        let secs = self.remaining as u64 % 60;
        frame.fill_text(iced::widget::canvas::Text {
            content: format!("{:02}:{:02}", mins, secs),
            position: center,
            color: Color::WHITE,
            size: time_font,
            font: self.font,
            align_x: TextAlign::Center,
            align_y: Vertical::Center,
            ..Default::default()
        });

        vec![frame.into_geometry()]
    }
}

// ── Track progress bar ────────────────────────────────────────────────────────

struct TrackBar {
    progress: f32, // 0.0 – 1.0 (arc_elapsed, timer elapsed / total)
    color: Color,
}

impl Program<Message> for TrackBar {
    type State = ();

    fn draw(
        &self,
        _state: &(),
        renderer: &iced::Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: iced::mouse::Cursor,
    ) -> Vec<Geometry<iced::Renderer>> {
        use iced::Size;

        let mut frame = Frame::new(renderer, bounds.size());

        // Dim background track
        frame.fill_rectangle(
            iced::Point::ORIGIN,
            bounds.size(),
            Color::from_rgba(1.0, 1.0, 1.0, 0.12),
        );

        // Filled portion
        let filled_w = (bounds.width * self.progress.clamp(0.0, 1.0)).max(0.0);
        if filled_w > 0.0 {
            frame.fill_rectangle(
                iced::Point::ORIGIN,
                Size::new(filled_w, bounds.height),
                self.color,
            );
        }

        vec![frame.into_geometry()]
    }
}
