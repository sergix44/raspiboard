# Audio fixtures

These are synthetic 440 Hz sine tones created with FFmpeg, with no third-party audio.
They exercise the enabled Symphonia codecs and containers without live playback.

Base command (replace codec and filename):

```sh
ffmpeg -f lavfi -i 'sine=frequency=440:duration=0.1:sample_rate=44100' -c:a CODEC FILE
```

Codecs: libmp3lame, aac, alac, libvorbis, pcm_s16be (AIFF), pcm_s16le (CAF),
flac (FLAC/Matroska), adpcm_ima_wav, mp2. MP2 uses `-b:a 128k` because its
mono stream must use a legal MPEG Layer II bitrate. The FLAC fixture is 0.04 s.
The `tone-opus.ogg` and `tone-opus.webm` fixtures use libopus at 48 kHz with
`-ac 2`. The remaining WebM fixture uses Vorbis. Opus is supplied through the
Symphonia libopus adapter.
