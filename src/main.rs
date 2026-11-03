mod ui;

use ui::Soundboard;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let Some(_instance_lock) = raspiboard::storage::Store::in_home()?.try_lock()? else {
        eprintln!("Raspiboard is already running.");
        return Ok(());
    };

    iced::daemon(Soundboard::start, Soundboard::update, Soundboard::view)
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
        .run()?;
    Ok(())
}
