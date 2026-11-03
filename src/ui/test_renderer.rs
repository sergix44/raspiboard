use iced::advanced::{image, renderer};
use iced::{Background, Rectangle, Transformation};

pub(super) fn load_font() {
    iced::advanced::graphics::text::font_system()
        .write()
        .expect("font system")
        .load_font(super::fonts::FONT_BYTES.into());
}

// Use the real text layout without a window or a graphics backend.
#[derive(Default)]
pub(super) struct TestRenderer;

impl renderer::Renderer for TestRenderer {
    fn start_layer(&mut self, _bounds: Rectangle) {}
    fn end_layer(&mut self) {}
    fn start_transformation(&mut self, _transformation: Transformation) {}
    fn end_transformation(&mut self) {}
    fn fill_quad(&mut self, _quad: renderer::Quad, _background: impl Into<Background>) {}
    fn reset(&mut self, _new_bounds: Rectangle) {}
    fn allocate_image(
        &mut self,
        _handle: &image::Handle,
        callback: impl FnOnce(Result<image::Allocation, image::Error>) + Send + 'static,
    ) {
        callback(Err(image::Error::Unsupported));
    }
}

impl iced::advanced::text::Renderer for TestRenderer {
    type Font = iced::Font;
    type Paragraph = iced::advanced::graphics::text::Paragraph;
    type Editor = iced::advanced::graphics::text::Editor;

    const ICON_FONT: iced::Font = iced::Font::DEFAULT;
    const CHECKMARK_ICON: char = '✓';
    const ARROW_DOWN_ICON: char = '▼';
    const SCROLL_UP_ICON: char = '▲';
    const SCROLL_DOWN_ICON: char = '▼';
    const SCROLL_LEFT_ICON: char = '◀';
    const SCROLL_RIGHT_ICON: char = '▶';
    const ICED_LOGO: char = ' ';

    fn default_font(&self) -> iced::Font {
        load_font();
        super::fonts::FONT
    }
    fn default_size(&self) -> iced::Pixels {
        iced::Pixels(16.0)
    }
    fn fill_paragraph(
        &mut self,
        _: &Self::Paragraph,
        _: iced::Point,
        _: iced::Color,
        _: Rectangle,
    ) {
    }
    fn fill_editor(&mut self, _: &Self::Editor, _: iced::Point, _: iced::Color, _: Rectangle) {}
    fn fill_text(
        &mut self,
        _: iced::advanced::text::Text,
        _: iced::Point,
        _: iced::Color,
        _: Rectangle,
    ) {
    }
}

impl image::Renderer for TestRenderer {
    type Handle = image::Handle;

    fn load_image(&self, _handle: &Self::Handle) -> Result<image::Allocation, image::Error> {
        Err(image::Error::Unsupported)
    }

    fn measure_image(&self, _handle: &Self::Handle) -> Option<iced::Size<u32>> {
        Some(iced::Size::new(30, 30))
    }

    fn draw_image(
        &mut self,
        _image: image::Image<Self::Handle>,
        _bounds: Rectangle,
        _clip_bounds: Rectangle,
    ) {
    }
}
