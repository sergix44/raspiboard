use iced::widget::{
    Action,
    canvas::{self, Canvas, Frame, Geometry, Path, Program, Stroke},
};
use iced::{Color, Element, Length, Point, Rectangle, Renderer, Size, Theme, mouse, window};

use raspiboard::audio::waveform::Waveform;

pub(super) fn view<'a, Message: Clone + 'a>(
    waveform: &'a Waveform,
    start_ms: Option<u64>,
    end_ms: Option<u64>,
    on_change: impl Fn(u64, u64, bool) -> Message + 'a,
) -> Element<'a, Message> {
    Canvas::new(Selection {
        waveform,
        range: Range::new(start_ms, end_ms, waveform.duration_ms),
        on_change,
    })
    .width(Length::Fill)
    .height(128)
    .into()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Range {
    start: u64,
    end: u64,
}

impl Range {
    fn new(start: Option<u64>, end: Option<u64>, duration: u64) -> Self {
        let duration = duration.max(1);
        let start = start.unwrap_or(0).min(duration - 1);
        let end = end.unwrap_or(duration).clamp(start + 1, duration);
        Self { start, end }
    }

    fn move_start(self, time: u64) -> Self {
        Self {
            start: time.min(self.end - 1),
            ..self
        }
    }

    fn move_end(self, time: u64, duration: u64) -> Self {
        Self {
            end: time.clamp(self.start + 1, duration.max(self.start + 1)),
            ..self
        }
    }

    fn selection(anchor: u64, time: u64, duration: u64) -> Self {
        let duration = duration.max(1);
        let start = anchor.min(time).min(duration - 1);
        let end = anchor.max(time).clamp(start + 1, duration);
        Self { start, end }
    }

    fn click(self, time: u64) -> Self {
        if self.start.abs_diff(time) <= self.end.abs_diff(time) {
            self.move_start(time)
        } else {
            Self {
                end: time.max(self.start + 1),
                ..self
            }
        }
    }
}

fn time_at(x: f32, width: f32, duration: u64) -> u64 {
    if !width.is_finite() || width <= 0.0 || !x.is_finite() {
        return 0;
    }
    ((f64::from(x / width).clamp(0.0, 1.0) * duration as f64).round() as u64).min(duration)
}

fn x_at(time: u64, width: f32, duration: u64) -> f32 {
    (time as f64 / duration.max(1) as f64) as f32 * width
}

#[derive(Debug, Clone, Copy)]
enum DragMode {
    Start,
    End,
    Selection(u64),
}

#[derive(Debug, Clone, Copy)]
struct Drag {
    mode: DragMode,
    original: Range,
    range: Range,
    press_x: f32,
    moved: bool,
}

#[derive(Default)]
struct State {
    source: Option<(usize, usize, u64)>,
    drag: Option<Drag>,
}

struct Selection<'a, F> {
    waveform: &'a Waveform,
    range: Range,
    on_change: F,
}

impl<F> Selection<'_, F> {
    fn identity(&self) -> (usize, usize, u64) {
        (
            self.waveform.peaks.as_ptr() as usize,
            self.waveform.peaks.len(),
            self.waveform.duration_ms,
        )
    }

    fn drag_mode(&self, x: f32, width: f32) -> DragMode {
        let start = x_at(self.range.start, width, self.waveform.duration_ms);
        let end = x_at(self.range.end, width, self.waveform.duration_ms);
        let distance_start = (x - start).abs();
        let distance_end = (x - end).abs();
        if distance_start <= 10.0 && distance_start <= distance_end {
            DragMode::Start
        } else if distance_end <= 10.0 {
            DragMode::End
        } else {
            DragMode::Selection(time_at(x, width, self.waveform.duration_ms))
        }
    }

    fn move_drag(&self, drag: &mut Drag, x: f32, width: f32) {
        drag.moved |= (x - drag.press_x).abs() > 3.0;
        if !drag.moved {
            return;
        }
        let time = time_at(x, width, self.waveform.duration_ms);
        drag.range = match drag.mode {
            DragMode::Start => drag.original.move_start(time),
            DragMode::End => drag.original.move_end(time, self.waveform.duration_ms),
            DragMode::Selection(anchor) => {
                Range::selection(anchor, time, self.waveform.duration_ms)
            }
        };
    }
}

