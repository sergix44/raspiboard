use super::{Message, Soundboard};
use iced::advanced::renderer;
use iced::widget::{container, mouse_area, space, stack};
use iced::{Element, Event, Fill, Task, Theme, mouse, window};
use raspiboard::storage::Change;
use raspiboard::theme::tokens;

pub(super) struct Drag {
    pub source: String,
    pub target: Option<String>,
}

pub(super) fn window_event(event: &Event, window: window::Id) -> Option<Message> {
    match event {
        Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Middle)) => {
            Some(Message::EndReorder(window))
        }
        Event::Mouse(mouse::Event::CursorLeft) | Event::Window(window::Event::Unfocused) => {
            Some(Message::CancelReorder(window))
        }
        _ => None,
    }
}

pub(super) fn pad<'a, Renderer: renderer::Renderer + 'a>(
    content: Element<'a, Message, Theme, Renderer>,
    id: &str,
    source: bool,
    edge: Option<bool>,
) -> Element<'a, Message, Theme, Renderer> {
    let pad: Element<'a, Message, Theme, Renderer> = mouse_area(content)
        .on_right_press(Message::OpenButtonMenu(id.into()))
        .on_middle_press(Message::BeginReorder(id.into()))
        .on_enter(Message::HoverReorder(id.into()))
        .on_exit(Message::LeaveReorder(id.into()))
        .into();
    let overlay: Element<'a, Message, Theme, Renderer> = if source {
        let outline = container(space().width(Fill).height(Fill))
            .width(Fill)
            .height(Fill)
            .style(|theme: &Theme| container::Style {
                border: iced::Border {
                    color: tokens(theme).accent,
                    width: 2.0,
                    radius: tokens(theme).radius.into(),
                },
                ..container::Style::default()
            });
        outline.into()
    } else if let Some(after) = edge {
        let line = container(space().width(3).height(Fill))
            .width(3)
            .height(Fill)
            .style(|theme: &Theme| container::Style {
                background: Some(tokens(theme).accent.into()),
                ..container::Style::default()
            });
        let marker = if after {
            container(line).align_right(Fill)
        } else {
            container(line).align_left(Fill)
        };
        marker.height(Fill).into()
    } else {
        space().width(Fill).height(Fill).into()
    };
    // Retain the mouse area's state when drag feedback appears or disappears.
    stack![pad, overlay].into()
}

impl Soundboard {
    pub(super) fn begin_reorder(&mut self, id: String) {
        if self.choosing_file.is_some() || !self.buttons.iter().any(|b| b.id == id) {
            return;
        }
        self.grid_menu = None;
        self.button_menu = None;
        self.reorder = Some(Drag {
            source: id.clone(),
            target: Some(id),
        });
    }

    pub(super) fn hover_reorder(&mut self, id: String) {
        let Some(drag) = &mut self.reorder else {
            return;
        };
        let category = self.button_categories.get(&drag.source);
        drag.target =
            (category.is_some() && category == self.button_categories.get(&id)).then_some(id);
    }

    pub(super) fn reorder_edge(&self, id: &str) -> Option<bool> {
        let drag = self.reorder.as_ref()?;
        if drag.target.as_deref() != Some(id) || drag.source == id {
            return None;
        }
        let source = self.buttons.iter().position(|b| b.id == drag.source)?;
        let target = self.buttons.iter().position(|b| b.id == id)?;
        Some(source < target)
    }

    pub(super) fn finish_reorder(&mut self) -> Task<Message> {
        let Some(Drag {
            source,
            target: Some(target),
        }) = self.reorder.take()
        else {
            return Task::none();
        };
        if source == target {
            return Task::none();
        }
        let Some(category_id) = self.button_categories.get(&source).cloned() else {
            return Task::none();
        };
        if self.button_categories.get(&target) != Some(&category_id) {
            return Task::none();
        }
        let Some(from) = self.buttons.iter().position(|b| b.id == source) else {
            return Task::none();
        };
        let Some(to) = self.buttons.iter().position(|b| b.id == target) else {
            return Task::none();
        };
        let button = self.buttons.remove(from);
        self.buttons.insert(to, button);
        let mut ids = Vec::new();
        for button in &mut self.buttons {
            if self.button_categories.get(&button.id) == Some(&category_id) {
                ids.push(button.id.clone());
                button.sorting = ids.len() as u64;
            }
        }
        self.save(Change::ReorderButtons { category_id, ids })
    }
}

#[cfg(test)]
mod tests;
