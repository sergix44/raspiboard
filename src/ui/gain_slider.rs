use super::volume_slider;
use iced::advanced::renderer;
use iced::widget::{mouse_area, slider};
use iced::{Element, Fill, Theme, mouse};

pub(super) fn view<'a, Message: Clone + 'a, Renderer: renderer::Renderer + 'a>(
    value: f32,
    on_change: impl Fn(f32) -> Message + Clone + 'a,
) -> Element<'a, Message, Theme, Renderer> {
    let control = slider(-15.0..=15.0, value.clamp(-15.0, 15.0), on_change.clone())
        .step(0.5_f32)
        .height(22)
        .width(Fill)
        .style(volume_slider::style);
    mouse_area(control)
        .on_scroll(move |delta| {
            let (mouse::ScrollDelta::Lines { y, .. } | mouse::ScrollDelta::Pixels { y, .. }) =
                delta;
            let step = if y.is_finite() && y != 0.0 {
                y.signum() * 0.5
            } else {
                0.0
            };
            on_change((value + step).clamp(-15.0, 15.0))
        })
        .into()
}

#[cfg(test)]
mod tests;
