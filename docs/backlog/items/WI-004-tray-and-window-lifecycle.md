---
id: WI-004
title: Add the tray icon and hide-to-tray window lifecycle
type: feature
status: proposed
branch:
created: 2026-11-03
updated: 2026-11-03
related: [WI-003]
epic:
children: []
---

## Proposal (rationale)

The app is meant to live in the tray (SPEC FR-2, FR-4, FR-12, FR-13), and the skeleton only has a window. Closing the window must release the webview, because the spec's memory limit depends on it (SPEC NFR-1). WI-003 also needs this in order to measure hidden memory.

## Decision

## Plan

## Execution

## Review
