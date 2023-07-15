# Flight Safety (docs/FLIGHT_SAFETY.md)

It implements the mission safety logic (state machine) and geofence evaluation.

## States

IDLE → TRANSIT_TO_ZONE → OPERATE_IN_ZONE → RTH → LAND

## Geofence rules

- Transit: within corridor tube (route polyline + width)
- Operate: inside zone polygon
- Max radius cap always enforced

## Triggers

- link loss (uplink)
- GNSS degrade (sat/HDOP/fix age)
- battery low (input hook)
- thermal high (input hook)
- tamper (input hook)
- weather flag (input hook)

It simulates triggers internally and logs transitions.

---

# Flight Safety & Failsafes (docs/FLIGHT_SAFETY.md)

This document defines the safety model for:

- route-to-zone operation (corridor constrained)
- work-in-zone operation (polygon constrained)
- Return-To-Home (RTH) under trouble: link loss, GNSS degrade, battery, thermal, tamper, weather

> Non-goal: “autonomous pursuit anywhere”.
> Navigation is **bounded** and **geofence-enforced**.

---

## Core concepts

- **Home**: configured recovery point (lat/lon/alt)
- **Corridor**: route polyline + width (“tube”) the drone must stay inside
- **Operation zone**: polygon defining allowed work area
- **RTH**: state machine to return safely and land at Home
- **Quality gates**: minimum GNSS quality required to move outward

---

## Mission states (explicit)

1) `IDLE`
2) `LAUNCH_CHECKS`
3) `TRANSIT_TO_ZONE`  (corridor constrained)
4) `OPERATE_IN_ZONE`  (polygon constrained)
5) `RTH`              (corridor constrained by default)
6) `LAND`
7) `ABORT`            (failsafe hold/land)

Every transition is logged (minimal, encrypted logs).

---

## Hard rules (always on)

- In `TRANSIT_TO_ZONE`: position must remain inside corridor tube
- In `OPERATE_IN_ZONE`: position must remain inside zone polygon
- Absolute cap: never exceed `max_radius_m` from Home
- If GNSS is uncertain: **do not move outward**; prefer HOLD/LAND

---

## “Trouble” triggers (examples)

Each trigger is measurable and maps to a conservative action.

### Link loss (LTE/Wi-Fi)

Condition:

- uplink health fails (loss/RTT), or interface down
  Action:
- wait `grace_link_loss_s`, then enter `RTH` (default)

### Battery low

Condition:

- `battery_pct <= battery_low_pct`
  Action:
- enter `RTH` immediately

### Thermal / throttling

Condition:

- `temp >= thermal_soft_c` sustained OR throttling detected
  Action:
- downgrade compute
- if sustained, enter `RTH` (configurable)

### GNSS degraded

Condition examples:

- satellites < `min_gnss_sats`
- HDOP > `max_hdop`
- fix age > `max_fix_age_s`
  Actions (ladder):

1) short degrade: HOLD position (no outward motion)
2) sustained degrade: `RTH` using last-known good path (if safe)
3) severe / no fix: LAND (safest)

### Tamper

Condition:

- enclosure switch, shock profile, manual panic flag
  Action:
- `RTH_IMMEDIATE`
- disable recording (if enabled)
- tighten telemetry (status-only)
- prepare key rotation on recovery

### Weather flag (rain)

Recommendation:

- dedicated sensor or manual operator flag (baro/humidity guesses are weak)
  Action:
- `RTH` (default)

---

## Return-To-Home ladder (conservative)

RTH is a predictable ladder:

1) **Stabilize**

- pause tracking
- reduce compute
- ensure stable attitude via flight controller

2) **Safe altitude** (optional)

- go to `cruise_alt_m` (or configured safe band)

3) **Navigate**

- default: reverse-follow corridor (predictable + bounded)
- optional: direct-to-home only if explicitly allowed and geofence permits

4) **Land**

- land at Home if configured
- else loiter at Home awaiting operator intervention (if enabled)

---

## Bounded “follow target” policy

Follow mode is only permitted when:

- inside zone polygon
- corridor/geofence margins are respected
- battery/thermal margins are healthy
- GNSS fix quality is sufficient

Constraints:

- minimum distance to target
- capped speed/acceleration
- abort-to-hover when confidence drops

---

## Config example (navigation + RTH)

