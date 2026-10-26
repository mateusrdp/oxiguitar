# SFZ fixtures

Our own files for the SFZ sampler's tests (`plugins/soundfont`), made with
ffmpeg: a 440 Hz sine at half scale, 0.1 s at 44.1 kHz.

```sh
G='aevalsrc=0.5*sin(2*PI*440*t):s=44100:d=0.1'
ffmpeg -f lavfi -i "$G" -c:a flac -sample_fmt s16 -bitexact sine-16.flac
ffmpeg -f lavfi -i "$G" -c:a flac -sample_fmt s32 -bits_per_raw_sample 24 -bitexact sine-24.flac
ffmpeg -f lavfi -i "$G" -c:a libvorbis -q:a 4 -bitexact sine.ogg
```
