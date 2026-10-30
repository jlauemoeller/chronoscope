# Copying and the terminal

`y` copies the selected section as displayed, `Y` copies its full content
(SPEC §6.4). A toast confirms it (`Copied 42 lines`).

chronoscope puts the text on the system clipboard with `arboard`. When there is
no display server (`$DISPLAY` and `$WAYLAND_DISPLAY` are both unset on Linux),
when `$TMUX` is set, or when `arboard` fails, it also writes an OSC52 escape
sequence to the terminal, which most terminals turn into a clipboard write.
That is what makes copying work over SSH.

## tmux

Under tmux the OSC52 sequence is wrapped in tmux's DCS passthrough. tmux must
be allowed to pass it on to the outer terminal. Add to `~/.tmux.conf`:

```
set -g set-clipboard on
set -g allow-passthrough on
```

`Esc` also feels laggy in tmux with its default `escape-time` of 500 ms:

```
set -sg escape-time 10
```

## herdr

herdr's OSC52 behaviour has not been verified yet (SPEC §9.4).
