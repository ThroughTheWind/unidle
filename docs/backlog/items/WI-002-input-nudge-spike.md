---
id: WI-002
title: Prove pointer and keystroke nudges on all three platforms
type: spike
status: proposed
branch:
created: 2026-11-03
updated: 2026-11-03
related: [ADR-0001]
epic:
children: []
---

## Proposal (rationale)

Nudges must have no visible effect (SPEC FR-7, FR-9) and need the macOS Accessibility permission and a Wayland fallback (SPEC PB-2). `enigo` is the candidate crate and its Wayland support is experimental. Check SPEC acceptance criteria 3, 4, 5 and 7, including whether real user input can be detected so nudges can be skipped (FR-8).

## Decision

## Plan

## Execution

## Review
