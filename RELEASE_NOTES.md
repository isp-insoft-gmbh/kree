## Fixes

- **A burst of reminders no longer buries itself.** When several fired at
  once they stacked up the screen, and past a certain number each new one
  landed on top of the last — so the ones underneath were unreachable. At
  most six are now shown at a time; the rest wait their turn and open as
  earlier ones are dismissed or expire. One that has been waiting longer
  than the 90-second staleness window is dropped rather than shown late.

## Internal

- The channels that feed the UI now live in a type the rendering code
  cannot reach. The bug fixed in rc6 — reminders not appearing while kree
  sat in the tray — came from draining those channels in the wrong place,
  and nothing but a comment stopped that from being undone. It is now a
  compile error.
- Previous release notes said reminders are discarded after 30 seconds.
  That is how long a popup stays on screen; the cutoff for discarding one
  that never got shown is 90 seconds.

## Testing this build

Both rc6 fixes are confirmed working, so this release is mainly about the
burst behaviour:

- Set several reminders to the same minute — more than six — and check
  that none of them is lost or hidden behind another, and that the later
  ones appear as the earlier ones go away.
