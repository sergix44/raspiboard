use super::{
    buffer::{SampleFeed, TransportFade},
    graph::{APP_ID, LOCAL_LINK_GROUP},
    job::Decoded,
};
use pipewire::{self as pw, properties::properties, spa};
use std::{
    cell::RefCell,
    io::Cursor,
    rc::Rc,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU8, Ordering},
        mpsc,
    },
};

pub struct Player {
    _listener: pw::stream::StreamListener<SampleFeed>,
    pub stream: pw::stream::StreamRc,
    pub finished: Arc<AtomicBool>,
    pub drained: Arc<AtomicBool>,
    pub result: mpsc::Receiver<Result<(), String>>,
    pub error: Rc<RefCell<Option<String>>>,
    pub draining: bool,
    pub enabled: Arc<AtomicBool>,
    pub stop: Arc<AtomicBool>,
}

impl Player {
    pub fn new(
        core: &pw::core::CoreRc,
        sink: Option<&str>,
        decoded: Decoded,
        volume: Arc<AtomicU8>,
        paused: Arc<AtomicBool>,
    ) -> Result<Self, String> {
        let mut props = properties! {
            "application.name" => "Raspiboard",
            "application.id" => APP_ID,
            "media.type" => "Audio",
            "media.category" => "Playback",
            "media.role" => "Music",
            "node.dont-fallback" => "true",
            "node.dont-reconnect" => "true",
            "audio.channels" => "2",
        };
        if let Some(sink) = sink {
            props.insert("target.object", sink);
        } else {
            // Keep automatic local playback away from our private remote sinks.
            props.insert("node.link-group", LOCAL_LINK_GROUP);
        }
        let stream = pw::stream::StreamRc::new(
            core.clone(),
            if sink.is_some() {
                "Raspiboard remote"
            } else {
                "Raspiboard local"
            },
            props,
        )
        .map_err(|e| e.to_string())?;
        let finished = Arc::new(AtomicBool::new(false));
        let drained = Arc::new(AtomicBool::new(false));
        let on_drained = drained.clone();
        let error = Rc::new(RefCell::new(None));
        let enabled = Arc::new(AtomicBool::new(false));
        let on_error = error.clone();
        let stop = Arc::new(AtomicBool::new(false));
        let feed = SampleFeed {
            consumer: decoded.samples,
            eof: decoded.eof,
            finished: finished.clone(),
            volume,
            clip_gain: decoded.clip_gain,
            enabled: enabled.clone(),
            stop: stop.clone(),
            paused: paused.clone(),
            fade: TransportFade::new(decoded.rate),
        };
        let listener = stream
            .add_local_listener_with_user_data(feed)
            .state_changed(move |_, _, _, state| {
                if let pw::stream::StreamState::Error(message) = state {
                    *on_error.borrow_mut() = Some(message.to_owned());
                } else if matches!(state, pw::stream::StreamState::Unconnected) {
                    *on_error.borrow_mut() = Some("The playback stream disconnected.".into());
                }
            })
            .drained(move |_, _| on_drained.store(true, Ordering::Release))
            .process(|stream, feed| {
                let Some(mut buffer) = stream.dequeue_buffer() else {
                    return;
                };
                let requested = buffer.requested() as usize;
                let Some(data) = buffer.datas_mut().first_mut() else {
                    return;
                };
                let size = if let Some(bytes) = data.data() {
                    let frames = bytes.len() / 8;
                    let frames = if requested == 0 {
                        frames
                    } else {
                        frames.min(requested)
                    };
                    feed.render(&mut bytes[..frames * 8])
                } else {
                    0
                };
                let chunk = data.chunk_mut();
                *chunk.offset_mut() = 0;
                *chunk.stride_mut() = 8;
                *chunk.size_mut() = size as u32;
            })
            .register()
            .map_err(|e| e.to_string())?;
        let mut info = spa::param::audio::AudioInfoRaw::new();
        info.set_format(spa::param::audio::AudioFormat::F32LE);
        info.set_rate(decoded.rate);
        info.set_channels(2);
        let mut position = [0; spa::param::audio::MAX_CHANNELS];
        position[0] = spa::sys::SPA_AUDIO_CHANNEL_FL;
        position[1] = spa::sys::SPA_AUDIO_CHANNEL_FR;
        info.set_position(position);
        let bytes = spa::pod::serialize::PodSerializer::serialize(
            Cursor::new(Vec::new()),
            &spa::pod::Value::Object(spa::pod::Object {
                type_: spa::sys::SPA_TYPE_OBJECT_Format,
                id: spa::sys::SPA_PARAM_EnumFormat,
                properties: info.into(),
            }),
        )
        .map_err(|e| format!("Cannot describe the audio format: {e:?}"))?
        .0
        .into_inner();
        let pod = spa::pod::Pod::from_bytes(&bytes).ok_or("Invalid audio format descriptor.")?;
        stream
            .connect(
                spa::utils::Direction::Output,
                None,
                pw::stream::StreamFlags::AUTOCONNECT | pw::stream::StreamFlags::MAP_BUFFERS,
                &mut [pod],
            )
            .map_err(|e| e.to_string())?;
        Ok(Self {
            _listener: listener,
            stream,
            finished,
            drained,
            result: decoded.result,
            error,
            draining: false,
            enabled,
            stop,
        })
    }
}
