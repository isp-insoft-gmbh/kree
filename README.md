# kree

> *"Loosely translated it means 'Attention', 'Listen up', 'Concentrate'."* — Daniel Jackson, SG-1

A lightweight Windows tray app for cron-scheduled reminders.

- Reminders live in a plain text file, one per line.
- When a reminder fires: bundled chime → popup above the tray icon → speech if you don't dismiss within 2 seconds.
- Edit reminders in your `$VISUAL` / `$EDITOR` (falls back to Notepad).

## Build

```
cargo build --release
```

The resulting `target/release/kree.exe` is a single self-contained binary.

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

Type emoji with the Windows picker (`Win + .`).

## Documentation

- [`MANUAL.md`](MANUAL.md) — user guide.
- [`SPEC.md`](SPEC.md) — full build specification.
- [`CLAUDE.md`](CLAUDE.md) — agent working instructions.

## License

MIT — see [`LICENSE`](LICENSE).

---

*Jaffa, kree!*
