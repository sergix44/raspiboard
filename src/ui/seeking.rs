use super::{Message, Soundboard, seek_slider};
use iced::Element;
use raspiboard::audio;

impl Soundboard {
    pub(super) fn sync_seek_draft(&mut self) {
        if let Some((id, _)) = self.seek_draft
            && self
                .seek_playback()
                .is_none_or(|(_, range)| range.playback_id != id)
        {
            self.seek_draft = None;
        }
    }

    fn seek_playback(&self) -> Option<(&audio::PlaybackInstance, audio::SeekRange)> {
        match self.audio.playbacks.as_slice() {
            [instance] => Some((instance, instance.seek_range?)),
            _ => None,
        }
    }

    pub(super) fn seek_position_ms(&self) -> Option<u64> {
        let (instance, range) = self.seek_playback()?;
        Some(
            self.seek_draft
                .filter(|(id, _)| *id == range.playback_id)
                .map_or(instance.position_ms, |(_, position)| position)
                .clamp(range.start_ms, range.end_ms - 1),
        )
    }

    pub(super) fn seek_changed(&mut self, playback_id: u64, position_ms: u64) {
        if let Some((_, range)) = self.seek_playback()
            && range.playback_id == playback_id
        {
            self.seek_draft = Some((
                playback_id,
                position_ms.clamp(range.start_ms, range.end_ms - 1),
            ));
        }
    }

    pub(super) fn seek_command(&self, playback_id: u64) -> Option<audio::Command> {
        let (_, range) = self.seek_playback()?;
        let (draft_id, position_ms) = self.seek_draft?;
        (range.playback_id == playback_id && draft_id == playback_id).then_some(
            audio::Command::Seek {
                playback_id,
                position_ms,
            },
        )
    }

    pub(super) fn seek_control(&self) -> Element<'_, Message> {
        let playback = self.seek_playback();
        let id = playback.map(|(_, range)| range.playback_id);
        let range = playback.map_or(0..=1, |(_, range)| range.start_ms..=range.end_ms - 1);
        let position = self.seek_position_ms().unwrap_or(0);
        seek_slider::view(
            range,
            position,
            id,
            move |position_ms| Message::SeekChanged {
                playback_id: id.unwrap_or(0),
                position_ms,
            },
            Message::SeekReleased(id.unwrap_or(0)),
            Message::SeekCancelled(id.unwrap_or(0)),
        )
    }
}

#[cfg(test)]
mod tests;
