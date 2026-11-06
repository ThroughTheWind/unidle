---
id: ADR-0004
title: The command line drives the tray app over a local channel
status: accepted
date: 2026-11-04
supersedes:
superseded_by:
---

# ADR-0004 — The command line drives the tray app over a local channel

## Context

Phase 4 adds `unidle on | off | status | timer` (SPEC §13). A CLI has to answer two questions: who holds the power assertion, and how a second command (`unidle off`, `unidle status`) reaches the first. The [work-machine review](https://claude.ai/artifact/QJMnhPPt683HrLk9RpuwKY) adds a constraint: a CLI must not become a hidden way to run Unidle, because an invisible keep-awake process is exactly what looks like evasion on a managed machine. The app is also a GUI-subsystem program on Windows, so a long-running foreground CLI there cannot own a terminal properly.

## Decision

The CLI is the same binary, and it never holds the assertion itself. Each command is one JSON request and one JSON response over a local channel to the running tray app: a Unix socket in a user-private folder, or a Windows named pipe created with `FILE_FLAG_FIRST_PIPE_INSTANCE` and `PIPE_REJECT_REMOTE_CLIENTS`. `on` and `timer` start the app in the tray when none answers. The transport lives in `unidle-core` (`remote`), with direct OS calls as in ADR-0002 and ADR-0003.

## Consequences

**Good**

- Every ON shows in the tray and stops on quit; there is one owner of the assertion and of SC-5 precedence.
- The same signed binary; nothing new to install, sign or approve.
- No network, no new dependency on Windows or Linux (`libc` on Unix for `getuid` only).

**Costs and risks**

- No headless use: a Linux server without a desktop can't run the tray app, so it can't use the CLI. A headless mode would be a separate item and has to stay visible in its terminal.
- On Windows the shell may print its prompt before the CLI's output, because the binary is a window program that joins the terminal late.
- A same-user process can send commands. That is the same trust as the user's own terminal.
- The Windows pipe uses the default security descriptor; another user could only reach it locally and only read, which the protocol never answers.

## Alternatives considered

**Forward arguments through `tauri-plugin-single-instance`** — the plugin already hands a second launch's arguments to the first, but it can't send an answer back, so `status` and error reporting are impossible, and a first launch would turn the terminal into the app.

**A foreground CLI that holds the assertion itself** until Ctrl+C — simplest on macOS and Linux, but it bypasses the tray (two owners, no SC-5 precedence) and doesn't work on Windows without a second console binary.

**A localhost TCP port** — reachable by every local user and indistinguishable from network access in a firewall prompt, against NFR-2.

**The `interprocess` crate** — a cross-platform local socket, but its namespaced Linux sockets are abstract (no file permissions), and the direct calls are about 150 lines.

## Revisit triggers

1. A headless Linux mode is accepted onto the roadmap.
2. A user report that the Windows output ordering breaks a script.
3. The app gains a second process (a helper or service) that should own the assertion.

## Admission check

Against [the rule](README.md): (1) constrains future work ✅ · (2) a real alternative was rejected with a stated reason ✅ · (3) reversing later costs meaningfully more ✅ · (4) not owned by another document ✅ · (5) not an implementation detail ✅.
