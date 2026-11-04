use super::*;
use crate::ui::test_renderer::TestRenderer;
use iced::Point;
use iced::advanced::{Shell, clipboard};
use iced::time::{Duration, Instant};

#[derive(Debug, Clone, PartialEq)]
enum Message {
    Change(u64),
    Release,
    Cancel,
}

#[test]
fn seeking_commits_clicks_and_outside_drags_and_cancels_on_focus_loss() {
    for cancel in [false, true] {
        let mut widget = view::<Message, TestRenderer>(
            1000..=8999,
            2000,
            Some(10),
            Message::Change,
            Message::Release,
            Message::Cancel,
        );
        let mut tree = Tree::new(widget.as_widget());
        let bounds = Size::new(100.0, 16.0);
        let node = widget.as_widget_mut().layout(
            &mut tree,
            &TestRenderer,
            &layout::Limits::new(Size::ZERO, bounds),
        );
        let mut messages = Vec::new();
        let mut update = |event, x| {
            widget.as_widget_mut().update(
                &mut tree,
                &event,
                Layout::new(&node),
                mouse::Cursor::Available(Point::new(x, 8.0)),
                &TestRenderer,
                &mut clipboard::Null,
                &mut Shell::new(&mut messages),
                &Rectangle::with_size(bounds),
            );
        };
        update(
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)),
            75.0,
        );
        if cancel {
            update(Event::Window(iced::window::Event::Unfocused), 75.0);
        } else {
            update(
                Event::Mouse(mouse::Event::CursorMoved {
                    position: Point::new(-10.0, 8.0),
                }),
                -10.0,
            );
        }
        update(
            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)),
            -10.0,
        );
        if cancel {
            assert_eq!(messages, vec![Message::Change(6999), Message::Cancel]);
        } else {
            assert_eq!(
                messages,
                vec![
                    Message::Change(6999),
                    Message::Change(1000),
                    Message::Release
                ]
            );
        }
    }
}

#[derive(Default)]
struct Surfaces {
    quads: Vec<(renderer::Quad, iced::Background)>,
    clips: Vec<Rectangle>,
    quad_layers: Vec<usize>,
    layer_stack: Vec<usize>,
    layer_count: usize,
}

impl renderer::Renderer for Surfaces {
    fn start_layer(&mut self, bounds: Rectangle) {
        self.clips.push(bounds);
        self.layer_count += 1;
        self.layer_stack.push(self.layer_count);
    }
    fn end_layer(&mut self) {
        self.layer_stack.pop();
    }
    fn start_transformation(&mut self, _: iced::Transformation) {}
    fn end_transformation(&mut self) {}
    fn fill_quad(&mut self, quad: renderer::Quad, background: impl Into<iced::Background>) {
        self.quads.push((quad, background.into()));
        self.quad_layers
            .push(self.layer_stack.last().copied().unwrap_or(0));
    }
    fn reset(&mut self, _: Rectangle) {
        *self = Self::default();
    }
    fn allocate_image(
        &mut self,
        handle: &iced::advanced::image::Handle,
        callback: impl FnOnce(Result<iced::advanced::image::Allocation, iced::advanced::image::Error>)
        + Send
        + 'static,
    ) {
        renderer::Renderer::allocate_image(&mut TestRenderer, handle, callback);
    }
}

