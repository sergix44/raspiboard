use super::{Button, Soundboard};

impl Soundboard {
    pub(super) fn visible_buttons(&self) -> impl Iterator<Item = &Button> {
        let query = self.search.trim().to_lowercase();
        self.buttons
            .iter()
            .filter(move |button| button.display_name().to_lowercase().contains(&query))
    }
}

#[cfg(test)]
mod tests;
