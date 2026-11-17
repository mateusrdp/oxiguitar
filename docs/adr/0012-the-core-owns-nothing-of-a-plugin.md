# 0012 — The core owns nothing of a plugin

- Status: accepted (Mateus, 17 Nov 2026)
- Date: 2026-11-17
- Milestone: v1.0 (oxiguitar-midi#19)
- Supersedes: ADR 0008's "A plugin may have a settings window"

## Context

oxiguitar-midi#19: picking sound sets in the MIDI plugin's settings window lagged, and picks made while going down a list with the keyboard queued up, each closing the list when it arrived. The window was the core's (interface 1.3): the core drew its frame and its Close button, said "Changes are saved automatically", and prepared the sound again after every change. The plugin couldn't hold its changes until the user decided, because nothing told it the window had closed.

Interface 1.8 already gives plugins dialogs of their own, with OK and Cancel, and tells the plugin how the user closed them.

## Decision

Mateus (17 Nov 2026):

- **The core owns nothing of a plugin.** What a plugin shows, keeps or decides is the plugin's: the core offers generic places and services (tabs, dialogs, tools, menus, footer messages, settings storage, track sections) and never a part made for one plugin or one kind of plugin. This is a directive of ours (CONTRIBUTING.md, "Design principles").
- **Settings windows are retired** (interface 1.9). A plugin shows its settings in a dialog of its own, opened by a tool, and keeps them when the user presses OK.
- Backward compatibility doesn't hold this back before v1.0.

To make that possible without the core knowing any plugin, interface 1.9 adds two generic things:

- **A tool that doesn't edit the score** (`"edits": false` in the manifest): its menu entry works with no score open, and it runs while the song plays.
- **Settings that changed prepare the sound again**: whenever a plugin saves settings that differ from what it had (`HostRequest::SetSettings`), the host tells the window (`Core::settings_changed`), which stops the song and prepares the instruments and effects again if that plugin plays any. Before, only the settings window's events did this.

The two retired requests (`Request::RetiredSettingsWindow`, `Request::RetiredSettingsEvent`) keep their place in the enum, never sent, so every later request keeps its encoding and plugins built with an older SDK still load.

## Consequences

- The plugin manager only lists, enables and disables. The MIDI and WAV plugins move to dialogs of their own (oxiguitar-midi#19, oxiguitar-wav); until they do, a newer OxiGuitar shows no settings for their older releases.
- The test plugin's `SettingsDialog` is the reference: a draft changed by the dialog, written with OK, forgotten with Cancel.
- What else the core still owns on plugins' behalf, and what removing each would take, was listed for Mateus in session 47 (the session's report and `claude/START-HERE.md`); each removal is his to schedule.
