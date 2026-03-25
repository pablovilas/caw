# caw Branding Kit

This folder turns the product's existing cues into a reusable brand system for app UI, docs, site work, and launch assets.

## What is here

- `brand-guide.md` defines the visual direction, tone, and UI/UX rules.
- `copy/messaging.md` contains taglines, product descriptions, and microcopy rules.
- `tokens/caw.tokens.json` is the canonical token export.
- `tokens/caw.css` exposes the same system as CSS variables.
- `tokens/ratatui_palette.rs` gives you a ready-to-copy Rust palette for the TUI.
- `logos/` contains the crow mark, app badge, lockup, and terminal mark.
- `icons/` contains the status symbols as standalone SVG assets.
- `patterns/` contains reusable background motifs.
- `social/og-card.svg` is a launch/social hero card.
- `preview/brand-board.svg` is a quick visual board for review.

## How to use it

- Start with `tokens/caw.tokens.json` if you need a source of truth.
- Use `tokens/caw.css` for any future web, docs, or marketing surface.
- Use `tokens/ratatui_palette.rs` when you want the TUI or tray UI to match the kit.
- Use `logos/caw-badge.svg` for app/store style contexts and `logos/caw-lockup.svg` for headers.
- Keep status colors semantic. Do not reuse working/waiting/dead colors as decoration.

## Notes

- The font recommendations are open source, but font binaries are not bundled here.
- The logo SVGs use text only in the lockup. The crow mark itself is vector artwork.
- The whole kit keeps the current "crow watching from above" concept and strengthens it instead of replacing it.
