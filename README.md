# kree

> *"Loosely translated it means 'Attention', 'Listen up', 'Concentrate'."* — Daniel Jackson, SG-1

A lightweight Windows tray app for cron-scheduled reminders.

- Reminders live in a plain text file, one per line.
- When a reminder fires: bundled chime → popup above the tray icon → speech if you don't dismiss within 2 seconds.
- Edit reminders in your `$VISUAL` / `$EDITOR` (falls back to Notepad).

📖 **[Read the user guide → `MANUAL.md`](MANUAL.md)**

## Install

### From source (current)

Prerequisites: a recent stable Rust toolchain (rustup → `stable`) and Windows 10 / 11.

```
git clone https://github.com/<your-fork>/kree.git
cd kree
cargo build --release
```

The resulting `target/release/kree.exe` is a single self-contained binary — no DLLs, no installer, no service. Drop it anywhere on your PATH (e.g. `%LOCALAPPDATA%\Programs\kree\kree.exe`) and run it.

### Run on login

Open the main window via tray left-click and tick **Start with Windows**. That writes an `HKCU\Software\Microsoft\Windows\CurrentVersion\Run` entry pointing at the current binary.

### Prebuilt binary

Not yet — see [open issues](../../issues) or build from source above.

## Use cases

kree is happiest with short, recurring nudges that don't need a calendar. Some shapes:

- **Hydration / posture / breathing.** `*/30 9-17 * * 1-5 | 💧 Drink water`
- **Daily rituals.** `0 9 * * 1-5 | ☕ Standup in 10`, `0 17 * * 1-5 | 🛑 Shutdown ritual` (close tabs, flush inbox, write tomorrow's first task).
- **Weekly cadences.** `0 9 * * 1 | 📋 Weekly review`, `0 16 * * 5 | 🧹 Friday tidy-up`.
- **Habits.** `*/45 9-18 * * * | 🧘 Stand and stretch`, `0 11 * * * | 🌳 Step outside for 5`.
- **Meds / supplements.** `0 8 * * * | 💊 Morning meds`, `0 21 * * * | 💊 Evening meds`.
- **Focused-work nudges.** `*/25 9-17 * * 1-5 | 🍅 Pomodoro tick` — paired with **Pause All** when you don't want interruptions.
- **Personal release process.** `0 14 * * 4 | 🚀 Cut weekly release branch`, `0 10 * * 1 | 🔍 Review oncall handoff`.

Anything that's *cron-shaped and short* is a fit. Anything that needs context, attendees, or a snooze button — use a calendar.

## Reminders file

Lives at `%APPDATA%\kree\reminders.txt`. Format:

```
# Lines starting with # are comments. Blank lines ignored.
# Format: <cron-expression> | <message>
# The first emoji of the message becomes the popup icon.

*/30 9-17 * * 1-5  | 💧 Drink water
0 17 * * 1-5       | 🛑 End of day shutdown
0 9 * * 1          | 📋 Weekly review
```

Type emoji with the Windows picker (`Win + .`). Save the file — kree hot-reloads within ~500 ms.

## Documentation

- [`MANUAL.md`](MANUAL.md) — user guide (install, edit, troubleshoot, cron syntax).
- [`SPEC.md`](SPEC.md) — full build specification.
- [`CLAUDE.md`](CLAUDE.md) — agent working instructions.

## License

MIT — see [`LICENSE`](LICENSE).

---

🛸 Lovingly hand-vibed through the chappa'ai with the
[Claude Code](https://claude.com/claude-code) coding harness.
*Tek'ma'tek, Jaffa.*

*Jaffa, kree!*
