---
id: WI-001
title: Prove the power assertion on all three platforms
type: spike
status: proposed
branch:
created: 2026-11-03
updated: 2026-11-03
related: [WI-003, ADR-0001]
epic:
children: []
---

## Proposal (rationale)

The whole product depends on holding and releasing an OS power assertion, and on the OS dropping it if the process dies (SPEC PB-3). `keepawake` is the candidate crate and hasn't been tried on any platform here. Replace `NoopAdapter` for this capability and check SPEC acceptance criteria 2 and 6 on Windows, macOS and Linux.

## Decision

## Plan

## Execution

## Review
