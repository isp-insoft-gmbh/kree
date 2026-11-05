## Fixes

- **Reminders now appear while kree sits in the tray.** A reminder that
  fired after the main window had been opened and then closed did not show
  up: it waited in the queue until something else woke the app — clicking
  the tray icon, usually — and if it had aged past the 30-second staleness
  cutoff by then, it was discarded without ever being shown. If you have
  been missing reminders, this is why.
- **The popup goes beside the taskbar wherever the taskbar is.** With the
  taskbar docked left, most of the popup was off the left edge of the
  screen; docked right or on top it ran off those edges instead. The side
  is now worked out from the tray icon's position, so all four dockings
  place the reminder correctly.
- **Reminders land on the display the tray is on.** With the taskbar on a
  secondary monitor the popup could appear on the primary one, nowhere near
  the tray.
- **A burst of reminders stays on screen.** Stacked popups no longer walk
  off the edge of the desktop once enough of them are open at the same
  time; they come to rest at the last fully visible slot instead.
- **Popups opened while hidden now auto-dismiss.** They were kept alive by
  the same mechanism that failed to show them, so they would have stayed up
  indefinitely.

## Internal

- The log now records the tray icon rect, the desktop work area and the
  inferred taskbar edge whenever a reminder is placed, so a popup that
  lands somewhere wrong can be diagnosed from the log alone.
- Dependency updates: eframe 0.37, tray-icon 0.26, rodio 0.23, croner
  4.0.1, plus assorted patch bumps.

## Testing this build

The two main fixes need a particular setup to see:

- For the taskbar fix, move the Windows taskbar to the left, right or top
  edge and wait for a reminder.
- For the tray fix, **open the main window and close it with the X**, then
  wait for a reminder. Starting kree silently at login never showed the bug.
