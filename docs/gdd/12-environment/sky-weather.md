
## Rules: Spell & Ability System

**Design Intent.** SRD 5.2.1–faithful spellcasting and abilities with a thin, deterministic real‑time MMO layer (cast bars, GCD, cooldowns) that preserves SRD math while fitting RoA’s pacing and telemetry needs.

**Player Experience.**

* Classic feel: cast bars, channels, interrupts; Concentration with visible feedback.
* Tooltips show save DC, components (V/S/M), duration, damage types, and tags.
* Auras/buffs/debuffs behave predictably; combat log is clear.

**Rules Fidelity (high level).**

* **Formulas:** Spell Save DC and Spell Attack bonus; Advantage/Disadvantage and conditions per SRD.
* **Components:** Verbal/Somatic/Material with focus/pouch substitution where allowed.
* **Casting Time:** Action/Bonus/Reaction, rituals, long casts; “one slot per turn.”
* **Durations & Concentration:** new concentration breaks old; damage triggers Con save DC 10 or half damage (floor), cap 30.
* **Targeting & Areas:** cone/cube/cylinder/line/sphere; clear path required.
* **Damage Resolution:** roll once for AoE saves; apply half on success where specified.
* **Mitigation Order:** adjusters → resistance → vulnerability; immunity nullifies; **Temporary HP** doesn’t stack and is consumed first.
* SRD attribution and licensing live in the GDD’s existing SRD sections.

**MMO Layer.**

* **Global Cooldown (GCD)** + per‑ability cooldowns.
* Cast/channel states; movement/interrupt policies tuned for RoA but never altering SRD math.
* Threat hooks (damage, healing, taunt) integrate with sim aggro.

**Data & Authoring.**

* **Authoring format:** JSON (or RON) for readability and CI validation; compiled at build time to a compact **binary spellpack** for runtime.
* Stable IDs; content hashes; server/client spellpack hash check at login.
* Authoring schema covers: school, level, lists, components, targeting, effects (op‑codes: MakeSpellAttack, PromptSave, DealDamage, GrantTempHP, ApplyCondition, SpawnArea, ModifyRoll), scaling by slot.

**Runtime & Events.**

* **Pipeline:** intent → validation (resources, components, LoS, range) → cast/channel → resolve effect graph → apply outcomes → emit events.
* **Events:** `CastStarted/Finished/Interrupted`, `SaveRolled`, `DamageDealt/Healed`, `AuraApplied/Removed`, `ConcentrationStarted/Broken`.
* HUD consumes events for cast bars, GCD, auras, combat log.

**Representative Coverage (MVP).**

* Direct attack (Fire Bolt), AoE save/half (Cone of Cold/Fireball pattern), buff with THP + Concentration (Heroism), curse rider (Hex), hybrid attack+AoE (Ice Knife), non‑damage zone with repeated saves (Zone of Truth).

**Performance & Determinism.**

* No dynamic dispatch in hot path; small enum jump table for effect ops.
