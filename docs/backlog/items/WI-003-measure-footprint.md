---
id: WI-003
title: Measure memory and installer size against the spec limits
type: spike
status: proposed
branch:
created: 2026-11-03
updated: 2026-11-03
related: [WI-004, ADR-0001]
epic:
children: []
---

## Proposal (rationale)

The system webview is the main risk to the 80 MB memory limit (SPEC NFR-1), and ADR-0001 is reopened if the limit can't be met. Measure the whole process tree on each OS with the window open, and with the webview closed while hidden in the tray. Do this before building much UI on top.

First numbers, Windows 11 only, 2026-11-03, from a throwaway build of the skeleton: installer 1.05 MiB; window open 155 MB private and 338 MB working set; webview destroyed 6 MB private and 27 MB working set. macOS and Linux are unmeasured. See F-002.

## Decision

## Plan

## Execution

## Review
