# galax-viz

Interactive N-body visualization.

Renders bodies as pixels in a window using `minifb`. Supports camera
controls (drag to pan, scroll to zoom) and a live HUD overlay.

## Usage

```bash
# Live simulation
cargo run -p galax-viz -- --n 10000 --init disk --gpu

# Snapshot replay
cargo run -p galax-viz -- --snap path/to/snapshot.galx

# CPU with FMM
cargo run -p galax-viz -- --n 50000 --fmm
```

### Controls

| Input | Action |
|-------|--------|
| Scroll wheel | Zoom in / out |
| Left-click drag | Pan camera |
| Escape | Exit |

### Key options

| Flag | Default | Description |
|------|---------|-------------|
| `--n` | 500 | Number of bodies |
| `--init` | uniform | Initial condition preset |
| `--gpu` | — | GPU acceleration |
| `--fmm` | — | Force FMM (N² auto-switches at 20000) |
| `--n2` | — | Force direct N² |
| `--dt` | auto | Timestep |
| `--target-fps` | 30 | Target render framerate |
| `--sim-speed` | 100 | Target simulation steps/second |
| `--width` | 1280 | Window width |
| `--height` | 720 | Window height |
| `--max-render` | — | Subsample bodies for rendering |
