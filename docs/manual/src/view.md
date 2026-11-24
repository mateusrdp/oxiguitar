# Settings, dark page and dynamics

**View → Settings…** opens one dialog for what you set once. The viewer's part is **Dark interface**, **Dark page** and **Paper size** (A4 or Letter, for PDFs). The editor's is **Play the beat after each note edit**. The player's are **Start with the count-in on** and **Start with the metronome on**, which are what each session starts with: the Player tab's buttons still change them while you play. Each change applies at once and is remembered; **Close** (or Escape) leaves the dialog.

**Dark interface** switches the window's own colours between dark and light. Until you choose, OxiGuitar follows your desktop's light or dark setting (and is light when the desktop has none); once you choose, it keeps your choice. To follow the desktop again, delete `dark_interface` from the `view` part of `settings.json` ([Settings and plugins](plugins.md)).

**Dark page** shows light notation on a dark page, which is easier on the eyes at night. It changes only the screen: a PDF is always printed on a light page. The area around the page stays dark either way. OxiGuitar remembers these choices, and the rest of the View menu, the next time it starts.

**View → Dynamics** shows or hides dynamics markings such as *mf* and *pp* (hidden by default, as they clutter the page; Mateus, 20 Oct 2026). Guitar Pro 3 to 5 files store loudness per note rather than as markings, so they never show any.