#[test]
fn yellow_progress_fills_the_track_and_hover_expands_it_upward_without_relayout() {
    use raspiboard::theme::ThemeChoice;
    for choice in ThemeChoice::ALL {
        let mut widget = view::<Message, Surfaces>(
            0..=8000,
            2000,
            Some(10),
            Message::Change,
            Message::Release,
            Message::Cancel,
        );
        let mut tree = Tree::new(widget.as_widget());
        let bounds = Size::new(100.0, 16.0);
        let node = widget.as_widget_mut().layout(
            &mut tree,
            &Surfaces::default(),
            &layout::Limits::new(Size::ZERO, bounds),
        );
        let mut surfaces = Surfaces::default();
        let mut faces = Vec::new();
        let start = tree.state.downcast_ref::<State>().now;
        widget.as_widget_mut().update(
            &mut tree,
            &Event::Window(iced::window::Event::RedrawRequested(start)),
            Layout::new(&node),
            mouse::Cursor::Unavailable,
            &surfaces,
            &mut clipboard::Null,
            &mut Shell::new(&mut Vec::new()),
            &Rectangle::with_size(bounds),
        );
        widget.as_widget().draw(
            &tree,
            &mut surfaces,
            &choice.iced(),
            &renderer::Style::default(),
            Layout::new(&node),
            mouse::Cursor::Unavailable,
            &Rectangle::with_size(bounds),
        );
        assert!(surfaces.quads.is_empty(), "start hidden behind the panel");
        let now = Instant::now() + Duration::from_secs(1);
        for (cursor, at) in [
            (mouse::Cursor::Unavailable, now),
            (
                mouse::Cursor::Available(Point::new(50.0, 8.0)),
                now + Duration::from_millis(1),
            ),
            (
                mouse::Cursor::Available(Point::new(50.0, 8.0)),
                now + Duration::from_millis(300),
            ),
        ] {
            let mut messages = Vec::new();
            widget.as_widget_mut().update(
                &mut tree,
                &Event::Window(iced::window::Event::RedrawRequested(at)),
                Layout::new(&node),
                cursor,
                &surfaces,
                &mut clipboard::Null,
                &mut Shell::new(&mut messages),
                &Rectangle::with_size(bounds),
            );
            surfaces.quads.clear();
            widget.as_widget().draw(
                &tree,
                &mut surfaces,
                &choice.iced(),
                &renderer::Style::default(),
                Layout::new(&node),
                cursor,
                &Rectangle::with_size(bounds),
            );
            faces.push(*surfaces.quads.last().expect("visible progress fill"));
        }
        let (rest, background) = faces[0];
        let (hover, _) = faces[2];
        assert!(
            rest.bounds.y + rest.bounds.height > bounds.height,
            "the fill must extend behind the panel's rounded top corners"
        );
        let clip = surfaces.clips.last().unwrap();
        assert!(
            clip.y + clip.height > bounds.height,
            "the layer must allow the overlapping fill behind the panel"
        );
        assert!((rest.bounds.width - 25.0).abs() < 0.01);
        assert!(
            bounds.height - rest.bounds.y <= 3.5,
            "the exposed resting progress must be slim"
        );
        assert!(
            rest.border.radius.top_left >= 3.0,
            "top rounding must remain visible when the exposed strip is thin"
        );
        assert_eq!(rest.border.radius.bottom_left, 0.0);
        assert_eq!(rest.border.radius, hover.border.radius);
        assert!(hover.bounds.height > rest.bounds.height);
        assert!(hover.bounds.y < rest.bounds.y);
        assert_eq!(
            rest.bounds.y + rest.bounds.height,
            hover.bounds.y + hover.bounds.height
        );
        let iced::Background::Gradient(iced::gradient::Gradient::Linear(gradient)) = background
        else {
            panic!("progress needs the button bevel");
        };
        for stop in gradient.stops.into_iter().flatten() {
            assert!(
                stop.color.r > 0.7 && stop.color.g > 0.5 && stop.color.b < 0.5,
                "progress must be yellow"
            );
        }
    }
}

