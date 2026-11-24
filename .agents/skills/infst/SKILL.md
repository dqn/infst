---
name: infst
description: Develop or operate infst, including the Windows tracker, INFINITAS score synchronization, automatic startup, memory-layout compatibility, and the web service.
---

# infst

Use the checkout containing this skill. Read the relevant sections of
[CLAUDE.md](../../../CLAUDE.md) and the local instruction files before changing
code; use [README.md](../../../README.md) for supported user workflows.

## Windows operations

For the user's Windows PC, use the `windows-ssh` skill to obtain the connection
target. The primary checkout is `%USERPROFILE%\dev\infst`; confirm that its Git
remote is `dqn/infst`. Execute PowerShell through SSH with a UTF-16LE
`-EncodedCommand` when quoting or Unicode paths would otherwise cross shells.
Read repository text as UTF-8 in Windows PowerShell 5.1.

Identify the active game and tracker with process name, PID, session ID, and
executable path. Omit game command lines from output: INFINITAS can put its
authentication token in its launch arguments. Resolve executable paths from the
running tracker or its startup configuration; a dirty checkout may differ from
the installed binary.

## Synchronize scores

When the user asks to sync while INFINITAS is running, synchronize play data to
the web service. For repository synchronization, inspect Git state instead.

Find the active `bm2dx` PID and run the configured tracker executable in its
usual working directory:

```powershell
& $trackerPath sync --pid $game.Id
if ($LASTEXITCODE -ne 0) { throw 'Score synchronization failed' }
```

Use existing credentials without printing tokens. Inspect `sync --help` for the
selected binary when its interface is uncertain. Completion requires a
successful server response and CLI exit status; report the uploaded entry count.
A cache can limit uploads to changed entries. Keep the game running and its
foreground window undisturbed during synchronization.

## Automatic startup

Use [scripts/register-autostart.ps1](../../../scripts/register-autostart.ps1)
and [scripts/autostart.ps1](../../../scripts/autostart.ps1) as the maintained
implementation. Register the actual executable and working directory, then
verify the task and watcher in the interactive desktop session.

The launch contract is a minimized console with `SW_SHOWMINNOACTIVE`, preserving
game focus. The watcher mutex and task settings prevent duplicate launches.
PowerShell's ordinary `-WindowStyle Minimized` can activate a window and is not
an equivalent replacement. Check the task state and autostart log when tracking
does not start. A task registered successfully can later stop, so distinguish
registration from a live watcher and tracker.

Use the regression scripts in `scripts/tests` for startup changes. Run the focus
test on an unlocked interactive desktop outside active gameplay; SSH session 0
cannot establish the user's foreground-window behavior.

## Development and Git

Follow `AGENTS.local.md`, when present, for Windows temporary directories and the
MSVC linker before invoking Cargo. For memory-layout work, read
[docs/memory-layouts.md](../../../docs/memory-layouts.md) and use its fixtures and
live-game validation procedure. Determine commands and feature flags from the
current repository rather than copying old offsets or build assumptions.

Other tasks may update this checkout concurrently. Re-read branch tips and
working changes before a Git mutation, preserve existing work, and honor the
configured merge/rebase preference. Confirm published SHAs against the remote
before reporting a push; distinguish remote publication from local alignment.

If Windows-to-GitHub SSH stalls, bound connection attempts. An authenticated Mac
can transfer the committed change with a Git bundle and push from an isolated
checkout. Re-read the remote base before integration, preserve the exact intended
patch, and verify the resulting remote SHA before updating Windows tracking refs.
