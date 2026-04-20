mod app;
mod config;
mod music;
mod timer;

fn main() -> iced::Result {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("pomodoro_timer=debug".parse().unwrap()),
        )
        .init();
    check_env();
    iced::application(app::PomodoroApp::new, app::PomodoroApp::update, app::PomodoroApp::view)
        .title(app::PomodoroApp::title)
        .subscription(app::PomodoroApp::subscription)
        .theme(|_: &app::PomodoroApp| iced::Theme::Dark)
        .centered()
        .run()
}

fn check_env() {
    let base = dirs::home_dir().unwrap().join(".timer");
    let _ = std::fs::create_dir_all(base.join("music/main"));
    let _ = std::fs::create_dir_all(base.join("music/break"));
    let _ = std::fs::create_dir_all(base.join("music/fx"));
}
