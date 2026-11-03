use super::{
    Message, Soundboard, Target,
    icons::{Icon, icon},
    sound_pad,
};
use iced::widget::{button, canvas, column, pick_list, row};
use iced::{Color, Element, Point, Renderer, Theme, mouse};
use raspiboard::audio::{Command, PassthroughStatus};

const GREEN: Color = Color::from_rgb(0.655, 0.953, 0.816);

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct SourceChoice(pub Option<Target>);

impl std::fmt::Display for SourceChoice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.0 {
            Some(source) => f.write_str(&source.display_label),
            None => f.write_str("No Input"),
        }
    }
}

impl Soundboard {
    pub(super) fn passthrough_command(&self) -> Option<Command> {
        if self.audio.passthrough != PassthroughStatus::Off {
            Some(Command::SetPassthrough(false))
        } else {
            (self.audio.connected && self.audio.source.is_some() && self.audio.selected.is_some())
                .then_some(Command::SetPassthrough(true))
        }
    }

    pub(super) fn routing_controls(&self) -> Element<'_, Message> {
        let active = self.audio.passthrough == PassthroughStatus::Active;
        let enabled = self.passthrough_command().is_some();
        let color = match self.audio.passthrough {
            PassthroughStatus::Off => GREEN.scale_alpha(0.5),
            PassthroughStatus::Connecting => GREEN.scale_alpha(0.8),
            PassthroughStatus::Active => GREEN,
        };
        let toggle = sound_pad::blinking(
            button(icon(Icon::Cable).size(18))
                .width(32)
                .height(28)
                .padding(4)
                .on_press_maybe(enabled.then_some(Message::TogglePassthrough)),
            enabled,
            active,
            color,
        );
        let choices = std::iter::once(SourceChoice(None))
            .chain(
                self.audio
                    .sources
                    .iter()
                    .cloned()
                    .map(|source| SourceChoice(Some(source))),
            )
            .collect::<Vec<_>>();
        let source = pick_list(
            choices,
            Some(SourceChoice(self.audio.source.clone())),
            |choice| Message::SelectSource(choice.0),
        )
        .text_size(12)
        .padding(6)
        .width(182);
        let selectors = column![
            row![toggle, source].spacing(6).align_y(iced::Center),
            self.target_controls(),
        ]
        .spacing(6);
        row![
            canvas::Canvas::new(RoutingArrow { active })
                .width(20)
                .height(62),
            selectors
        ]
        .spacing(4)
        .align_y(iced::Center)
        .into()
    }
}

struct RoutingArrow {
    active: bool,
}

impl canvas::Program<Message> for RoutingArrow {
    type State = ();

    fn draw(
        &self,
        _: &(),
        renderer: &Renderer,
        theme: &Theme,
        bounds: iced::Rectangle,
        _: mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        let mut frame = canvas::Frame::new(renderer, bounds.size());
        let path = canvas::Path::new(|path| {
            path.move_to(Point::new(18.0, 14.0));
            path.line_to(Point::new(6.0, 14.0));
            path.line_to(Point::new(6.0, 48.0));
            path.line_to(Point::new(18.0, 48.0));
            path.move_to(Point::new(14.0, 44.0));
            path.line_to(Point::new(18.0, 48.0));
            path.line_to(Point::new(14.0, 52.0));
        });
        let color = if self.active {
            GREEN
        } else {
            theme.palette().text.scale_alpha(0.4)
        };
        frame.stroke(
            &path,
            canvas::Stroke::default().with_color(color).with_width(2.0),
        );
        vec![frame.into_geometry()]
    }
}
