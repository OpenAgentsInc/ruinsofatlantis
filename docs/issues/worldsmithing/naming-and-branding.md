# Naming & Branding — “World Builder” vs Public Labels

Summary
- “World Builder” is very crowded in tabletop/RPG circles and adjacent software, including an official D&D/WotC book, a well‑known charity (“Worldbuilders”), and historic tools named “World Builder.” SEO and trademark clarity are weak if we ship that as the public brand.

Recommendation
- Keep “World Builder” as an internal feature label (menus, flags, issue titles are fine).
- Use a distinct public‑facing name in site/blog/trailers to avoid confusion with WotC and the charity.

Shortlist (safer, on‑brand)
- Atlas Forge — clear world/map connotation; good searchability.
- Aethyr Forge — ties into Aethyr‑ naming in the project.
- Campaign Studio — descriptive, straightforward, clean SEO next to “Ruins of Atlantis.”

Editorial Guidance
- Internal text (engineering/docs): “World Builder,” “builder overlay,” “Place Tree ability.”
- External text (marketing/docs): choose one of the shortlist above and stick to it consistently.

Implementation Notes (docs‑only change)
- Keep feature code/docs using internal naming; add a single sentence where relevant that clarifies public naming will differ.
- Do not change code identifiers or crates for naming alone; treat public label as a presentation‑layer choice.

