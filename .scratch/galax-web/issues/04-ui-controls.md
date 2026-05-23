Status: ready-for-agent

## Parent

`.scratch/galax-web/PRD.md`

## What to build

Add HTML/CSS/JS UI controls for interactive simulation configuration. All controls live in HTML/CSS/JS, not in WASM. JS reads control values and calls `#[wasm_bindgen]` functions on the WASM side.

Controls:
- **Body count slider**: range input (min ~1000, max ~262144, step any). On change, calls `SimState::resize(n, preset)` on WASM side — triggers full re-init (new Bodies, new tree, new interaction lists, GPU context re-upload).
- **Init preset dropdown**: options for "uniform", "plummer", "disk", "galaxy". On change, calls `SimState::resize(n, preset)` — re-initialises Bodies with the selected generator, preserves N.
- **Pause/resume button**: toggles the `requestAnimationFrame` loop between stepping the simulation vs. freezing the current frame.

Style: dark theme, control bar positioned at the bottom or top of the page, semi-transparent, non-overlapping with the main canvas area.

## Acceptance criteria

- [ ] Body count slider re-initialises the simulation at the new N with the current preset
- [ ] Init preset dropdown switches the initial condition generator and re-initialises
- [ ] Pause/resume button freezes and resumes the simulation loop
- [ ] Controls are usable and responsive during active simulation
- [ ] Style is consistent with a dark theme

## Blocked by

- `.scratch/galax-web/issues/03-canvas-rendering.md`
