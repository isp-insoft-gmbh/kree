# kree — User Manual

A Windows tray app for cron-scheduled reminders. Edits live in a plain text file.

## Install

1. Download `kree.exe` (or `cargo build --release` from source — see `README.md`).
2. Run it. A tray icon appears.
3. Optional: open the main window (left-click tray) and toggle **Start with Windows**.

## Add a reminder

1. Right-click the tray icon → **Edit Reminders**. Your `$VISUAL` / `$EDITOR` opens `%APPDATA%\kree\reminders.txt`.
2. Add a line:

   ```
   <cron-expression> | <message>
   ```

3. Save and close. kree reloads automatically.

### Examples

```
# every 30 minutes 09:00–17:00 on weekdays
*/30 9-17 * * 1-5  | 💧 Drink water

# 17:00 weekdays
0 17 * * 1-5       | 🛑 Shutdown ritual

# Mondays at 09:00
0 9 * * 1          | 📋 Weekly review
```

The first emoji of the message becomes the popup icon. No emoji = default 🔔.

## When a reminder fires

1. A short chime plays.
2. A popup appears above the tray icon.
3. If you don't dismiss within 2 seconds, the message is read aloud.

Auto-dismiss after 30 seconds.

## Tray menu

| Item | Action |
|---|---|
| Pause / Resume | Stop firing reminders (icon greys out) |
| Edit Reminders | Open `reminders.txt` in your editor |
| Reload | Re-parse `reminders.txt` |
| Open Log | Open `%APPDATA%\kree\logs\app.log` |
| Quit | Exit kree |

Closing the main window only hides it. Quit only via tray menu.

## File locations

| Path | Purpose |
|---|---|
| `%APPDATA%\kree\reminders.txt` | Your reminders |
| `%APPDATA%\kree\logs\app.log` | Daily-rolling log |

## Cron syntax

POSIX 5-field: `minute hour day-of-month month day-of-week`.

| Field | Range |
|---|---|
| minute | 0–59 |
| hour | 0–23 |
| day of month | 1–31 |
| month | 1–12 |
| day of week | 0–6 (0 = Sun) |

Operators: `*`, `,`, `-`, `/`. Examples in the file above.

## Troubleshooting

- **Reminders don't fire:** check `app.log` for parser warnings — bad lines are skipped, not fatal.
- **No sound:** confirm system audio is up. The chime is a short `.ogg`; any default audio device works.
- **TTS silent:** Windows SAPI must be available (it is by default on Win10/11). Confirm a voice is selected in Settings → Time & Language → Speech.
- **Reload didn't pick up edits:** the file watcher debounces ~500 ms; wait a beat, then check the log.

---

*Jaffa, kree!*
