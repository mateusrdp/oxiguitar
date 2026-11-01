# Advice

Since interface 1.4, a plugin can ask OxiGuitar to show the user a dialog: *"Only your system's basic sound set is installed. Free recorded sets sound much better; here is where to get them."* The user reads it, may click a link, and may tick **Don't show this again**. OxiGuitar remembers what was ticked, so your plugin never has to.

Advice is for the thing a plugin can't do without help from the user: a model file to put in a folder, a sound set to install. It is not for status messages ([the footer](sdk.md) holds those) and not for anything the plugin can fix itself.

## Asking

```rust,ignore
use oxiguitar_plugin::api::message::Link;
use oxiguitar_plugin::Advice;

host.advise(Advice {
    id: "no-model".into(),
    title: "No amp model yet".into(),
    text: "NAM needs a model file (.nam) before it can change a track's sound.\n\
           Put one in the nam folder beside the plugins folder."
        .into(),
    links: vec![Link {
        text: "Where to get amp models".into(),
        url: "https://mateusrdp.github.io/oxiguitar/manual/amp-models.html".into(),
    }],
})?;
```

| Field | What it is |
| --- | --- |
| `id` | What the advice is about: 1 to 64 lowercase letters, digits and hyphens (`no-model`). Unique within your plugin; it is what the user's *Don't show this again* silences |
| `title` | The dialog's heading: a few words, at most 80 characters |
| `text` | What to tell the user, at most 1200 characters. A new line starts a new line in the dialog; there is no other markup |
| `links` | At most three, each with the words shown (60 characters) and an `https://` address |

`advise` returns an error only when the advice can't be shown as given (a fourth link, an `http:` address, an id with capitals): the error says what to change.  In a test with a detached host (`Host::detached()`) it does nothing and returns `Ok`.

## When it is shown

- **Once the window is up, one dialog at a time.** Ask whenever you find out, even in `start`: a dialog that arrives before the window exists waits for it, and several wait in line.
- **Once a session.** Ask again as often as your reason holds (every time a score opens, say); OxiGuitar passes on each advice once per session and the plugin is told nothing either way.
- **Not at all after the user ticks the box.** It is saved in settings.json, one entry for each plugin and advice, and the dialog never comes back for it:

  ```json
  "advice": { "yourname.example": { "no-model": "hidden" } }
  ```

  A user who wants a dialog back deletes its line. Other advice of the same plugin is untouched.
- **Never in safe mode** (`oxiguitar --safe-mode`), where no plugin runs.

## Links

A plugin can't open anything itself. The links in an advice dialog open in the system's browser only when the user clicks them, and only `https://` addresses are taken, as with `Widget::Link` ([Tabs](tabs.md)) and the footer's links. OxiGuitar sends nothing anywhere on its own, and nothing about the click goes back to your plugin.

Link to a page you keep up to date, and say in the text what to do there: the page may change, the dialog's words can't.

## Writing it well

- Say what is missing, then what to do, in the user's words: "No amp model yet", not "model_path is empty".
- One advice per thing. Two separate gaps (a model and a cabinet) are two advices with two ids, so the user can silence one and keep the other.
- Don't ask for what the plugin works without. A plugin that sounds fine out of the box needs no dialog.
- Ask only when it matters now: when the user opens a score on a track the plugin would play, not at every start.

## Which interface

A plugin that calls `advise` must ask for interface 1.4 in its manifest (`"interface": "1.4"`); the host refuses the call from a plugin built for an earlier minor version. See [Interface versions](versions.md).
