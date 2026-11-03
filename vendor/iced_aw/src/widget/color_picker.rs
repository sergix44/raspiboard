//! Use a color picker as an input element for picking colors.
//!
//! *This API requires the following crate features to be activated: `color_picker`*

use self::style::{Status, StyleFn};

use super::overlay::color_picker::{
    self, ColorBarDragged, ColorPickerOverlay, ColorPickerOverlayButtons,
};

use iced_core::{
    Clipboard, Color, Element, Event, Layout, Length, Point, Rectangle, Shell, Vector, Widget,
    layout::{Limits, Node},
    mouse::{self, Cursor},
    overlay, renderer,
    widget::{
        Operation,
        tree::{self, Tag, Tree},
    },
};
use iced_widget::Renderer;

pub use crate::style::{self, color_picker::Style};

//TODO: Remove ignore when Null is updated. Temp fix for Test runs
/// An input element for picking colors.
///
/// # Example
/// ```ignore
/// # use iced_aw::ColorPicker;
/// # use iced::{Color, widget::{button, Button, Text}};
/// #
/// #[derive(Clone, Debug)]
/// enum Message {
///     Open,
///     Cancel,
///     Submit(Color),
/// }
///
/// let color_picker = ColorPicker::new(
///     true,
///     Color::default(),
///     Button::new(Text::new("Pick color"))
///         .on_press(Message::Open),
///     Message::Cancel,
///     Message::Submit,
/// );
/// ```
#[allow(missing_debug_implementations)]
pub struct ColorPicker<'a, Message, Theme = iced_widget::Theme>
where
    Message: Clone,
    Theme: style::color_picker::Catalog + iced_widget::button::Catalog,
{
    /// Show the picker.
    show_picker: bool,
    /// The color to show.
    color: Color,
    /// The underlying element.
    underlay: Element<'a, Message, Theme, Renderer>,
    /// The function that produces a message when the submit button of the [`ColorPickerOverlay`] is pressed.
    on_submit: Box<dyn Fn(Color) -> Message>,
    /// Optional function that produces a message when the color changes during selection (real-time updates).
    on_color_change: Option<Box<dyn Fn(Color) -> Message>>,
    /// The style of the [`ColorPickerOverlay`].
    class: <Theme as style::color_picker::Catalog>::Class<'a>,
    /// The buttons of the overlay.
    overlay_state: ColorPickerOverlayButtons<'a, Message, Theme>,
}

