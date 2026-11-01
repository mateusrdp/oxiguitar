# Amp models (NAM)

OxiGuitar can play a guitar or bass track through an **amp model**: a capture of a real amplifier (or pedal, or a whole rig) made with [Neural Amp Modeler](https://github.com/sdatkinson/neural-amp-modeler), stored in a `.nam` file. The track's sound goes through the model, then through a **cabinet**: an impulse response of a speaker cabinet and its microphone. The NAM plugin does it; it comes with OxiGuitar. OxiGuitar downloads nothing: models are files you get and put in a folder.

## What you need

1. **A guitar sound to feed the amp.** An amp model expects what a guitar's cable carries, a clean, plain signal, called DI (direct input). Any guitar sound plays through it, but a DI set sounds the most real. FreePats' **Direct** set is one ([Better sounds](better-sounds.md), "Electric guitars as SoundFonts"): put its `.sf2` file in your `soundfonts` folder and pick it for **Electric guitars** in the SoundFont plugin's settings window.
2. **Amp models.** Thousands are shared for free, most of them at [TONE3000](https://www.tone3000.com/). Each capture's page says its licence; most allow using it and publishing what you play through it, but not passing the file on. A capture marked as an amp head (amp only) needs a cabinet after it; a full rig already has one.
3. **Cabinets, if you want others.** Four come with OxiGuitar (below). Any `.wav` impulse response works, mono or the first channel of a stereo file, at any sample rate.

## Where to put them

Put `.nam` and `.wav` files in the `nam` folder, beside your `plugins` and `soundfonts` folders (OxiGuitar creates it when it starts), or in folders inside it, up to five levels deep:

| System | The `nam` folder |
| --- | --- |
| Linux | `~/.local/share/oxiguitar/nam/` |
| macOS | `~/Library/Application Support/OxiGuitar/nam/` |
| Windows | `%APPDATA%\OxiGuitar\nam\` |

With `OXIGUITAR_HOME` set, it is the `nam` folder in that folder.

## Using them on a track

Open a guitar or bass track's dialog (right-click its name, **Properties…**, or **Edit → Track Properties…**; [Tracks and notation](tracks.md)). Its **Amp (NAM)** section has:

- **Play this track through an amp model**: on or off. Picking a model turns it on.
- **Model**: every `.nam` file in the `nam` folder, by its path inside the folder.
- **Cabinet**: **From the model** (none for a model that says it includes its cabinet, the first built-in one otherwise), **None**, one of the four built-in cabinets, or a `.wav` file from the `nam` folder.
- **Input** and **Output**, from −24 to +24 dB. Input drives the amp harder or softer, as a guitar's volume knob does; output sets the track's level after it. A model whose file says how loud it plays is first brought to −18 dBFS, so models sound about as loud as each other.

The settings are saved per track beside the score, in `FILE.oxg`, like every plugin's track settings; the score itself doesn't change. A track whose model is missing (the file was moved or deleted) plays without it, and the bottom of the window says so.

## How heavy it is

An amp model is a neural network computed for every sample, much heavier than playing samples: measured on GitHub's build machines, a standard model takes a fifth to two fifths of one processor core per track. On Linux on a PC whose processor has AVX2 and FMA (Intel from 2013, AMD from 2015, nearly every one in use), OxiGuitar uses a build of the plugin made for them, which takes about a tenth of that and sounds the same; its cabinets come 64 samples (about 1.3 ms) later, and the other tracks wait for them. OxiGuitar plays each track on the audio device's own thread, so a few amp tracks at once can be too much for a slower computer. If playback stutters, use the amp on fewer tracks, or pick lighter models (captures labelled "lite", "feather" or "nano"). Models run at the rate they were captured at, 48 kHz; on a sound device that runs at another rate, OxiGuitar converts the track to 48 kHz and back, which makes it a millisecond or so late, and moves every other track and the cursor by the same amount so all stays together.

## The built-in cabinets

Four impulse responses from the **BestPlugins Mega Pack 2** by David Fau Casquel, as [Guitarix](https://github.com/brummer10/guitarix) ships them, under the GNU GPL, version 2 or later: **Mesa Boogie Mark V**, **Marshall JMP 2203 (modded)**, **EVH 5150 III** and **Engl Retro Tube**. They are named after the amplifiers their author made them sound like; the makers have nothing to do with them.
