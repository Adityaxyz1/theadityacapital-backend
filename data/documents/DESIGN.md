---
name: The Aditya Capital CRM
description: A calm, warm insurance book-of-business CRM — restrained neutrals with one accent color drawn from the firm's own logo.
colors:
  primary: "#2E9D57"
  primary-hover: "#227C43"
  primary-deep: "#1A6335"
  primary-tint: "#EFFBF3"
  logo-mark-green: "#3EFF7A"
  neutral-bg: "#FAF9F6"
  neutral-surface: "#FFFEFB"
  neutral-border: "#E8E3D9"
  neutral-border-strong: "#D8D0C0"
  neutral-text-muted: "#8C8271"
  neutral-text: "#26211B"
  status-pending: "#F59E0B"
  status-danger: "#F43F5E"
  status-info: "#3B82F6"
typography:
  display:
    fontFamily: "'Plus Jakarta Sans Variable', ui-sans-serif, system-ui, sans-serif"
    fontWeight: 800
    letterSpacing: "-0.02em"
  body:
    fontFamily: "'Plus Jakarta Sans Variable', ui-sans-serif, system-ui, sans-serif"
    fontWeight: 500
  label:
    fontFamily: "'JetBrains Mono Variable', ui-monospace, monospace"
    letterSpacing: "0.04em"
rounded:
  sm: "8px"
  md: "12px"
  lg: "14px"
  xl: "20px"
  pill: "9999px"
spacing:
  sm: "8px"
  md: "16px"
  lg: "20px"
components:
  button-primary:
    backgroundColor: "{colors.primary}"
    textColor: "#FFFFFF"
    rounded: "{rounded.pill}"
    padding: "9px 20px"
  button-primary-hover:
    backgroundColor: "{colors.primary-hover}"
  card:
    backgroundColor: "{colors.neutral-surface}"
    rounded: "{rounded.lg}"
    padding: "20px"
---

# Design System: The Aditya Capital CRM

## Overview

**Creative North Star: "The Quiet Ledger"**

This is a daily-use book-of-business tool for insurance agency staff, not a marketing surface performing for a visitor — so the visual language is restrained by design: a warm, paper-toned neutral ground carries almost the entire interface, with one accent spent sparingly on the things that actually need to be found first (primary actions, active states, the one number on a screen that matters most). That accent is a muted, legible green pulled directly from the firm's own logo mark (a neon-green "ac" monogram) rather than an unrelated brand color — brand and product now agree with each other. The calm, restrained rendering of that green (versus the logo's own vivid neon) keeps the identity from the earlier terracotta pass: confident, unhurried, still fully at home doing dense daily data work — the color changed, the restraint did not.

Confirmed visual rejections: no gradient-filled text, no glass/blur used as decoration (translucency, backdrop blur, or a "liquid glass" surface with no functional reason), no zero-offset colored glow halos standing in for depth, no decorative hairline grid backgrounds, no small-caps eyebrow/kicker above a heading, no "hero-metric" stat-tile grid standing in for real proof.

**Key Characteristics:**
- Warm neutral paper background and surfaces, not cool blue-gray
- One accent color — a muted green derived from the logo's own hue — used deliberately and rarely
- The logo mark itself keeps its more vivid, authentic neon-green rendering; the rest of the app uses the muted version, so the two read as clearly related without the UI itself feeling neon
- Flat, softly-shadowed cards — depth from real offset+blur, never a colored halo
- Motion is quiet and purposeful: one authored hero entrance on the landing page, a single consistent scroll-reveal for every card/section below it, and hover/tap feedback on every interactive control — never decoration added just because a section exists
- Functional status colors (pending/danger/info) stay outside the brand accent family so they read as system state, not brand expression

## Colors

The palette is Restrained: neutrals carry the interface, one accent — derived from the logo — signals action and emphasis.

### Primary
- **Signal Green** (`#2E9D57`): The one accent color, muted/darkened from the logo mark's own vivid `#3EFF7A` for legible everyday UI use. Primary buttons, active nav/tab state, links, focus rings, checkmarks/"active" indicators, and the rare highlighted data point (a chart's current-period line). Never used for large background fields — it is a mark, not a wash.
- **Signal Green Deep** (`#227C43`): Hover/pressed state for primary actions — always darker than the resting accent, never lighter.
- **Signal Green Forest** (`#1A6335`): Deep-tint accent text on a light green background, where `#2E9D57` alone would be too light for body-text contrast.
- **Signal Green Whisper** (`#EFFBF3`): The lightest tint, used only as a background behind accent-colored text/icons (badge fills, tinted icon chips) — never as a page or card background.

