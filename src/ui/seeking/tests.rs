use super::*;
fn default_board() -> Soundboard {
    Soundboard::default()
}

fn playback(id: u64) -> audio::PlaybackInstance {
    audio::PlaybackInstance {
        button_id: "sound".into(),
        status: audio::PlaybackStatus::Playing,
        preview: false,
        position_ms: 2000,
        seek_range: Some(audio::SeekRange {
            playback_id: id,
            start_ms: 1000,
            end_ms: 9000,
        }),
    }
}

#[test]
fn dragging_holds_the_draft_until_the_seek_is_acknowledged_and_rejects_stale_events() {
    let mut board = default_board();
    board.audio.playbacks = vec![playback(10)];
    board.seek_changed(10, 6000);
    assert_eq!(board.seek_position_ms(), Some(6000));
    board.audio.playbacks[0].position_ms = 2500;
    assert_eq!(board.seek_position_ms(), Some(6000));
    assert!(matches!(
        board.seek_command(10),
        Some(audio::Command::Seek {
            playback_id: 10,
            position_ms: 6000
        })
    ));

    board.audio.playbacks[0] = playback(11);
    assert_eq!(board.seek_position_ms(), Some(2000));
    board.seek_changed(10, 8000);
    assert_eq!(board.seek_position_ms(), Some(2000));
    assert!(board.seek_command(10).is_none());
    assert!(board.seek_command(11).is_none());

    board.seek_changed(11, u64::MAX);
    assert_eq!(board.seek_position_ms(), Some(8999));
    board.seek_changed(11, 0);
    assert_eq!(board.seek_position_ms(), Some(1000));
    board.audio.playbacks.clear();
    assert!(board.seek_command(11).is_none());
    assert_eq!(board.seek_position_ms(), None);
}

#[test]
fn progress_requires_one_eligible_playback_and_remains_available_when_paused_or_seeking() {
    let mut board = default_board();
    board.audio.playbacks = vec![playback(10)];
    board.audio.playbacks[0].status = audio::PlaybackStatus::Paused;
    assert_eq!(board.seek_position_ms(), Some(2000));
    board.audio.playbacks[0].status = audio::PlaybackStatus::Loading;
    assert_eq!(board.seek_position_ms(), Some(2000));
    board.audio.playbacks.push(playback(11));
    assert_eq!(board.seek_position_ms(), None);
    board.audio.playbacks.truncate(1);
    board.audio.playbacks[0].seek_range = None;
    assert_eq!(board.seek_position_ms(), None);
}

#[test]
fn stopping_or_disconnecting_clears_the_draft_before_a_playback_id_can_be_reused() {
    let mut board = default_board();
    board.audio.playbacks = vec![playback(10)];
    board.seek_changed(10, 6000);
    let _ = board.update(Message::Audio(audio::Event::Changed(Box::default())));
    board.audio.playbacks = vec![playback(10)];
    assert_eq!(board.seek_position_ms(), Some(2000));
    assert!(board.seek_command(10).is_none());
}
