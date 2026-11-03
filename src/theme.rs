use iced::{
    Color, Theme, color,
    theme::{Base, Palette, palette},
};
use serde::{Deserialize, Serialize};
use std::{fmt, sync::LazyLock};

#[derive(Default, Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ThemeChoice {
    #[default]
    Deck,
    Console,
}

impl ThemeChoice {
    pub const ALL: &'static [Self] = &[Self::Deck, Self::Console];

    pub fn iced(self) -> Theme {
        match self {
            Self::Deck => DECK_THEME.clone(),
            Self::Console => CONSOLE_THEME.clone(),
        }
    }
}

impl fmt::Display for ThemeChoice {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Deck => "Deck",
            Self::Console => "Console",
        })
    }
}

#[derive(Debug)]
pub struct Tokens {
    pub background: Color,
    pub surface: Color,
    pub raised: Color,
    pub border: Color,
    pub accent: Color,
    pub illuminated: Color,
    pub text: Color,
    pub muted: Color,
    pub radius: f32,
    pub metallic: bool,
}

const DECK: Tokens = Tokens {
    background: color!(0x0B0D10),
    surface: color!(0x191D22),
    raised: color!(0x282E35),
    border: color!(0x3A424C),
    accent: color!(0x24B9ED),
    illuminated: color!(0x16465A),
    text: color!(0xE8EDF1),
    muted: color!(0xADB7C1),
    radius: 6.0,
    metallic: false,
};

const CONSOLE: Tokens = Tokens {
    background: color!(0x111212),
    surface: color!(0x262727),
    raised: color!(0x3D3E3E),
    border: color!(0x5B5E60),
    accent: color!(0x24B9ED),
    illuminated: color!(0x20546A),
    text: color!(0xE8EDF1),
    muted: color!(0xB7BABB),
    radius: 3.0,
    metallic: true,
};

static DECK_THEME: LazyLock<Theme> = LazyLock::new(|| custom("Deck", &DECK));
static CONSOLE_THEME: LazyLock<Theme> = LazyLock::new(|| custom("Console", &CONSOLE));

pub fn tokens(theme: &Theme) -> &'static Tokens {
    if theme.name() == "Console" {
        &CONSOLE
    } else {
        &DECK
    }
}

fn custom(name: &'static str, tokens: &Tokens) -> Theme {
    let palette = Palette {
        background: tokens.background,
        text: tokens.text,
        primary: tokens.accent,
        success: color!(0x4BCC88),
        warning: color!(0xF1C75B),
        danger: color!(0xEF6B73),
    };
    Theme::custom_with_fn(name, palette, |palette| extended(palette, tokens))
}

fn extended(palette: Palette, tokens: &Tokens) -> palette::Extended {
    let dark_text = tokens.background;
    let pair = |color| palette::Pair {
        color,
        text: tokens.text,
    };
    let solid = |color| palette::Pair {
        color,
        text: dark_text,
    };
    palette::Extended {
        background: palette::Background {
            base: pair(tokens.background),
            weakest: pair(tokens.surface),
            weaker: pair(tokens.surface),
            weak: pair(tokens.surface),
            neutral: pair(tokens.raised),
            strong: pair(tokens.border),
            stronger: pair(tokens.border),
            strongest: palette::Pair {
                color: tokens.muted,
                text: dark_text,
            },
        },
        primary: palette::Primary {
            base: solid(palette.primary),
            weak: pair(tokens.illuminated),
            strong: solid(palette.primary),
        },
        secondary: palette::Secondary {
            // Native text inputs use this color for their placeholder.
            base: palette::Pair {
                color: tokens.muted,
                text: dark_text,
            },
            weak: pair(tokens.raised),
            strong: palette::Pair {
                color: tokens.text,
                text: dark_text,
            },
        },
        success: palette::Success {
            base: solid(palette.success),
            weak: pair(color!(0x174E36)),
            strong: solid(palette.success),
        },
        warning: palette::Warning {
            base: solid(palette.warning),
            weak: pair(color!(0x56471F)),
            strong: solid(palette.warning),
        },
        danger: palette::Danger {
            base: solid(palette.danger),
            weak: pair(color!(0x5B2930)),
            strong: solid(palette.danger),
        },
        is_dark: true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_text_inputs_and_menu_selection_keep_readable_contrast() {
        for choice in ThemeChoice::ALL {
            let theme = choice.iced();
            let palette = theme.extended_palette();
            for pair in [
                palette.background.base,
                palette.background.weak,
                palette.background.neutral,
                palette.background.strong,
                palette.primary.base,
                palette.primary.weak,
                palette.primary.strong,
                palette.secondary.base,
                palette.secondary.weak,
                palette.secondary.strong,
                palette.success.base,
                palette.warning.base,
                palette.danger.base,
            ] {
                assert!(
                    pair.text.relative_contrast(pair.color) >= 4.5,
                    "{choice}: unreadable text on {pair:?}"
                );
            }
            assert!(
                palette
                    .background
                    .base
                    .text
                    .relative_contrast(palette.primary.weak.color)
                    >= 4.5,
                "{choice}: selected input text must remain readable"
            );
            assert!(
                palette
                    .secondary
                    .base
                    .color
                    .relative_contrast(palette.background.base.color)
                    >= 4.5,
                "{choice}: placeholder must remain readable"
            );
        }
    }

    #[test]
    fn own_choices_select_distinct_surfaces_and_shared_tokens() {
        let deck = ThemeChoice::Deck.iced();
        let console = ThemeChoice::Console.iced();
        assert_eq!(deck.to_string(), "Deck");
        assert_eq!(console.to_string(), "Console");
        assert_ne!(deck.palette().background, console.palette().background);
        assert_eq!(deck.palette().background, tokens(&deck).background);
        assert_eq!(console.palette().background, tokens(&console).background);
        assert!(!tokens(&deck).metallic);
        assert!(tokens(&console).metallic);
    }
}