### Neutral
- **Paper** (`#FAF9F6`): Page background. Warm, not stark white, not cool gray.
- **Surface** (`#FFFEFB`): Card and elevated-surface background — a hair lighter/warmer than paper so cards still read as distinct without a hard white/gray split.
- **Warm Border** (`#E8E3D9`): Default card/input border and hairline dividers.
- **Warm Border Strong** (`#D8D0C0`): Hover-state border, or a border that needs to read as more present (e.g. an active input).
- **Ink Muted** (`#8C8271`): Secondary/muted text — labels, timestamps, helper copy.
- **Ink** (`#26211B`): Primary text and headings. Warm near-black, not cool slate.

### Status (functional, not brand)
- **Pending Amber** (`#F59E0B`): "Pending"/"awaiting action" states (renewal pipeline, claims).
- **Danger Rose** (`#F43F5E`): Overdue, lapsed, lost, rejected, or otherwise needs-attention states.
- **Info Blue** (`#3B82F6`): Neutral informational state (e.g. "contacted", in-review).

### The logo mark
- **Logo Mark Green** (`#3EFF7A`): The actual `AdityaCapitalLogo` artwork's own vivid neon green (a fixed brand asset per PRODUCT.md) — kept more vivid than the muted Signal Green used everywhere else in the UI, so the real mark still reads as authentically itself. Its tile's border/glow/hover motion is keyed to this exact value; the rest of the app uses the muted derivative above instead.

### Named Rules
**The One Voice Rule.** The accent appears on a small minority of any given screen — a page with more than one or two accent-colored regions has over-spent it. Its rarity is what makes it findable.