impl<'a, Message, Theme> ColorPicker<'a, Message, Theme>
where
    Message: 'a + Clone,
    Theme: 'a
        + style::color_picker::Catalog
        + iced_widget::button::Catalog
        + iced_widget::text::Catalog,
{
    /// Creates a new [`ColorPicker`] wrapping around the given underlay.
    ///
    /// It expects:
    ///     * if the overlay of the color picker is visible.
    ///     * the initial color to show.
    ///     * the underlay [`Element`] on which this [`ColorPicker`]
    ///         will be wrapped around.
    ///     * a message that will be send when the cancel button of the [`ColorPicker`]
    ///         is pressed.
    ///     * a function that will be called when the submit button of the [`ColorPicker`]
    ///         is pressed, which takes the picked [`Color`] value.
    pub fn new<U, F>(
        show_picker: bool,
        color: Color,
        underlay: U,
        on_cancel: Message,
        on_submit: F,
    ) -> Self
    where
        U: Into<Element<'a, Message, Theme, Renderer>>,
        F: 'static + Fn(Color) -> Message,
    {
        Self {
            show_picker,
            color,
            underlay: underlay.into(),
            on_submit: Box::new(on_submit),
            on_color_change: None,
            class: <Theme as style::color_picker::Catalog>::default(),
            overlay_state: ColorPickerOverlayButtons::new(on_cancel),
        }
    }

    /// Replaces the default cancel and submit buttons with custom action widgets.
    ///
    /// The cancel widget forwards its messages unchanged. Any message published by
    /// the submit widget invokes `on_submit` with the currently selected color.
    /// Both widgets must publish a message when activated; the submit message
    /// itself is ignored, so the cancel message can be reused for that widget.
    #[must_use]
    pub fn actions(
        mut self,
        cancel: impl Into<Element<'a, Message, Theme, Renderer>>,
        submit: impl Into<Element<'a, Message, Theme, Renderer>>,
    ) -> Self {
        self.overlay_state.cancel_button = cancel.into();
        self.overlay_state.submit_button = submit.into();
        self
    }

    /// Sets a callback that will be called whenever the color changes during selection (real-time updates).
    #[must_use]
    pub fn on_color_change<F>(mut self, on_color_change: F) -> Self
    where
        F: 'static + Fn(Color) -> Message,
    {
        self.on_color_change = Some(Box::new(on_color_change));
        self
    }

    /// Sets the style of the [`ColorPicker`].
    #[must_use]
    pub fn style(mut self, style: impl Fn(&Theme, Status) -> Style + 'a) -> Self
    where
        <Theme as style::color_picker::Catalog>::Class<'a>: From<StyleFn<'a, Theme, Style>>,
    {
        self.class = (Box::new(style) as StyleFn<'a, Theme, Style>).into();
        self
    }

    /// Sets the class of the input of the [`ColorPicker`].
    #[must_use]
    pub fn class(
        mut self,
        class: impl Into<<Theme as style::color_picker::Catalog>::Class<'a>>,
    ) -> Self {
        self.class = class.into();
        self
    }
}

/// The state of the [`ColorPicker`].
#[derive(Debug, Default)]
pub struct State {
    /// The state of the overlay.
    pub(crate) overlay_state: color_picker::State,
    /// Was overlay shown during the previous render?
    pub(crate) old_show_picker: bool,
}

impl State {
    /// Creates a new [`State`].
    #[must_use]
    pub fn new(color: Color) -> Self {
        Self {
            overlay_state: color_picker::State::new(color),
            old_show_picker: false,
        }
    }

    /// Resets the color of the state.
    pub fn reset(&mut self) {
        self.overlay_state.color = Color::from_rgb(0.5, 0.25, 0.25);
        self.overlay_state.color_bar_dragged = ColorBarDragged::None;
    }

    /// Synchronize with the provided color if it was changed or picker was reopened
    ///
    /// Keep the overlay state in sync. While overlay is open, it "owns" the value
    /// (there is no other way the user can update its value). When it is reopened,
    /// reset the color to the initial one.
    fn synchronize(&mut self, show_picker: bool, color: Color) {
        if show_picker && (!self.old_show_picker || self.overlay_state.initial_color != color) {
            self.overlay_state.force_synchronize(color);
        }
        self.old_show_picker = show_picker;
    }
}

impl<'a, Message, Theme> Widget<Message, Theme, Renderer> for ColorPicker<'a, Message, Theme>
where
    Message: 'static + Clone,
    Theme: 'a
        + style::color_picker::Catalog
        + iced_widget::button::Catalog
        + iced_widget::text::Catalog,
{
    fn tag(&self) -> Tag {
        Tag::of::<State>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(State::new(self.color))
    }

    fn children(&self) -> Vec<Tree> {
        vec![
            Tree::new(&self.underlay),
            Tree::new(&self.overlay_state as &dyn Widget<Message, Theme, Renderer>),
        ]
    }

    fn diff(&self, tree: &mut Tree) {
        let color_picker_state = tree.state.downcast_mut::<State>();

        color_picker_state.synchronize(self.show_picker, self.color);

        tree.diff_children(&[
            self.underlay.as_widget(),
            &self.overlay_state as &dyn Widget<Message, Theme, Renderer>,
        ]);
    }

    fn size(&self) -> iced_core::Size<Length> {
        self.underlay.as_widget().size()
    }

    fn layout(&mut self, tree: &mut Tree, renderer: &Renderer, limits: &Limits) -> Node {
        self.underlay
            .as_widget_mut()
            .layout(&mut tree.children[0], renderer, limits)
    }

    fn update(
        &mut self,
        state: &mut Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: Cursor,
        renderer: &Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        viewport: &Rectangle,
    ) {
        self.underlay.as_widget_mut().update(
            &mut state.children[0],
            event,
            layout,
            cursor,
            renderer,
            clipboard,
            shell,
            viewport,
        );
    }

    fn mouse_interaction(
        &self,
        state: &Tree,
        layout: Layout<'_>,
        cursor: Cursor,
        viewport: &Rectangle,
        renderer: &Renderer,
    ) -> mouse::Interaction {
        self.underlay.as_widget().mouse_interaction(
            &state.children[0],
            layout,
            cursor,
            viewport,
            renderer,
        )
    }

    fn draw(
        &self,
        state: &Tree,
        renderer: &mut Renderer,
        theme: &Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        cursor: Cursor,
        viewport: &Rectangle,
    ) {
        self.underlay.as_widget().draw(
            &state.children[0],
            renderer,
            theme,
            style,
            layout,
            cursor,
            viewport,
        );
    }

    fn operate<'b>(
        &'b mut self,
        state: &'b mut Tree,
        layout: Layout<'_>,
        renderer: &Renderer,
        operation: &mut dyn Operation<()>,
    ) {
        self.underlay
            .as_widget_mut()
            .operate(&mut state.children[0], layout, renderer, operation);
    }

    fn overlay<'b>(
        &'b mut self,
        tree: &'b mut Tree,
        layout: Layout<'b>,
        renderer: &Renderer,
        viewport: &Rectangle,
        translation: Vector,
    ) -> Option<overlay::Element<'b, Message, Theme, Renderer>> {
        let picker_state: &mut State = tree.state.downcast_mut();

        if !self.show_picker {
            return self.underlay.as_widget_mut().overlay(
                &mut tree.children[0],
                layout,
                renderer,
                viewport,
                translation,
            );
        }

        let bounds = layout.bounds();
        let position = Point::new(bounds.center_x(), bounds.center_y());

        Some(
            ColorPickerOverlay::new(
                picker_state,
                &mut self.overlay_state,
                &self.on_submit,
                self.on_color_change.as_deref(),
                position,
                &self.class,
                &mut tree.children[1],
                *viewport,
            )
            .overlay(),
        )
    }
}

