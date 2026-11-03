use super::{Message, sound_pad, volume_slider};
use iced::widget::{button, column, container, pick_list, row, scrollable, slider, space, text};
use iced::{Element, Fill};
use raspiboard::storage::{MAX_SOUND_BUTTON_SCALE, MIN_SOUND_BUTTON_SCALE};
use raspiboard::theme::ThemeChoice;

pub(super) fn view(theme: ThemeChoice, sound_button_scale: u16) -> Element<'static, Message> {
    let controls = column![
        column![
            text("Theme").size(14),
            pick_list(ThemeChoice::ALL, Some(theme), Message::ThemeChanged)
                .padding(12)
                .width(Fill),
        ]
        .spacing(8),
        text(match theme {
            ThemeChoice::Deck => "Near-black panels, graphite pads, and cyan illumination.",
            ThemeChoice::Console => "Charcoal panels with stronger metallic bevels.",
        })
        .size(13)
        .style(text::secondary),
        column![
            row![
                text("Sound button size").size(14),
                space::horizontal(),
                text(format!("{sound_button_scale}%")).size(14),
            ],
            slider(
                MIN_SOUND_BUTTON_SCALE..=MAX_SOUND_BUTTON_SCALE,
                sound_button_scale,
                Message::SoundButtonScaleChanged,
            )
            .step(5u16)
            .height(22)
            .width(Fill)
            .style(volume_slider::style),
        ]
        .spacing(8),
        text("Changes apply immediately.")
            .size(13)
            .style(text::secondary),
    ]
    .spacing(20);
    column![
        scrollable(container(controls).padding(iced::Padding {
            top: 28.0,
            right: 26.0,
            bottom: 4.0,
            left: 26.0,
        }))
        .spacing(0)
        .height(Fill),
        container(
            row![
                sound_pad::animated(
                    button(text("Use default").width(Fill).center())
                        .on_press_maybe(
                            (theme != ThemeChoice::Deck)
                                .then_some(Message::ThemeChanged(ThemeChoice::Deck))
                        )
                        .width(144)
                        .height(36)
                        .padding(8),
                    theme != ThemeChoice::Deck,
                    false
                ),
                space::horizontal(),
                sound_pad::animated(
                    button(text("Close").width(Fill).center())
                        .on_press(Message::CloseSettings)
                        .width(84)
                        .height(36)
                        .padding(8),
                    true,
                    false,
                ),
            ]
            .spacing(12)
        )
        .padding(iced::Padding {
            top: 20.0,
            right: 24.0,
            bottom: 24.0,
            left: 24.0,
        }),
    ]
    .width(Fill)
    .height(Fill)
    .into()
}
