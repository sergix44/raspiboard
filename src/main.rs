mod ui;

use ui::Soundboard;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let store = raspiboard::storage::Store::in_home()?;
    let Some(_instance_lock) = store.try_lock()? else {
        eprintln!("Raspiboard is already running.");
        return Ok(());
    };

    let logger = raspiboard::logging::init(&store.logs_directory())?;
    log::info!("Starting Raspiboard {}", env!("CARGO_PKG_VERSION"));
    let result = iced::daemon(Soundboard::start, Soundboard::update, Soundboard::view)
        .title(Soundboard::title)
        .default_font(ui::fonts::FONT)
        .font(ui::fonts::FONT_BYTES)
        .font(ui::icons::FONT_BYTES)
        .font(iced_aw::ICED_AW_FONT_BYTES)
        .theme(|board: &Soundboard, _window| board.theme())
        .style(
            |_board: &Soundboard, theme: &iced::Theme| iced::theme::Style {
                background_color: iced::Color::TRANSPARENT,
                text_color: theme.palette().text,
            },
        )
        .subscription(Soundboard::subscription)
        .run();
    match &result {
        Ok(()) => log::info!("Raspiboard stopped"),
        Err(error) => log::error!("Application failed: {error}"),
    }
    logger.shutdown();
    result.map_err(Into::into)
}
