Below is a **developer‑ready movement & camera spec** that recreates World of Warcraft’s default third‑person character controller (keyboard + mouse) closely enough for players to say “this feels like WoW.” I’ve split it into inputs, rules, numeric targets, state & update order, camera, physics, and acceptance tests. Where Blizzard has public values (e.g., speeds, CVars), I cite them. Where Blizzard hasn’t published exact internals (e.g., slope and step thresholds), I give practical, conservative targets and call them out as tunables.

---

## 0) Coordinate system & units

* Right‑handed world coordinates.
* Character yaw rotates around +Y (up).
* **Units:** use meters internally but define WoW‑style targets in **yards** (1 yd = **0.9144 m**).
* Delta‑time based movement/rotation (no frame dependence).

---

## 1) Default input map (keyboard + mouse)

Match WoW defaults (Retail). These are bindings, not behavior yet:

**Keyboard**

| Action                    | Default                                               |
| ------------------------- | ----------------------------------------------------- |
| Move Forward              | `W`                                                   |
| Move Backward (Backpedal) | `S`                                                   |
| Turn Left                 | `A`                                                   |
| Turn Right                | `D`                                                   |
| **Strafe Left**           | `Q`                                                   |
| **Strafe Right**          | `E`                                                   |
| Jump                      | `Space`                                               |
| Toggle Autorun            | `Num Lock` (also allow Mouse4 as an optional default) |
| Toggle Walk/Run           | `Numpad /` (or assignable)                            |

Source for defaults (incl. Q/E strafe, A/D turn, Autorun=NumLock): community‑documented defaults consistent since vanilla. ([frostshock.github.io][1])

**Mouse (hold‑to‑act unless marked “toggle”)**

* **RMB hold** → “mouselook/turn”: yaw & pitch follow mouse deltas; **character yaw follows camera** (you “steer” the character). Widely described as default behavior. ([Blizzard Forums][2])
* **LMB hold** → orbit camera only (character yaw does **not** change). Reported in multiple Blizzard forum threads. ([Blizzard Forums][3])
* **LMB + RMB held together** → **move forward** in camera‑forward, while RMB continues to steer yaw (classic WoW chording). ([bluetracker.gg][4])
* **Middle mouse (Button 3)** → WoW binds “Move & Steer” here by default in several versions (optional in our spec; see §2). ([frostshock.github.io][1])
* **Mouse Wheel** → camera zoom in/out (see camera §6).

**Important modifier rule (WoW muscle memory):**
When **RMB is held**, treat `A` and `D` **as strafes** instead of turn left/right (because turning is already supplied by the mouse). Players and long‑running guides rely on this behavior. ([GameFAQs][5])

---

## 2) Movement speeds (grounded)

Use WoW’s published movement targets (retail, unbuffed, on foot):

* **Run (forward & strafe):** **7.0 yd/s** (≈ **6.4008 m/s**). ([Warcraft Wiki][6])
* **Walk:** **2.5 yd/s** (≈ **2.286 m/s**) when “walk” mode is toggled. ([Wowpedia][7])
* **Backpedal (`S`):** **4.5 yd/s** (≈ **4.1148 m/s**). ([Wowpedia][8])

**Normalization rule (no diagonal speed boost):**
When combining forward/back + strafe, normalize the planar movement vector and then scale by the appropriate speed (run, walk, or backpedal). This matches MMO conventions and prevents >100% speed when holding two directions.

**Instantaneous acceleration:**
WoW has essentially **no inertia** on foot; starts and stops are immediate (feel). Implement as step‑changes to target velocity with optional very small smoothing (≤ 0.05 s) if animation demands it.

---

## 3) Rotation / turning

**Keyboard turning (A/D when RMB not held):**

* Apply a constant yaw rate set by a **Turn Speed** parameter. In WoW this is a CVar (`TurnSpeed`) measured in **degrees/sec**; current default is **~180°/s** (2 s for a full turn), with players referencing the same. Implement `turn_speed_deg_per_s = 180` as default; expose as tunable. ([Warcraft Wiki][9])

**Mouse turning (RMB held):**

* While RMB is held, yaw and pitch follow mouse delta at configurable sensitivities (no explicit cap beyond your sensitivity).
* Character yaw is **slaved to camera yaw** every frame (i.e., you always “face where you look”). This is the defining feel in WoW. ([Blizzard Forums][2])

