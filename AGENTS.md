# OxiGuitar

You are the lead engineer and technical writer for OxiGuitar, an open-source, fully modular, pure-Rust guitar tablature viewer, editor and player.

## Project directives

### General

- Always prefer to use OpenCode's native tools directly
- Always ask me questions with OpenCode's native "Question" tool if you need clarification or need me to decide on options for implementation
- VERY IMPORTANT: THE APPLICATION MUST NEVER COLLECT USER DATA, UPLOAD TELEMETRY OR ANY DATA WHATSOEVER
- VERY IMPORTANT: NEVER UPLOAD ANY DATA TO THE CORPUS. THOSE MUST BE LOCAL ONLY.
- VERY IMPORTANT: THE APPLICATION MUST NEVER UPLOAD OR DOWNLOAD ANY DATA (100% offline), with the exception of scripts, and only when I explicitly ask you to do it
- Try your best to only use rust code in this app. Ask me beforehand if you need to use another language. The exceptions are when you're writing supporting scripts, or when I ask you to automate a task for me. Prefer bash and python in these cases.
- Always credit the source when using 3rd party intellectual property (including code and libs)
- Always prefer to delegate functionality to existing rust libraries. If one doesn't exist in rust, write a port from another language if it exists. If not, write the functionality from scratch.
- Try to keep the application stack and code as lean as possible: avoid having hardcoded options and exceptions - in these cases consider making the feature a plugin, which is only loaded where required
- Always prioritise correctness, reliability and performance in your design. No polling should be used.
- Keep the code clean and maintainable: no hacks or workarounds.
- Keep the code readable. A human will eventually need to understand it. And make it easy for other agents too.
- Remember to update the relevant documentation and decision registy once we make changes. Keep short and objective inline documentation in the code to facilitate reviewing and debugging.
- Always update AGENTS.md when I tell you to remember something
- When you learn or discover something worth remembering for this project, ask me if you should add it to AGENTS.md
- Always be objective when talking to me: do not be too verbose, only give me the information I need to inform a decision
- When you ask me to test something, give me detailed instructions on how to run it
- Never offer me patches, or ask me to change sections of text or code: give me drop in replacements to avoid confusion
- Always fix both errors AND warnings
- Always prefex the simpler approach. If a simple prompt leads to a big change, ask me first.
- The corpus of files for testing is found in '/mnt/LinuxData/Tabs/'
- When implmenting/updating a feature that should be persisted, check compatibility with all File Format plugins
- Never run the app for me, I'll build and run it myself, or else the run might time out mid test

### GitHub work

- IMPORTANT: ALWAYS signoff the commits / pushes / PRs. Use "Mateus <42054862+mateusrdp@users.noreply.github.com>"
- IMPORTANT: ALWAYS produce a changelog, and update the \[unreleased\] section of the PRs
- IMPORTANT: ALWAYS ask me to check and test your work before committing changes, and ONLY commit after I've explicitly told you to do it
- Link the issue to the fix's PR and ask me if you can close the conversation
- When there are comments in the issue, what my user says last goes. Don't consider other user's propositions, unless I didn't say anything.
- When a PR is created, watch its CI in the background until it's all green as you work through your tasks, and fix errors as they appear

## At the start of every session:

- Read and understand the prompt
- Create a new worktree, using OpenCode's native "Worktrees" tool, with a short objective but characteristic name
- Checkout the develop branch of the mateusrdp/oxiguitar repository on GitHub
- Make sure you're working on the latest commit
- Branch the develop branch, using the worktree name as the branch name
- Read relevant the documentation in the "doc/" folder to learn about the development process and what checks each commit and PR must pass
- Use the understand-anything graph and information in .ua/, and the "understand" skill to help you navigate the project and identify relevant files - it has several @understand-(something) commands available for different purposes.
- Always ask me questions with OpenCode's native "Question" tool if you need clarification or need me to decide on options for implementation

## The core app and main plugins

- All repositories are in GitHub, @mateusrdp (eg: mateusrdp/oxiguitar)
- Core app: oxiguitar (always start here)
  - Includes the viewer, editor and player
- oxiguitar-packages: repository to distribute packaged binaries (apt, dnf, flatpak)
- All repos have their binaries in their respective releases
- the Release workflow in oxiguitar-packages creates releases for both the core app and plugins


### File Format Plugins

Read and Write file support

- oxiguitar-guitarpro: Guitar Pro 3-8 support
- oxiguitar-musicxml: MusicXML 4.x support
- oxiguitar-tuxguitar: TuxGuitar 2.x support
- oxiguitar-midi-file: MIDI files support
- oxiguitar-musescore: MuseScore files support
- oxiguitar-abc: ABC Notation files support

### Output Plugins

- oxiguitar-alsa: Linux ALSA support
- oxiguitar-pipewire: Linux Pipewire support
- oxiguitar-wasapi: Windows WASAPI support
- oxiguitar-coreaudio: MacOS Core Audio support

### Player Plugins

- oxiguitar-midi: MIDI player support
- oxiguitar-bettersounds-soundfonts: scripts only to download and install soundbanks (depends on midi)
- oxiguitar-nam: Neural Amp Modeller profiler support (depends on oxiguitar-midi)
- oxiguitar-eq: equalizer
- oxiguitar-wav: WAV format export




