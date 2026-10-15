# 0001 — Record decisions in the repository

- Status: accepted
- Date: 2026-10-15
- Milestone: M0

## Context

OxiGuitar is built across many separate working sessions between the maintainer and an AI engineer, plus outside contributors. None of them can rely on remembering earlier conversations. The project brief holds the plan; the repository has to hold what was actually decided and why.

## Decision

Every significant decision gets a short Architecture Decision Record in `docs/adr/`, numbered in order and written from `0000-template.md`. ROADMAP.md carries the live milestone status. A record is never rewritten after it is accepted; a later record supersedes it.

Decisions already taken in the project brief (15 October 2026) stand without separate records: the name, GPL-3.0, pure Rust, the supported systems and packages, the file-format list with .gp as the default save format, the milestone order, the Guitar Pro 8-like interface with our own artwork, no telemetry ever, and an MSRV that follows Debian unstable's rustc.

## Consequences

Anyone starting a session reads ROADMAP.md and the latest records instead of asking. Records cost a few minutes each; we skip them for small, easily reversed choices.