**Priority rules:**

1. If **RMB held**: mouse controls yaw; `A/D` become strafe; `Q/E` remain strafe.
2. Else if **A/D** pressed without RMB: apply keyboard turn yaw rate (no strafing from A/D).
3. **S** always backpedals (never flips facing).

---

## 4) Autorun & “move & steer”

* **Autorun toggle**: when engaged, behave as if **`W` is continuously down** until canceled by pressing `S` or triggering autorun toggle again. Default key is **Num Lock** (and often bound to Mouse Button 4 by users). ([Wowpedia][10])
* **Both mouse buttons held**: engage forward run (same speed as `W`), and allow RMB to steer. Releasing either button cancels. This is a canonical WoW behavior and should be implemented. ([Blizzard Forums][11])
* **“Move & Steer” (optional convenience action)**: a single button that **simulates “W + RMB hold”** while held (WoW has a binding for this, historically defaulting to Middle Mouse in some configs). Implement as an action you can bind; default unbound is fine, but supporting it increases parity. ([frostshock.github.io][1])

---

## 5) Input → velocity resolution (order of operations)

Run this each frame:

1. **Sample inputs & modes:** `walkToggle`, `autorun`, `RMB`, `LMB`, `W/S/Q/E/A/D`, `Jump`.
2. **Compute intent axes (camera‑relative):**

   * Determine **forward intent**:

     * `+1` if `W` or (`autorun` true) or (`LMB && RMB`)
     * `−1` if `S` (backpedal always wins; cancels autorun just like WoW)
     * `0` otherwise
   * Determine **strafe intent**:

     * Start from `Q/E` (−1 left, +1 right).
     * If `RMB` is held, add `A/D` into strafe (−1/+1).
     * If `RMB` is not held, ignore `A/D` for strafe (they are turn keys).
3. **Choose base speed**:

   * If backpedaling (`S` active, and forward intent ≤ 0): `speed = 4.5 yd/s`.
   * Else if walk mode on: `speed = 2.5 yd/s`.
   * Else: `speed = 7.0 yd/s`. ([Wowpedia][7])
4. **Build planar vector:** `v_local = (strafe_intent, 0, fwd_intent)`.

   * If both `v_local.x` and `v_local.z` are 0 → velocity is zero (unless autorun).
   * Else **normalize** `v_local` and multiply by `speed`.
5. **Transform to world:**

   * If `RMB` held (or Move & Steer active): use **camera yaw** to rotate `v_local` into world.
   * Else: use **character yaw** (so strafing moves relative to facing).
6. **Apply rotation:**

   * If `RMB` held: set `characterYaw = cameraYaw` (each frame).
   * Else: apply keyboard turn (`A/D`) at `±turn_speed_deg_per_s * dt`.
7. **Integrate position:**

   * `pos += v_world * dt` (subject to collision, slope & step correction in §7).
8. **Jump:**

   * If grounded and `Space` pressed, set vertical velocity to `jumpImpulse` (see §7).
9. **Stop conditions:** releasing all movement inputs **immediately zeros** v_local (no inertia).

---

## 6) Camera (third‑person orbit with WoW defaults)

* **Orbit camera** around a pivot slightly above the character’s hips.
* **Zoom:** mouse wheel adjusts distance between `[minZoom, maxZoom]`.

  * Use WoW’s `cameraDistanceMaxZoomFactor` model: `maxZoom = min(39 yd, 15 yd × factor)` with default **factor = 1.9** and allowed **range 1.0–2.6**. Set **default max ≈ 28.5 yd** (15×1.9). Hard cap **39 yd**. ([Wowpedia][12])
* **Follow style (auto‑recenter):** default **“adjust camera only when moving”** (WoW `cameraSmoothStyle = 4`). Expose 0/1/2/4 as options. ([Warcraft Wiki][13])
* **Yaw/pitch sensitivities:** expose as sliders equivalent to WoW’s `cameraYawMoveSpeed` / `cameraPitchMoveSpeed`. Example defaults ~180 and 90 (deg/s per mouse unit), but let the game convert from device deltas. ([CurseForge][14])
* **Pitch limits:** clamp to ~[−80°, +80°] to avoid flipping. WoW exposes a `pitchLimit` CVar and warns about >~88° behaving oddly; keep safer limits by default. ([Wowpedia][15])
* **First‑person at min zoom:** if distance ≤ a small epsilon, switch to a head‑anchored view. WoW does exactly this when you scroll all the way in. (See Blizzard support/tips referencing first‑person toggling via zoom). ([Blizzard Support][16])
* **Occlusion handling:** if line from camera to pivot collides, **push the camera in** along that ray to the hit point minus a small offset (0.2 m) and smoothly restore when clear.

