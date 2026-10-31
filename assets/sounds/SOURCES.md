# Sounds

`shutter.ogg` is synthesised for easyshot (two filtered noise-and-tone clicks, 70 ms apart, 150 ms total, mono 48 kHz Vorbis, decoded peak -4.2 dBFS). It is our own work and is dedicated to the public domain under CC0 1.0 (https://creativecommons.org/publicdomain/zero/1.0/). No third-party audio is used.

Generated with:

```sh
ffmpeg -y \
  -f lavfi -i "aevalsrc='(random(1)*2-1)*exp(-t*220)+0.6*sin(2*PI*2400*t)*exp(-t*400)':d=0.15:s=48000:c=mono" \
  -f lavfi -i "aevalsrc='0.8*(random(2)*2-1)*exp(-t*160)+0.5*sin(2*PI*1500*t)*exp(-t*300)':d=0.15:s=48000:c=mono" \
  -filter_complex "[0:a]highpass=f=1200,lowpass=f=9000[a];[1:a]highpass=f=800,lowpass=f=7000,adelay=70[b];[a][b]amix=inputs=2:normalize=0,atrim=0:0.15,volume=0.5,alimiter=limit=0.6:level=disabled,afade=t=out:st=0.135:d=0.015" \
  -ac 1 -ar 48000 -c:a libvorbis -q:a 4 assets/sounds/shutter.ogg
```

The noise is seeded (`random(1)`, `random(2)`), so the output is reproducible with the same ffmpeg build.
