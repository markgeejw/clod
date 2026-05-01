# clod

A small Rust CLI that lets you keep multiple [Claude Code](https://docs.claude.com/en/docs/claude-code/overview) accounts side-by-side
(e.g. personal Max + work) and switch between them without logging out and back in. Each profile is its own
[`CLAUDE_CONFIG_DIR`](https://docs.claude.com/en/docs/claude-code/settings) with its own credentials and
session history; skills, plugins, hooks, and `CLAUDE.md` are symlinked back to `~/.claude/` so they stay
in one place and update everywhere at once.

## Install

```bash
cargo install --path .
```

This builds and installs the `clod` binary to `~/.cargo/bin/clod`.

## Quickstart

```bash
clod new personal              # create a profile dir, link shared assets
clod new work
clod switch personal           # set the active profile
clod                           # launch claude with that profile
```

The first time you run against a profile you'll be asked to log in — that login lives in the profile's
own `.credentials.json`, so the two accounts never collide.

## Commands

| Command | Behavior |
| --- | --- |
| `clod init` | Create `~/.clod/` if missing. |
| `clod new <name> [--share-history]` | Create a new profile and link the shared assets from `~/.claude`. With `-S`/`--share-history`, also share session history (see below). |
| `clod ls` | List profiles. The active one is marked with `*`. |
| `clod switch <name>` | Set the active profile. |
| `clod current` | Print the active profile and where it was resolved from. |
| `clod rm <name> [-y]` | Delete a profile. Refuses if it is the active profile. |
| `clod share-history <name> [--force]` | Convert an existing profile to share session history with other share-history profiles. |
| `clod run [-- args…]` | Explicit form — run `claude` with extra args. |
| `clod completions <shell>` | Print a shell completion script (`fish`, `bash`, `zsh`, `elvish`, `powershell`). |

### Forwarding to claude

Anything that isn't a clod subcommand is passed straight through to `claude`:

```bash
clod -p "summarize this"        # → claude -p "summarize this"
clod --resume                   # → claude --resume (session list is profile-scoped)
clod some-prompt.md             # → claude some-prompt.md
```

If you ever need to pass a positional arg to claude that happens to share a name with a clod subcommand
(e.g. literally the word `switch`), use the explicit form: `clod run -- switch`.

### Running with a different profile for one invocation

`--profile <name>` overrides the active profile for a single `clod` invocation, without changing
`~/.clod/active` or `$CLOD_PROFILE`:

```bash
clod --profile work -p "summarize this"   # one-off run as `work`, active stays personal
clod --profile work --resume               # resume a session from `work`
```

It only affects launching `claude`; `clod ls`, `clod current`, etc. still report the persisted state.

## Sharing session history across profiles

By default each profile's session history (the conversation logs `--resume`
reads from, plus the typed-prompt recall list) is profile-local — that's the
whole point of separate `CLAUDE_CONFIG_DIR`s. If you'd rather have profiles
share session history (e.g. so you can switch from `personal` to `work` after
hitting an account limit and pick up where you left off), opt in with
`--share-history`:

```bash
clod new personal --share-history          # fresh profile, sharing from day one
clod new work     --share-history          # joins the same shared store
```

For existing profiles, retrofit one at a time:

```bash
clod share-history personal                # moves personal's history into ~/.clod/shared/
clod share-history work                    # merges work's history into the shared store
```

The first `share-history` call seeds the shared store; later calls *merge* the
profile's existing history into it (no data loss). Sessions are keyed by UUID
so file collisions are extremely unlikely in practice; if one happens you'll
see an error and can pass `--force` to let the profile's copy overwrite the
shared one.

What gets shared (symlinked into `~/.clod/shared/`):

- `projects/` — per-project conversation logs (this is what `claude --resume` lists from)
- `history.jsonl` — the typed-prompt recall list

What stays per-profile:

- `.credentials.json` — each profile keeps its own login.
- `mcp-needs-auth-cache.json`, `cache/`, `backups/`, and any other Claude Code
  bookkeeping not listed above.

Caveat: messages you send after switching profiles count against the
**currently active** profile's account, not whichever profile created the
session. The shared history is just the conversation log on disk.

## Direnv integration

Set `CLOD_PROFILE` in a project's `.envrc` to override the persisted active profile when you're inside
that directory:

```bash
# in your repo's .envrc
export CLOD_PROFILE=work
```

Now `clod` (or `clod -p …` etc.) will use the `work` profile while you're in that directory tree, and
fall back to `~/.clod/active` everywhere else. `clod current` will show which one is in effect and why.

## Shell completions

```bash
clod completions fish > ~/.config/fish/completions/clod.fish
clod completions bash > /usr/local/etc/bash_completion.d/clod
clod completions zsh  > ~/.zsh/completions/_clod
```

The fish output additionally enables dynamic profile-name completion for `clod switch <TAB>` and
`clod rm <TAB>`.

## How profiles work on disk

```
~/.clod/
  active                        # plain text: name of the active profile
  profiles/
    personal/                   # this directory is used as CLAUDE_CONFIG_DIR
      .credentials.json         # real, profile-specific
      projects/ sessions/ todos/ history.jsonl  …    (claude writes these)
      skills      -> ~/.claude/skills            (symlinks to shared assets)
      plugins     -> ~/.claude/plugins
      hooks       -> ~/.claude/hooks
      CLAUDE.md   -> ~/.claude/CLAUDE.md
      settings.json -> ~/.claude/settings.json
    work/
```

`clod` only manages the symlinked entries; everything else inside a profile directory is Claude Code's
own data. If you delete a symlink (or a target doesn't exist when you run `clod new`), nothing in the
profile is corrupted — claude just won't see that asset.

## Active profile resolution

`clod` resolves the active profile in this order:

1. `--profile <name>` (when launching `claude`) — one-off override.
2. `$CLOD_PROFILE` (if non-empty) — the direnv hook.
3. The contents of `~/.clod/active`.
4. Error: no active profile, suggesting `clod switch <name>`.

## Build notes

- MSRV: Rust 1.84. The version pins on `clap`/`clap_complete`/`assert_cmd`/`tempfile` exist because
  newer releases require Rust 1.85+ (`edition2024`). On a newer toolchain you can relax the pins.
- Unix only (Mac and Linux). The implementation uses Unix symlinks and `CommandExt::exec`.

### Building on Nix-on-Mac

If you build under a Nix-managed clang on macOS and hit a linker error about `-liconv`, point cargo at
your Nix-provided libiconv. Create a local-only `.cargo/config.toml` (it is gitignored):

```toml
[env]
LIBRARY_PATH = "/nix/store/<hash>-libiconv-<version>/lib"
```

Replace the path with whatever `find /nix/store -maxdepth 2 -name 'libiconv.dylib'` returns. This is
not needed on standard macOS or Linux toolchains.

## License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or [MIT license](LICENSE-MIT)
at your option.