```toml
[nav]
home = { lat = 48.000000, lon = 2.000000, alt_m = 35.0 }
cruise_alt_m = 35.0
max_radius_m = 1200
min_gnss_sats = 9
max_hdop = 1.6
max_fix_age_s = 2

[nav.route]
corridor_width_m = 30
waypoints = [
  { lat = 48.000100, lon = 2.000050 },
  { lat = 48.000600, lon = 2.001000 },
  { lat = 48.001000, lon = 2.002000 }
]

[nav.zone]
zone_polygon = [
  { lat = 48.001100, lon = 2.002100 },
  { lat = 48.001200, lon = 2.002400 },
  { lat = 48.001000, lon = 2.002500 },
  { lat = 48.000900, lon = 2.002200 }
]

[rth]
grace_link_loss_s = 20
battery_low_pct = 22
thermal_soft_c = 75
gnss_bad_fix_s = 8
action_on_tamper = "RTH_IMMEDIATE"
action_on_weather = "RTH"
land_at_home = true
```

# Flight Safety & Failsafes (docs/FLIGHT_SAFETY.md)

This document describes the safety model for **route-to-zone operation** and **Return-To-Home (RTH)**.
The goal is predictable behavior under stress: low battery, bad GNSS, link loss, thermal throttling, weather, tamper.

> Non-goal: “autonomous pursuit anywhere”.
> We keep navigation **bounded** and **geofence-enforced**.

---

## Terms

- **Home**: the configured recovery point (lat/lon/alt)
- **Corridor**: a polyline route with width (a “tube” the drone must stay inside)
- **Operation zone**: polygon defining allowed work area
- **RTH**: a state machine to return safely and land at Home

---

## Mission states

1) `IDLE`
2) `LAUNCH_CHECKS`
3) `TRANSIT_TO_ZONE` (corridor constrained)
4) `OPERATE_IN_ZONE` (polygon constrained)
5) `RTH` (corridor constrained or direct if allowed)
6) `LAND`
7) `ABORT` (failsafe hold/land)

Transitions are explicit and logged (minimal, encrypted logs).

---

## Geofencing rules (hard)

- In `TRANSIT_TO_ZONE`: must stay within corridor tube
- In `OPERATE_IN_ZONE`: must stay inside zone polygon
- Absolute cap: `max_radius_m` from Home is never exceeded
- If fix is uncertain: **do not move outward**; prefer hover/land

---

## Trigger engine (examples)

### Link loss

- condition: uplink health fails (loss/RTT), or interface down
- action: after `grace_link_loss_s` → enter `RTH` (default)

### Battery low

- condition: `battery_pct <= battery_low_pct`
- action: enter `RTH` (immediate)

### Thermal / CPU throttling

- condition: `temp >= thermal_soft_c` sustained OR throttling detected
- action: downgrade compute + possibly enter `RTH` if sustained

### GNSS degraded

- condition: sats < `min_gnss_sats`, HDOP > `max_hdop`, or fix age > `gnss_stale_s`
- action: configurable ladder:
  - short degrade: HOLD position (stop moving outward)
  - sustained degrade: `RTH` if last-good track exists
  - severe / no fix: LAND (safest)

### Tamper

- condition: enclosure switch, accelerometer shock profile, manual “panic”
- action: `RTH_IMMEDIATE` + disable recording + tighten telemetry

### Weather flag (rain)

We recommend a **dedicated sensor** or manual operator flag.

- action: `RTH` (default)

---

## RTH behavior (ladder)

RTH is a state machine with a conservative ladder:

1) **Stabilize**

   - pause tracking
   - reduce compute load
   - ensure attitude stable (via FCU)
2) **Recover altitude** (optional)

   - go to `cruise_alt_m` or a safe altitude band
3) **Navigate**

   - preferred: reverse-follow corridor (predictable)
   - optional: direct-to-home if explicitly allowed and geofence allows
4) **Land**

   - land at Home if configured
   - else loiter at Home until manual takeover (if enabled)

> Default is corridor-based RTH: predictable and bounded.

---

## “Follow target” policy (bounded, safe)

Target following is only permitted when:

- inside zone polygon
- geofence margin is respected
- battery/thermal margins are healthy
- GNSS fix quality is sufficient

Follow mode constraints:

- minimum distance to target
- capped speed/acceleration
- “abort to hover” if confidence drops

---

## What happens when things go wrong (examples)

### GNSS goes bad during transit

- stop outward movement
- hold for short grace window
- if still bad: land or return via last-good path (policy)

### LTE drops

- continue mission briefly if safe
- after grace: RTH

### Device captured

- tamper trigger fires → RTH if possible
- storage and spool remain encrypted
- keys rotate and can be revoked server-side

---

## Checklist before flight (operator)

- verify geofence + route make sense
- run `scout doctor`
- verify RTH triggers configured
- confirm key material present and permissions correct
- ensure camera FPS stable in the chosen mode
- confirm GNSS sats/HDOP meet thresholds

---

## Test philosophy

Every failsafe must be testable in simulation:

- inject GNSS dropout
- inject link loss
- inject battery low
- inject tamper event
- confirm state transitions and resulting commands