---

## 7) Collision, grounding, slopes, steps, and jump

WoW’s exact physics constants aren’t public. The following values are robust targets that reproduce the “feel”:

* **Character collider:** vertical capsule, **radius 0.35 m**, **height 1.9 m** (humanoid).
* **Grounding:** sphere cast down each frame; snap to floor within **0.2 m**.
* **Slope limit:** **45°** max walkable (tunable 40–50°). Above limit → slide down along surface tangent.
* **Step offset:** allow climbing steps up to **0.45 m** (≈ 18 in).
* **Continuous sweep** (capsule) for movement to prevent tunneling (this style was suggested for MMO‑like character physics). ([GameDev][17])
* **Jump impulse:** choose **vertical takeoff speed ~ 4.5–5.0 m/s** (g ≈ 9.81 m/s²) for a modest hop that feels like WoW’s short jumps. Allow **full yaw control** while airborne (WoW lets you turn in the air).
* **Air control:** keep same planar input vector; apply with reduced traction (e.g., 60–80% of ground speed) or full, depending on how close you want it to feel to WoW; many players perceive near‑full control.

> Note: If you’re shipping an MMO, you’ll likely constrain movement server‑side exactly like WoW (server is authoritative and may cap `TurnSpeed` etc.). For a local prototype, client‑auth is fine.

---

## 8) State machine (high‑level)

**States:** `Grounded`, `Airborne`, `Swimming` (optional later), `Mounted` (optional later).

* **Grounded:** uses §5 rules, applies slope/step, can jump.
* **Airborne:** ignore slope/step, integrate gravity; allow yaw control & planar inputs; land when contacting walkable surface.
* **Swimming (optional later):** replace backpedal speed with symmetric forward/back speed and pitch controls; not required for a first pass.

---

## 9) Frame update pseudocode

```pseudo
function UpdateController(dt):
  readInputs()

  // 1) Modes & modifiers
  RMB = mouse.rightHeld
  LMB = mouse.leftHeld
  autorun = autorun or (LMB && RMB)

  // 2) Intent axes (camera-relative)
  fwd = 0
  if autorun or key.W: fwd += 1
  if key.S:            fwd = -1              // backpedal wins

  strafe = 0
  if key.Q: strafe -= 1
  if key.E: strafe += 1
  if RMB:
     if key.A: strafe -= 1                   // A/D become strafe under RMB
     if key.D: strafe += 1

  // 3) Choose speed bucket
  if key.S and fwd <= 0:
     baseSpeed = 4.5_yd_per_s
  else if walkMode:
     baseSpeed = 2.5_yd_per_s
  else:
     baseSpeed = 7.0_yd_per_s

  // 4) Build local move vector, normalize, scale
  vLocal = Vector3(strafe, 0, fwd)
  if vLocal.lengthSquared > 0:
     vLocal = normalize(vLocal) * baseSpeed
  else:
     vLocal = Vector3.zero

  // 5) Orientation
  if RMB:
     camera.updateFromMouse(dt)              // yaw/pitch from mouse deltas
     character.yaw = camera.yaw              // hard-snap yaw to camera
     vWorld = rotateY(camera.yaw, vLocal)
  else:
     // keyboard turn
     if key.A: character.yaw -= turnSpeed * dt
     if key.D: character.yaw += turnSpeed * dt
     vWorld = rotateY(character.yaw, vLocal)

  // 6) Jump
  if grounded and keyPressed.Space:
     verticalVel = jumpVelocity

  // 7) Integrate & collide (capsule sweep)
  velocity = vWorld + Vector3(0, verticalVel, 0)
  proposed = position + velocity * dt
  position, verticalVel, grounded = collideAndResolveCapsule(position, proposed, settings)

  // 8) Camera follow
  camera.follow(character, style=cameraSmoothStyle, dt)
```

---

## 10) Camera details & constants (to match WoW feel)

