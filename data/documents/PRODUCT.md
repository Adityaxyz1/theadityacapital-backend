# Product

<!-- impeccable:product-schema 1 -->

## Platform

web

## Users

Internal staff of The Aditya Capital, an insurance agency/wealth-management brokerage — not customers or the public. Three roles with a strict visibility hierarchy: **Admin** (sees the entire book of business), **Manager** (sees their team's book), **Agent** (sees only records assigned to them). The assistant panel greets the logged-in user as "Director," suggesting the primary daily user is agency leadership/ownership as well as working agents.

## Product Purpose

An insurance policy CRM that replaces spreadsheet/manual tracking for managing the agency's full book of business: customers, policies, renewals, claims, payments, leads, and follow-up activity. Success means staff can see what's coming due, who owes what, and what needs action today, without reconciling separate spreadsheets.

## Positioning

Purpose-built for an insurance agency's actual workflow (policy renewal cadence, claims settlement tracking, insurer-by-insurer premium book) rather than a generic CRM retrofitted with insurance fields — renewal risk, premium-at-stake, and claims-aging are first-class views, not custom fields bolted onto a sales pipeline.

## Operating Context

Daily desk use by agency staff, primarily on desktop browsers (no evidence of a packaged mobile/native app). Core workflows: track policies through their renewal lifecycle (pending → contacted → renewed/lapsed/lost); process claims through settlement; log payments; manage leads; upload/extract policy documents (PDF/image/Excel/HTML) via an AI extraction pipeline; ask an embedded AI copilot ("AIC's Copilot") natural-language questions about the live book of business, grounded in real data via tool calls (never invented figures). A live WebSocket layer pushes real-time updates (renewal board, notifications).

## Capabilities and Constraints

- Domains: Customers, Policies, Renewals (Kanban board), Claims, Payments, Leads, Activities, Calendar, Documents/Upload, Reports, Admin (users/teams), plus a Landing/marketing page and Login.
- Row-level visibility enforced server-side by role (Admin/Manager/Agent), not just hidden in the UI.
- AI features: document data-extraction (single + bulk) and a conversational assistant, both backed by NVIDIA NIM models, calling back into the Rust API as the actual asking user (never an elevated system account) so results respect visibility rules.
- Currency is Indian Rupees (₹); dates and figures are India-market insurance data (insurers like New India Assurance, Bajaj General, ICICI Lombard, HDFC Ergo, etc.).
- Existing visual identity is undocumented (no DESIGN.md yet) — an emerald/slate Tailwind theme with glassmorphism/gradient flourishes on the landing page; this redesign treats that look as evidence, not a constraint to preserve.

## Brand Commitments

The actual logo mark (`frontend/src/assets/aditya-capital-logo.jpg`, an "ac" monogram) is a fixed neon-green-on-near-black artwork — a real, confirmed brand asset, never to be recolored. The app's own UI accent (DESIGN.md) is a muted, legible derivative of this exact green, so brand and product agree — but the logo tile itself always renders at its own more vivid, authentic value, never diluted to the muted UI shade.

## Evidence on Hand

Live local MongoDB with real extracted policy/customer/renewal/claim data (not seed/lorem-ipsum placeholders) — screens should be designed against how that real, sometimes-imperfect data actually looks (long insurer names, long policy numbers, ₹ figures of varying magnitude), not idealized sample content.

## Product Principles

1. Operate mode throughout the authenticated app — staff completing tasks (checking a renewal, filing a claim, correcting a policy) outrank marketing-style expression; the Landing/Login pages are the one Persuade-adjacent exception.
2. Grounded over generic — every AI-touched surface (copilot, extraction) must visibly earn trust that figures are real, not decorative "AI magic."
3. Density with clarity — this is a book-of-business tool used daily; scanability and fast comprehension of tables/lists matter more than whitespace-heavy marketing polish.
4. Role-aware by default — Admin/Manager/Agent see different slices of the same screens; the design should read cleanly at every scope, not just the Admin "sees everything" case.

## Accessibility & Inclusion

No explicit standard confirmed yet; treat standard WCAG AA contrast/focus-visible expectations as the baseline for a daily-use internal tool until told otherwise.