#[test]
fn the_progress_retracts_on_stop_then_stops_redrawing_and_ignores_input() {
    let make = |id| {
        view::<Message, Surfaces>(
            0..=8000,
            2000,
            id,
            Message::Change,
            Message::Release,
            Message::Cancel,
        )
    };
    let mut widget = make(Some(10));
    let mut tree = Tree::new(widget.as_widget());
    let bounds = Size::new(100.0, 16.0);
    let node = widget.as_widget_mut().layout(
        &mut tree,
        &Surfaces::default(),
        &layout::Limits::new(Size::ZERO, bounds),
    );
    let mut surfaces = Surfaces::default();
    let mut messages = Vec::new();
    tree.state.downcast_mut::<State>().reveal = Animation::new(true);
    widget = make(None);
    tree.diff(widget.as_widget());
    let now = Instant::now();
    let mut shell = Shell::new(&mut messages);
    widget.as_widget_mut().update(
        &mut tree,
        &Event::Window(iced::window::Event::RedrawRequested(now)),
        Layout::new(&node),
        mouse::Cursor::Unavailable,
        &surfaces,
        &mut clipboard::Null,
        &mut shell,
        &Rectangle::with_size(bounds),
    );
    assert_ne!(shell.redraw_request(), iced::window::RedrawRequest::Wait);
    widget.as_widget().draw(
        &tree,
        &mut surfaces,
        &iced::Theme::Dark,
        &renderer::Style::default(),
        Layout::new(&node),
        mouse::Cursor::Unavailable,
        &Rectangle::with_size(bounds),
    );
    assert!(
        !surfaces.quads.is_empty(),
        "retain the bar while it retracts"
    );
    let mut shell = Shell::new(&mut messages);
    widget.as_widget_mut().update(
        &mut tree,
        &Event::Window(iced::window::Event::RedrawRequested(
            now + Duration::from_secs(1),
        )),
        Layout::new(&node),
        mouse::Cursor::Unavailable,
        &surfaces,
        &mut clipboard::Null,
        &mut shell,
        &Rectangle::with_size(bounds),
    );
    assert_eq!(shell.redraw_request(), iced::window::RedrawRequest::Wait);
    surfaces.quads.clear();
    widget.as_widget().draw(
        &tree,
        &mut surfaces,
        &iced::Theme::Dark,
        &renderer::Style::default(),
        Layout::new(&node),
        mouse::Cursor::Unavailable,
        &Rectangle::with_size(bounds),
    );
    assert!(surfaces.quads.is_empty());
    widget.as_widget_mut().update(
        &mut tree,
        &Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)),
        Layout::new(&node),
        mouse::Cursor::Available(Point::new(75.0, 8.0)),
        &surfaces,
        &mut clipboard::Null,
        &mut Shell::new(&mut messages),
        &Rectangle::with_size(bounds),
    );
    assert!(messages.is_empty());
}

#[test]
fn the_panel_covers_the_seek_overlap_in_a_later_render_layer() {
    let progress = view::<Message, Surfaces>(
        0..=8000,
        2000,
        Some(10),
        Message::Change,
        Message::Release,
        Message::Cancel,
    );
    let panel = iced::widget::container(iced::widget::space().width(Fill).height(32)).style(|_| {
        iced::widget::container::Style {
            background: Some(iced::Color::BLACK.into()),
            ..Default::default()
        }
    });
    let mut widget = stack_panel(progress, panel.into());
    let mut tree = Tree::new(widget.as_widget());
    let viewport = Rectangle::with_size(Size::new(100.0, 48.0));
    let mut surfaces = Surfaces::default();
    let node = widget.as_widget_mut().layout(
        &mut tree,
        &surfaces,
        &layout::Limits::new(Size::ZERO, viewport.size()),
    );
    widget.as_widget_mut().update(
        &mut tree,
        &Event::Window(window::Event::RedrawRequested(
            Instant::now() + Duration::from_secs(1),
        )),
        Layout::new(&node),
        mouse::Cursor::Unavailable,
        &surfaces,
        &mut clipboard::Null,
        &mut Shell::new(&mut Vec::new()),
        &viewport,
    );
    widget.as_widget().draw(
        &tree,
        &mut surfaces,
        &Theme::Dark,
        &renderer::Style::default(),
        Layout::new(&node),
        mouse::Cursor::Unavailable,
        &viewport,
    );
    let panel_index = surfaces
        .quads
        .iter()
        .position(|(quad, _)| quad.bounds.y == 16.0 && quad.bounds.height == 32.0)
        .expect("panel background");
    let fill_index = surfaces
        .quads
        .iter()
        .position(|(_, background)| matches!(background, iced::Background::Gradient(_)))
        .expect("progress fill");
    assert!(
        surfaces.quad_layers[panel_index] > surfaces.quad_layers[fill_index],
        "panel must cover the overlap, not be batched below it"
    );
    let mut messages = Vec::new();
    for button in [
        mouse::Event::ButtonPressed(mouse::Button::Left),
        mouse::Event::ButtonReleased(mouse::Button::Left),
    ] {
        widget.as_widget_mut().update(
            &mut tree,
            &Event::Mouse(button),
            Layout::new(&node),
            mouse::Cursor::Available(Point::new(75.0, 8.0)),
            &surfaces,
            &mut clipboard::Null,
            &mut Shell::new(&mut messages),
            &viewport,
        );
    }
    assert_eq!(
        messages,
        vec![Message::Change(6000), Message::Release],
        "the foreground panel must not block seek gestures above it"
    );
}