**The Muted Mark Rule.** The UI accent and the logo mark share one hue family on purpose, but never the same value: the logo stays vivid/neon (it's the real artwork), the UI stays muted (it has to be legible as text and buttons all day). Matching the exact neon value anywhere outside the logo tile is a mistake, not a bolder choice.

**The No-Glow Rule.** Depth comes from a real shadow (offset + blur, always in a warm near-black, never colored). A symmetric, zero-offset colored halo around an element is decoration, not depth, and does not appear anywhere in this system.

## Typography

**Display/Body Font:** Plus Jakarta Sans Variable (with `ui-sans-serif, system-ui, sans-serif` fallback)
**Label/Mono Font:** JetBrains Mono Variable (with `ui-monospace, monospace` fallback)

**Character:** A single warm geometric sans carries both display and body duty — this is an Operate-mode, daily-use tool, so legibility and density outrank typographic personality. Mono is reserved for genuinely tabular/code-like data: policy numbers, currency figures in dense tables, timestamps, uppercase micro-labels.

*Carried over unchanged from the incumbent build — this redesign's brief was color/surface treatment, not typography. Flagged by the project's own slop detector as an overused face; revisiting type is a reasonable follow-up but was out of scope here.*

### Hierarchy
- **Display** (800 weight, `clamp(38px, 5.6vw, 62px)` on the landing hero only, tight `-0.03em` tracking): Marketing-surface hero headline only.
- **Headline** (800 weight, 22–36px): Section headings, card titles. Carries its own weight — never preceded by a small-caps eyebrow/kicker (see Do's and Don'ts).
- **Body** (500 weight, 12–15px): Table cells, paragraph copy, form labels.
- **Label** (700–800 weight, 10–12px, `0.04–0.14em` tracking, uppercase): Status pills and table headers only — mono where the content itself is tabular data. Not used as a section-heading preamble.

### Named Rules
**The No-Kicker Rule.** No small-caps eyebrow label ever sits above a heading purely to add editorial weight — a heading that needs one hasn't been written strongly enough yet. Uppercase tracked labels are reserved for status pills and table headers, where they name real data, not for decorating a headline.

## Layout

Dense, table-first layouts for the authenticated app (Policies, Renewals, Claims, Payments, Reports) — this redesign did not change spacing, grid, or breakpoints, only color and surface treatment. The landing page keeps its existing bento-style feature grid (12-column, spans of 4/8) and section rhythm; its hero was rebuilt around one primary CTA + one quiet secondary link (not two competing pill buttons) and dropped the abstract stat-tile grid in favor of letting the interactive product demo prove capability directly.

## Elevation & Depth

Flat-by-default: cards sit on the paper background with a hairline warm border and a soft, real (offset + blur) shadow — never a colored glow. Hover states darken the border slightly and add a touch more shadow spread plus a small upward translate; they never introduce a colored halo.

### Shadow Vocabulary
- **Resting card** (`box-shadow: 0 1px 2px rgba(38,33,27,0.04), 0 8px 20px -6px rgba(38,33,27,0.06)`): Default `.crm-card` shadow.
- **Hover card** (`box-shadow: 0 2px 4px rgba(38,33,27,0.05), 0 12px 28px -8px rgba(38,33,27,0.1)`): `.crm-card-hover:hover` — paired with a border-color shift and `translateY(-2px)`.
- **Floating surface** (`box-shadow: 0 1px 2px rgba(38,33,27,0.04), 0 16px 32px -12px rgba(38,33,27,0.08)`): Modals, the pill navbar, elevated tiles.

### Named Rules
**The Warm Shadow Rule.** Every shadow in this system is cast in the neutral ink color (`rgba(38,33,27,…)`), never in the accent color. A shadow tinted with the brand accent reads as a glow effect, which this system does not use.

## Shapes

Soft, consistent radii throughout: `8px` for small controls (inputs, small buttons), `12–14px` for cards and standard buttons, `20px` for feature/hero cards, full pill (`9999px`) for status badges, nav pills, and primary CTAs. Borders are always `1px`, in the warm neutral border color — never a colored or multi-pixel accent border on the edge of a card or list item.

## Components

### Buttons
- **Shape:** Pill (`9999px`) for primary CTAs and nav actions; `12px` rounded rectangle for in-table/toolbar buttons.
- **Primary:** Solid Signal Green (`#2E9D57`) background, white text.
- **Hover/Focus:** Background darkens to Signal Green Deep (`#227C43`); focus-visible gets a 2px Signal Green outline with 2px offset (themed globally via `:focus-visible`, not per-component).
- **Secondary/Ghost:** Warm Surface background, Warm Border, Ink text; hover darkens the border to Warm Border Strong.

### Status Pills
- **Style:** Small pill or rounded-rect badge, tinted background at ~10–15% of the status color, full-strength status color for text/icon, matching-hue border at low opacity.
- **State:** One pill per record status (pending/contacted/renewed/lapsed/lost, or claim equivalents) — status color is fixed per state, never re-themed by context.

### Cards / Containers
- **Corner Style:** `14px` standard (`.crm-card`), `20px` for feature/hero cards.
- **Background:** Warm Surface (`#FFFEFB`).
- **Shadow Strategy:** See Elevation & Depth — resting/hover pair, never a glow.
- **Border:** `1px solid` Warm Border, darkening to Warm Border Strong on hover where interactive.

### Inputs / Fields
- **Style:** `12px` rounded rectangle, `1px` Warm Border, white/Surface background.
- **Focus:** Border shifts to Signal Green, plus a 1px Signal Green focus ring — no glow.

### AIC's Copilot (signature component)
The AI assistant panel is deliberately the surface that should feel most like Claude's own chat UI: a slide-out drawer, user messages as solid Signal Green bubbles (right-aligned), assistant replies as Warm Surface bubbles with a hairline border (left-aligned, full-width for tables), and a "Grounded" status pill signaling the assistant answers from live data rather than invented figures.

## Do's and Don'ts

### Do:
- **Do** spend the accent on primary actions, active/selected state, and the single most important data point on a screen — nowhere else.
- **Do** keep status colors (pending amber / danger rose / info blue) visually distinct from the brand accent so record state is never confused with brand emphasis.
- **Do** cast every shadow in warm neutral ink, with a real offset and blur.
- **Do** give every interactive control (buttons, tabs, chips, links) some hover/tap feedback — the primary-CTA spring lift is the baseline vocabulary, not an exception reserved for a few buttons.
- **Do** give below-the-fold sections a scroll-triggered arrival (fade + short rise, fires once) so content unfolds consistently as the visitor scrolls — driven from one shared place (`TiltCard`'s own entrance), not re-invented per section.
- **Do** wrap pages with meaningful motion in `MotionConfig reducedMotion="user"` so `prefers-reduced-motion` is honored automatically.
- **Do** let a landing page's real, working product demo carry the "proof" burden instead of abstract stat tiles or claimed metrics.

### Don't:
- **Don't** use gradient-filled text for emphasis — weight or size carries emphasis instead.
- **Don't** reach for glass/blur/translucency as decoration. `.liquid-glass-card`/`.liquid-glass-pill` are solid warm surfaces now, kept only as class names for backward compatibility with existing markup.
- **Don't** add a colored, zero-offset glow halo around any element to suggest depth or "AI magic."
- **Don't** reintroduce a decorative hairline grid background outside an actual map/blueprint/canvas surface.
- **Don't** put a small-caps eyebrow/kicker above a heading — see The No-Kicker Rule under Typography.
- **Don't** use the "hero-metric" template (big number, small label, icon, repeated in a grid) as a stand-in for proof — it reads as generic AI-template filler; show the real product instead.
- **Don't** give two CTAs equal visual weight in the same row. One primary action per decision point; everything else is a quieter link.
