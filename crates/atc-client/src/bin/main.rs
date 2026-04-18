use atc_client::ui::AtcApp;

pub fn main() -> iced::Result {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    iced::application(AtcApp::title, AtcApp::update, AtcApp::view)
        .subscription(AtcApp::subscription)
        .theme(|_| iced::Theme::Dark)
        .run_with(AtcApp::new)
}