impl<Message, F> Program<Message> for Selection<'_, F>
where
    F: Fn(u64, u64, bool) -> Message,
{
    type State = State;

    fn update(
        &self,
        state: &mut State,
        event: &canvas::Event,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Option<Action<Message>> {
        if state.source != Some(self.identity()) {
            state.source = Some(self.identity());
            state.drag = None;
        }
        match event {
            canvas::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                let point = cursor.position_in(bounds)?;
                if self.waveform.duration_ms == 0 {
                    return None;
                }
                state.drag = Some(Drag {
                    mode: self.drag_mode(point.x, bounds.width),
                    original: self.range,
                    range: self.range,
                    press_x: point.x,
                    moved: false,
                });
                Some(Action::capture())
            }
            canvas::Event::Mouse(mouse::Event::CursorMoved { position }) => {
                let drag = state.drag.as_mut()?;
                self.move_drag(drag, position.x - bounds.x, bounds.width);
                if !drag.moved {
                    return Some(Action::capture());
                }
                Some(
                    Action::publish((self.on_change)(drag.range.start, drag.range.end, false))
                        .and_capture(),
                )
            }
            canvas::Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) => {
                let mut drag = state.drag.take()?;
                if let Some(point) = cursor.position() {
                    self.move_drag(&mut drag, point.x - bounds.x, bounds.width);
                }
                if !drag.moved {
                    let time = time_at(drag.press_x, bounds.width, self.waveform.duration_ms);
                    drag.range = drag.original.click(time);
                }
                Some(
                    Action::publish((self.on_change)(drag.range.start, drag.range.end, true))
                        .and_capture(),
                )
            }
            canvas::Event::Window(window::Event::Unfocused) => {
                let drag = state.drag.take()?;
                Some(Action::publish((self.on_change)(
                    drag.original.start,
                    drag.original.end,
                    false,
                )))
            }
            _ => None,
        }
    }

    fn draw(
        &self,
        state: &State,
        renderer: &Renderer,
        theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<Geometry> {
        let mut frame = Frame::new(renderer, bounds.size());
        let palette = theme.extended_palette();
        let background = palette.background.weak.color;
        let accent = palette.primary.base.color;
        frame.fill_rectangle(Point::ORIGIN, bounds.size(), background);
        let range = state
            .drag
            .filter(|_| state.source == Some(self.identity()))
            .map_or(self.range, |drag| drag.range);
        let start = x_at(range.start, bounds.width, self.waveform.duration_ms);
        let end = x_at(range.end, bounds.width, self.waveform.duration_ms);
        frame.fill_rectangle(
            Point::new(start, 0.0),
            Size::new(end - start, bounds.height),
            Color { a: 0.16, ..accent },
        );
        let midpoint = bounds.height / 2.0;
        let count = self.waveform.peaks.len().max(1);
        for (index, peak) in self.waveform.peaks.iter().enumerate() {
            let x = (index as f32 + 0.5) / count as f32 * bounds.width;
            let height = if peak.is_finite() {
                peak.clamp(0.0, 1.0) * (midpoint - 14.0)
            } else {
                0.0
            }
            .max(0.5);
            let color = if x >= start && x <= end {
                accent
            } else {
                palette.background.strong.text
            };
            frame.stroke(
                &Path::line(
                    Point::new(x, midpoint - height),
                    Point::new(x, midpoint + height),
                ),
                Stroke::default()
                    .with_width((bounds.width / count as f32).max(1.0))
                    .with_color(color),
            );
        }
        for x in [start, end] {
            let x = x.clamp(0.75, (bounds.width - 0.75).max(0.75));
            let handle_color = palette.warning.base.color;
            frame.fill_rectangle(
                Point::new(x - 0.75, 0.0),
                Size::new(1.5, bounds.height),
                handle_color,
            );
            frame.fill_rectangle(
                Point::new((x - 3.0).max(0.0), 0.0),
                Size::new(6.0_f32.min(bounds.width - x + 3.0), 8.0),
                handle_color,
            );
        }
        vec![frame.into_geometry()]
    }

    fn mouse_interaction(
        &self,
        state: &State,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> mouse::Interaction {
        if state.drag.is_some() && state.source == Some(self.identity()) {
            return mouse::Interaction::ResizingHorizontally;
        }
        if let Some(point) = cursor.position_in(bounds) {
            match self.drag_mode(point.x, bounds.width) {
                DragMode::Start | DragMode::End => mouse::Interaction::ResizingHorizontally,
                DragMode::Selection(_) => mouse::Interaction::Crosshair,
            }
        } else {
            mouse::Interaction::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_waveform() -> Waveform {
        Waveform {
            peaks: vec![0.2, 0.8],
            duration_ms: 1000,
        }
    }

    fn message(
        selection: &Selection<'_, impl Fn(u64, u64, bool) -> (u64, u64, bool)>,
        state: &mut State,
        event: canvas::Event,
        x: f32,
    ) -> Option<(u64, u64, bool)> {
        selection
            .update(
                state,
                &event,
                Rectangle::new(Point::ORIGIN, Size::new(1000.0, 128.0)),
                mouse::Cursor::Available(Point::new(x, 50.0)),
            )
            .and_then(|action| action.into_inner().0)
    }

    #[test]
    fn release_outside_commits_clamped_range_once_after_preview() {
        let waveform = sample_waveform();
        let selection = Selection {
            waveform: &waveform,
            range: Range {
                start: 200,
                end: 800,
            },
            on_change: |a, b, c| (a, b, c),
        };
        let mut state = State::default();
        assert_eq!(
            message(
                &selection,
                &mut state,
                canvas::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)),
                200.0
            ),
            None
        );
        assert_eq!(
            message(
                &selection,
                &mut state,
                canvas::Event::Mouse(mouse::Event::CursorMoved {
                    position: Point::new(-50.0, 50.0)
                }),
                -50.0
            ),
            Some((0, 800, false))
        );
        assert_eq!(
            message(
                &selection,
                &mut state,
                canvas::Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)),
                -100.0
            ),
            Some((0, 800, true))
        );
        assert_eq!(
            message(
                &selection,
                &mut state,
                canvas::Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)),
                -100.0
            ),
            None
        );
    }

    #[test]
    fn drag_survives_program_redraw_with_updated_preview_range() {
        let waveform = sample_waveform();
        let callback = |a, b, c| (a, b, c);
        let selection = Selection {
            waveform: &waveform,
            range: Range {
                start: 200,
                end: 800,
            },
            on_change: callback,
        };
        let mut state = State::default();
        message(
            &selection,
            &mut state,
            canvas::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)),
            500.0,
        );
        assert_eq!(
            message(
                &selection,
                &mut state,
                canvas::Event::Mouse(mouse::Event::CursorMoved {
                    position: Point::new(600.0, 50.0)
                }),
                600.0
            ),
            Some((500, 600, false))
        );
        let redrawn = Selection {
            range: Range {
                start: 500,
                end: 600,
            },
            ..selection
        };
        assert_eq!(
            message(
                &redrawn,
                &mut state,
                canvas::Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)),
                300.0
            ),
            Some((300, 500, true))
        );
    }

    #[test]
    fn replacing_source_discards_old_drag_without_emitting_to_new_target() {
        let waveform = sample_waveform();
        let callback = |a, b, c| (a, b, c);
        let selection = Selection {
            waveform: &waveform,
            range: Range {
                start: 200,
                end: 800,
            },
            on_change: callback,
        };
        let mut state = State::default();
        message(
            &selection,
            &mut state,
            canvas::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)),
            200.0,
        );
        let replacement = sample_waveform();
        let replaced = Selection {
            waveform: &replacement,
            ..selection
        };
        assert_eq!(
            message(
                &replaced,
                &mut state,
                canvas::Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)),
                100.0
            ),
            None
        );
    }

    #[test]
    fn time_coordinates_clamp_both_sides_and_handle_zero_width() {
        assert_eq!(time_at(-20.0, 200.0, 5000), 0);
        assert_eq!(time_at(100.0, 200.0, 5000), 2500);
        assert_eq!(time_at(220.0, 200.0, 5000), 5000);
        assert_eq!(time_at(100.0, 0.0, 5000), 0);
    }

    #[test]
    fn moving_handles_cannot_cross_or_exceed_duration() {
        let range = Range {
            start: 200,
            end: 800,
        };
        assert_eq!(
            range.move_start(900),
            Range {
                start: 799,
                end: 800
            }
        );
        assert_eq!(
            range.move_end(0, 1000),
            Range {
                start: 200,
                end: 201
            }
        );
        assert_eq!(
            range.move_end(2000, 1000),
            Range {
                start: 200,
                end: 1000
            }
        );
    }

    #[test]
    fn selecting_backwards_orders_the_interval_and_preserves_minimum() {
        assert_eq!(
            Range::selection(800, 200, 1000),
            Range {
                start: 200,
                end: 800
            }
        );
        assert_eq!(
            Range::selection(1000, 1000, 1000),
            Range {
                start: 999,
                end: 1000
            }
        );
        assert_eq!(Range::selection(0, 0, 1000), Range { start: 0, end: 1 });
    }

    #[test]
    fn invalid_saved_values_are_clamped_for_safe_drawing() {
        assert_eq!(
            Range::new(Some(2000), Some(200), 1000),
            Range {
                start: 999,
                end: 1000
            }
        );
        assert_eq!(Range::new(None, None, 1), Range { start: 0, end: 1 });
    }

    #[test]
    fn single_click_moves_the_nearest_boundary() {
        let range = Range {
            start: 200,
            end: 800,
        };
        assert_eq!(
            range.click(100),
            Range {
                start: 100,
                end: 800
            }
        );
        assert_eq!(
            range.click(600),
            Range {
                start: 200,
                end: 600
            }
        );
    }
}