impl<'a, Message, Theme> From<ColorPicker<'a, Message, Theme>>
    for Element<'a, Message, Theme, Renderer>
where
    Message: 'static + Clone,
    Theme: 'a
        + style::color_picker::Catalog
        + iced_widget::button::Catalog
        + iced_widget::text::Catalog,
{
    fn from(color_picker: ColorPicker<'a, Message, Theme>) -> Self {
        Element::new(color_picker)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone, Debug, PartialEq)]
    enum TestMessage {
        Cancel,
        Submit(Color),
    }

    type TestColorPicker<'a> = ColorPicker<'a, TestMessage, iced_widget::Theme>;

    fn create_test_button() -> iced_widget::Button<'static, TestMessage, iced_widget::Theme> {
        iced_widget::button(iced_widget::text::Text::new("Pick"))
    }

    #[test]
    fn color_picker_new_with_picker_hidden() {
        let color = Color::from_rgb(0.5, 0.5, 0.5);
        let button = create_test_button();

        let picker = TestColorPicker::new(
            false,
            color,
            button,
            TestMessage::Cancel,
            TestMessage::Submit,
        );

        assert!(!picker.show_picker);
        assert_eq!(picker.color, color);
    }

    #[test]
    fn color_picker_new_with_picker_shown() {
        let color = Color::from_rgb(0.3, 0.6, 0.9);
        let button = create_test_button();

        let picker = TestColorPicker::new(
            true,
            color,
            button,
            TestMessage::Cancel,
            TestMessage::Submit,
        );

        assert!(picker.show_picker);
        assert_eq!(picker.color, color);
    }

    #[test]
    fn color_picker_state_new() {
        let color = Color::from_rgb(0.5, 0.5, 0.5);
        let state = State::new(color);

        assert!(!state.old_show_picker);
    }

    #[test]
    fn color_picker_state_default() {
        let state = State::default();

        assert!(!state.old_show_picker);
    }

    #[test]
    fn color_picker_state_reset() {
        let color = Color::from_rgb(0.5, 0.5, 0.5);
        let mut state = State::new(color);

        state.reset();
        // State should still be valid after reset
        assert!(!state.old_show_picker);
    }

    fn custom_picker(show: bool) -> TestColorPicker<'static> {
        let action = |label| {
            iced_widget::container(
                iced_widget::button(iced_widget::text(label))
                    .width(Length::Fill)
                    .on_press(TestMessage::Cancel),
            )
            .width(Length::Fill)
        };
        TestColorPicker::new(
            show,
            Color::BLACK,
            create_test_button(),
            TestMessage::Cancel,
            TestMessage::Submit,
        )
        .actions(action("Cancel"), action("Apply"))
    }

    fn send_action_event(
        picker: &mut TestColorPicker<'_>,
        tree: &mut Tree,
        action: usize,
        event: &Event,
    ) -> (
        Vec<TestMessage>,
        iced_core::event::Status,
        iced_core::window::RedrawRequest,
    ) {
        let viewport = Rectangle::with_size(iced_core::Size::new(800.0, 600.0));
        let underlay = picker.layout(
            tree,
            &(),
            &Limits::new(iced_core::Size::ZERO, viewport.size()),
        );
        let mut overlay = picker
            .overlay(tree, Layout::new(&underlay), &(), &viewport, Vector::ZERO)
            .expect("picker is visible");
        let overlay = overlay.as_overlay_mut();
        let node = overlay.layout(&(), viewport.size());
        let layout = Layout::new(&node);
        let action_bounds = layout
            .children()
            .nth(1)
            .expect("picker RGBA block")
            .children()
            .nth(2 + action)
            .expect("picker action")
            .bounds();
        let cursor = Cursor::Available(action_bounds.center());
        let mut messages = Vec::new();
        let mut shell = Shell::new(&mut messages);
        overlay.update(
            event,
            layout,
            cursor,
            &(),
            &mut iced_core::clipboard::Null,
            &mut shell,
        );
        let status = shell.event_status();
        let redraw = shell.redraw_request();
        drop(shell);
        (messages, status, redraw)
    }

    #[test]
    fn color_picker_custom_cancel_action_is_used() {
        let mut picker = custom_picker(true);
        let mut tree = Tree::new(&picker as &dyn Widget<TestMessage, iced_widget::Theme, Renderer>);
        let _ = send_action_event(
            &mut picker,
            &mut tree,
            0,
            &Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)),
        );
        let (messages, status, _) = send_action_event(
            &mut picker,
            &mut tree,
            0,
            &Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)),
        );
        assert_eq!(messages, vec![TestMessage::Cancel]);
        assert_eq!(status, iced_core::event::Status::Captured);
    }

    #[test]
    fn color_picker_custom_submit_keeps_pressed_state_across_rebuild_and_hide() {
        let mut picker = custom_picker(true);
        let mut tree = Tree::new(&picker as &dyn Widget<TestMessage, iced_widget::Theme, Renderer>);
        let _ = send_action_event(
            &mut picker,
            &mut tree,
            1,
            &Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)),
        );
        let hidden_picker = custom_picker(false);
        hidden_picker.diff(&mut tree);
        let mut rebuilt_picker = custom_picker(true);
        rebuilt_picker.diff(&mut tree);
        let selected_color = Color::from_rgb(0.2, 0.4, 0.6);
        tree.state.downcast_mut::<State>().overlay_state.color = selected_color;
        let (messages, status, _) = send_action_event(
            &mut rebuilt_picker,
            &mut tree,
            1,
            &Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)),
        );
        assert_eq!(messages, vec![TestMessage::Submit(selected_color)]);
        assert_eq!(status, iced_core::event::Status::Captured);
    }

    #[test]
    fn color_picker_custom_submit_forwards_redraw_and_capture_without_messages() {
        let mut picker = custom_picker(true);
        let mut tree = Tree::new(&picker as &dyn Widget<TestMessage, iced_widget::Theme, Renderer>);
        let _ = send_action_event(
            &mut picker,
            &mut tree,
            1,
            &Event::Window(iced_core::window::Event::RedrawRequested(
                web_time::Instant::now(),
            )),
        );
        let (messages, status, redraw) = send_action_event(
            &mut picker,
            &mut tree,
            1,
            &Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)),
        );
        assert!(messages.is_empty());
        assert_eq!(status, iced_core::event::Status::Captured);
        assert_eq!(redraw, iced_core::window::RedrawRequest::NextFrame);
    }
}
