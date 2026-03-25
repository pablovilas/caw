# caw Brand Guide

## Brand Core

- Product name: `caw`
- Expansion: `coding assistant watcher`
- Role: ambient observability for coding agents
- Core metaphor: a crow perched above the work, watching without interrupting
- Brand promise: see every active coding assistant from one quiet control layer

## Positioning

`caw` is not another agent. It is the calm layer around agents. The product should feel observant, precise, and low-drama. It lives closer to a terminal status bar than a noisy chat app.

## Personality

- Watchful
- Quiet
- Exact
- Useful under pressure
- Dense, but never chaotic

## Voice And Tone

- Write in short, factual sentences.
- Prefer "watch", "track", "focus", "group", "session", "status".
- Avoid hype language like "revolutionary", "magical", or "supercharged".
- Prefer grounded microcopy like "No active sessions", "Waiting for input", "Focus terminal".

## Visual Direction

The visual system should feel like:

- a dark operations perch
- terminal density with stronger composition
- quiet confidence instead of bright dashboard energy
- subtle scan lines, separators, and bird-eye motifs

The visual system should avoid:

- playful gradients with no semantic purpose
- glossy SaaS cards
- oversized rounded blobs
- rainbow accents that compete with status colors

## Color System

### Core Neutrals

| Token | Hex | Use |
| --- | --- | --- |
| Raven | `#0B0C0D` | Main canvas, app badge background |
| Coal | `#141618` | Raised surfaces |
| Graphite | `#212529` | Dividers, deep borders |
| Ash | `#5D615D` | Muted labels |
| Mist | `#A7AAA4` | Secondary content |
| Bone | `#EAE6DD` | Primary text on dark backgrounds |

### Status Colors

| Status | Hex | Use |
| --- | --- | --- |
| Working | `#1D9E75` | Active progress, healthy live state |
| Waiting | `#EF9F27` | Needs input, pending attention |
| Idle | `#888780` | Calm inactive state |
| Dead | `#E24B4A` | Failure, disconnected, ended |

### Accent Support

| Token | Hex | Use |
| --- | --- | --- |
| Teal Glow | `#7CC8B2` | Thin highlight only, never full fills |
| Signal Line | `#2A2D2F` | Scan lines, subtle rules |

## Typography

Recommended stack:

- Display: `Space Grotesk`
- UI: `IBM Plex Sans`
- Mono and terminal surfaces: `IBM Plex Mono`

Rules:

- Headlines are compact and slightly tight.
- Numbers and session stats should prefer mono.
- Keep most labels uppercase only for compact technical metadata.
- Do not mix too many weights. The system works best with 400, 500, and 700.

## Shape Language

- Corners: 12px for panels, 20px for marketing cards, 32px plus for app badges
- Lines: thin separators, often horizontal, often low contrast
- Icons: geometric first, illustrative second
- Bird motif: use sparingly as a mnemonic, not wallpaper

## UI Principles

### 1. Ambient first

The product is allowed to sit in the periphery. Put status and counts first. Detail comes on demand.

### 2. Dense but calm

Compact layouts are good. Visual noise is not. Use spacing to separate groups, not oversized containers.

### 3. Color means state

Status colors should encode meaning. Decorative accents should stay neutral or very restrained.

### 4. Motion follows change

Use motion only when a session changes state, groups reorder, or focus shifts. Idle screens should feel still.

### 5. Terminal heritage matters

Keep some monospace rhythm, separators, and plain language. Do not turn the product into a generic dashboard.

## Motion

- Default transitions: 160ms to 240ms
- Screen reveals: 400ms to 700ms with stagger
- Use opacity, vertical nudge, or sweep lines
- Avoid bounce, elastic motion, or constant pulsing

## Accessibility

- Bone on Raven is the default text pairing.
- Keep semantic color paired with text or shape, not color alone.
- Status red should be reserved for failure and dead sessions only.
- Long rows should maintain strong contrast for primary data and softer contrast for metadata.

## Application By Surface

- TUI: use Bone, Ash, and the four status colors; keep separators subtle.
- Tray: minimal text, direct verbs, semantic states only.
- Docs or site: use the crow badge, watchline pattern, and a dark-first layout.
- Social graphics: big lockup, one short sentence, one status line.