* **Max zoom model**:

  * Default `cameraDistanceMaxZoomFactor = 1.9`.
  * Allowed range `1.0–2.6`.
  * Hard cap: **39 yd** regardless of factor (Blizzard hard‑cap). ([Wowpedia][12])
* **Follow style default**: `4` (“adjust only when moving”). ([Warcraft Wiki][13])
* **Keyboard turn rate default**: `TurnSpeed = 180` deg/s (use this to tune “keyboard turning feels right”). ([Warcraft Wiki][9])

---

## 11) Edge‑case rules & quality‑of‑life

* **Cancel autorun** when `S` is pressed (matches WoW). ([Wowpedia][10])
* **RMB vs A/D precedence**: When RMB is down, A/D must **not** apply any yaw; they only strafe (crucial for combat feel). ([GameFAQs][5])
* **First‑person transition**: zooming completely in becomes a head/eyes pivot; zooming out returns to 3P. ([Blizzard Support][16])
* **Immediate stops**: releasing all movement inputs stops the character instantly on ground (no skid).
* **Both buttons move forward**: ensure pressing both LMB+RMB is exactly equivalent to `W` **plus** RMB steering; releasing either cancels forward. ([Blizzard Forums][11])

---

## 12) Exposed tunables (with WoW‑like defaults)

* `runSpeed = 7.0 yd/s`, `walkSpeed = 2.5 yd/s`, `backpedalSpeed = 4.5 yd/s`. ([Wowpedia][7])
* `turnSpeed = 180 deg/s` (keyboard) ; `mouseYawSensitivity`, `mousePitchSensitivity`. ([Warcraft Wiki][9])
* `cameraDistanceMaxZoomFactor = 1.9` (min 1.0, max 2.6; hard cap 39 yd). ([Wowpedia][12])
* `cameraSmoothStyle = 4`. ([Warcraft Wiki][13])
* Physics: `slopeLimitDeg = 45`, `stepOffset = 0.45 m`, `capsuleRadius = 0.35 m`, `capsuleHeight = 1.9 m`, `jumpVelocity ≈ 4.6 m/s`. (These are practical, not Blizzard‑published; mark as tunable.)

---

## 13) Minimum acceptance tests (playtest checklist)

**Keyboard / Mouse basics**

* `W` forward at ~7.0 yd/s; `S` backpedal at ~4.5 yd/s; toggle to walk reduces to ~2.5 yd/s. (Check with debug HUD.) ([Wowpedia][7])
* With **RMB held**, moving mouse left/right **turns the character** immediately; with **LMB held**, camera orbits but **character yaw stays fixed**. ([Blizzard Forums][2])
* Holding **LMB+RMB** moves forward; releasing either stops. ([Blizzard Forums][11])
* With **RMB held**, `A/D` **strafe**; without RMB, `A/D` **turn** at ~180°/s. ([Warcraft Wiki][9])
* Pressing **Num Lock** toggles autorun; pressing **S** cancels autorun. ([Wowpedia][10])

**Diagonal speed**

* Holding `W+Q` (or `W+E`) does **not** exceed 7.0 yd/s; direction is 45° of facing (vector normalized).

**Camera**

* Zoom out limit matches factor×15 yd, clamped to ≤39 yd; default feel ≈28.5 yd at factor 1.9. ([Wowpedia][12])
* Camera follow style “only when moving” matches expectation (no involuntary recentering while idle). ([Warcraft Wiki][13])
* Full zoom‑in switches to first‑person.

**Physics**

* Walk up shallow ramps (≤45°). Slide or fail to ascend on steeper slopes. Step cleanly over 0.3–0.45 m ledges.

---

## 14) Implementation notes (engine‑agnostic)

* **Authoritative camera**: when RMB is held, treat camera yaw as authoritative for character yaw each frame; when released, character yaw persists.
* **Input buffering**: for very short taps (≤50 ms), consider debouncing so a stray click doesn’t produce a micro‑stutter.
* **Networking** (if applicable): send **intent** (forward/back/strafe, RMB flag, turn input) rather than raw deltas; enforce speed caps server‑side.

---

## 15) What we matched to public sources

