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
| `clod new <name>` | Create a new profile and link the shared assets from `~/.claude`. |
| `clod ls` | List profiles. The active one is marked with `*`. |
| `clod switch <name>` | Set the active profile. |
| `clod current` | Print the active profile and where it was resolved from. |
| `clod rm <name> [-y]` | Delete a profile. Refuses if it is the active profile. |
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

1. `$CLOD_PROFILE` (if non-empty) — the direnv hook.
2. The contents of `~/.clod/active`.
3. Error: no active profile, suggesting `clod switch <name>`.

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
