use iced::advanced::text::{self, Paragraph, paragraph};
use iced::advanced::widget::{Tree, tree};
use iced::advanced::{Layout, Widget, layout, mouse, renderer};
use iced::{Element, Fill, Font, Length, Rectangle, Size, Theme};

pub(super) fn label<'a, Message: 'a>(content: &'a str) -> Element<'a, Message> {
    Element::new(FittedLabel(content))
}

struct FittedLabel<'a>(&'a str);

fn fit<P: Paragraph<Font = Font>>(
    paragraph: &mut paragraph::Plain<P>,
    content: &str,
    bounds: Size,
) {
    let mut definition = text::Text {
        content,
        bounds: Size::INFINITE,
        size: 18.0.into(),
        line_height: text::LineHeight::Relative(1.2),
        font: super::fonts::FONT,
        align_x: text::Alignment::Center,
        align_y: iced::alignment::Vertical::Center,
        shaping: text::Shaping::Advanced,
        wrapping: text::Wrapping::None,
    };
    paragraph.update(definition);
    let measured = paragraph.min_bounds();
    let scale = (bounds.width / measured.width.max(1.0))
        .min(bounds.height / measured.height.max(1.0))
        .min(1.0);
    let single_line_size = 18.0 * scale;
    definition.bounds = Size::new(bounds.width, f32::INFINITY);
    if single_line_size >= 10.0 {
        definition.size = single_line_size.into();
    } else {
        // Wrap unusually long names before reducing them below a readable size.
        definition.wrapping = text::Wrapping::Glyph;
        definition.size = 10.0.into();
    }
    loop {
        paragraph.update(definition);
        let measured = paragraph.min_bounds();
        if (measured.width <= bounds.width && measured.height <= bounds.height)
            || definition.size.0 <= 1.0
        {
            break;
        }
        definition.size = (definition.size.0 - 0.5).max(1.0).into();
    }
    definition.bounds = bounds;
    paragraph.update(definition);
}

impl<Message, Renderer> Widget<Message, Theme, Renderer> for FittedLabel<'_>
where
    Renderer: text::Renderer<Font = Font>,
{
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<paragraph::Plain<Renderer::Paragraph>>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(paragraph::Plain::<Renderer::Paragraph>::default())
    }

    fn size(&self) -> Size<Length> {
        Size::new(Fill, Fill)
    }

    fn layout(
        &mut self,
        tree: &mut Tree,
        _renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        let size = limits.resolve(Fill, Fill, Size::ZERO);
        fit(
            tree.state
                .downcast_mut::<paragraph::Plain<Renderer::Paragraph>>(),
            self.0,
            size,
        );
        layout::Node::new(size)
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut Renderer,
        _theme: &Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        _cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        let paragraph = tree
            .state
            .downcast_ref::<paragraph::Plain<Renderer::Paragraph>>();
        iced::advanced::widget::text::draw(
            renderer,
            style,
            layout.bounds(),
            paragraph.raw(),
            iced::advanced::widget::text::Style::default(),
            viewport,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use iced::advanced::graphics::text::Paragraph as GraphicsParagraph;

    #[test]
    fn short_labels_keep_their_size_and_long_names_fit_the_pad() {
        crate::ui::test_renderer::load_font();
        let bounds = Size::new(156.0, 62.0);
        let mut paragraph = paragraph::Plain::<GraphicsParagraph>::default();
        fit(&mut paragraph, "Horn", bounds);
        assert_eq!(paragraph.raw().size().0, 18.0);
        for label in ["A moderately long name", &"very_long_filename_".repeat(12)] {
            fit(&mut paragraph, label, bounds);
            assert!(paragraph.raw().size().0 < 18.0);
            assert!(paragraph.min_width() <= bounds.width);
            assert!(paragraph.min_height() <= bounds.height);
            assert_eq!(paragraph.align_x(), text::Alignment::Center);
            assert_eq!(paragraph.align_y(), iced::alignment::Vertical::Center);
        }
        fit(&mut paragraph, "Horn", bounds);
        assert_eq!(paragraph.raw().size().0, 18.0);
    }

    #[test]
    fn label_fitting_reacts_to_available_width() {
        crate::ui::test_renderer::load_font();
        let mut paragraph = paragraph::Plain::<GraphicsParagraph>::default();
        fit(
            &mut paragraph,
            "A moderately long name",
            Size::new(156.0, 62.0),
        );
        let narrow_size = paragraph.raw().size().0;
        fit(
            &mut paragraph,
            "A moderately long name",
            Size::new(400.0, 62.0),
        );
        assert!(paragraph.raw().size().0 > narrow_size);
        assert_eq!(paragraph.raw().size().0, 18.0);
    }
}