* Base speeds: run 7 yd/s, walk 2.5, backpedal 4.5. ([Warcraft Wiki][6])
* Autorun default (Num Lock) and common behavior. ([Wowpedia][10])
* RMB=turn, LMB=orbit, both buttons=move forward. ([Blizzard Forums][2])
* Keyboard turn rate exposed via `TurnSpeed`, commonly 180°/s. ([Warcraft Wiki][9])
* Camera max zoom model (`cameraDistanceMaxZoomFactor`, cap 39 yd) and default follow style. ([Wowpedia][12])

---

### Drop‑in constants (in meters/seconds, already converted)

```txt
RUN_SPEED = 6.4008   // 7.0 yd/s
WALK_SPEED = 2.2860  // 2.5 yd/s
BACKPEDAL_SPEED = 4.1148 // 4.5 yd/s
TURN_SPEED_DEG = 180.0
CAPSULE_RADIUS = 0.35
CAPSULE_HEIGHT = 1.90
SLOPE_LIMIT_DEG = 45.0
STEP_OFFSET = 0.45
JUMP_VELOCITY = 4.6
MAX_ZOOM_YARDS = min(39, 15 * cameraDistanceMaxZoomFactor)  // default factor 1.9
```

---

If you want, I can translate this spec into a Unity or Unreal implementation skeleton (same behavior, engine‑specific APIs) in a single script/class—just say which engine you’re targeting.

[1]: https://frostshock.github.io/wabc/hotkeys.html?utm_source=chatgpt.com "HotKeys (Key Bindings, Keyboard Shortcuts) in Vanilla WoW"
[2]: https://us.forums.blizzard.com/en/wow/t/rotate-camera-by-holding-right-mouse-button/996285?utm_source=chatgpt.com "Rotate Camera by holding Right Mouse Button"
[3]: https://us.forums.blizzard.com/en/wow/t/mouse-right-click-move-left-click-camera-look-not-working/688121?utm_source=chatgpt.com "Mouse Right click move / Left click camera look not working"
[4]: https://www.bluetracker.gg/wow/topic/eu-en/88312-turn-off-click-both-mouse-buttons-to-move/?utm_source=chatgpt.com "Turn off click both mouse buttons to move"
[5]: https://gamefaqs.gamespot.com/boards/534914-world-of-warcraft/62503311?utm_source=chatgpt.com "Are you supposed to bind A & D to strafe or turn? - GameFAQs"
[6]: https://warcraft.wiki.gg/wiki/Speed?utm_source=chatgpt.com "Speed - Warcraft Wiki"
[7]: https://wowpedia.fandom.com/wiki/Movement?utm_source=chatgpt.com "Movement - Wowpedia - Your wiki guide to the World of Warcraft"
[8]: https://wowpedia.fandom.com/wiki/Speed?utm_source=chatgpt.com "Speed - Wowpedia - Your wiki guide to the World of Warcraft"
[9]: https://warcraft.wiki.gg/wiki/CVar_TurnSpeed?utm_source=chatgpt.com "CVar TurnSpeed - Warcraft Wiki"
[10]: https://wowpedia.fandom.com/wiki/Autorun?utm_source=chatgpt.com "Autorun - Wowpedia - Your wiki guide to the World of Warcraft"
[11]: https://eu.forums.blizzard.com/en/wow/t/pressing-your-2-mouse-buttons-is-another-way-to-move-your-character/278733?utm_source=chatgpt.com "Pressing your 2 mouse buttons is another way to move ..."
[12]: https://wowpedia.fandom.com/wiki/CVar_cameraDistanceMaxZoomFactor?utm_source=chatgpt.com "CVar cameraDistanceMaxZoomFactor - Wowpedia - Fandom"
[13]: https://warcraft.wiki.gg/wiki/CVar_cameraSmoothStyle?utm_source=chatgpt.com "CVar cameraSmoothStyle - Warcraft Wiki"
[14]: https://www.curseforge.com/wow/addons/camerapitchyaw?utm_source=chatgpt.com "CameraPitchYaw - World of Warcraft Addons"
[15]: https://wowpedia.fandom.com/wiki/CVar_pitchLimit?utm_source=chatgpt.com "CVar pitchLimit - Wowpedia - Fandom"
[16]: https://us.support.blizzard.com/en/article/18631?utm_source=chatgpt.com "Cannot Zoom World of Warcraft Camera Out or In"
[17]: https://www.gamedev.net/forums/topic/711080-how-to-implement-wow-like-physics/?utm_source=chatgpt.com "How to implement WoW-like physics"
