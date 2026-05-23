use galax_core::{
    build_p2p_lists, build_constrained, compute_fmm_force, InteractionLists, BodiesSoA, Tree,
};
use galax_gpu::{GpuConfig, GpuContext};
use galax_init::{disk, galaxy, plummer, uniform};
use std::sync::Arc;
use wasm_bindgen::prelude::*;
use web_sys::CanvasRenderingContext2d;

use crate::renderer::{render_frame, Camera, WorldBounds};

fn map_gpu_err(e: galax_gpu::GpuError) -> JsValue {
    JsValue::from_str(&format!("{}", e))
}

const NEEDED_STORAGE_BUFFERS: u32 = 9;

enum SimBackend {
    Gpu {
        gpu_ctx: GpuContext,
        tree: Tree,
        m2l_lists: InteractionLists,
        p2p_lists: InteractionLists,
        dt: f64,
    },
    Cpu {
        tree: Tree,
        m2l_lists: InteractionLists,
        p2p_lists: InteractionLists,
        p: usize,
        softening: f64,
        dt: f64,
    },
}

pub struct SimState {
    pub bodies: BodiesSoA,
    pub step_count: u64,
    pub n: usize,
    pub preset: String,
    backend: SimBackend,
}

fn compute_dt(bodies: &BodiesSoA) -> f64 {
    let ma = bodies
        .ax
        .iter()
        .zip(bodies.ay.iter())
        .map(|(x, y)| (x * x + y * y).sqrt())
        .fold(0.0_f64, f64::max);
    if ma > 1e-30 {
        0.5 / ma.sqrt()
    } else {
        0.01
    }
}

fn choose_init(preset: &str, bodies: &mut BodiesSoA) {
    let n = bodies.len();
    match preset {
        "plummer" => plummer(bodies, n),
        "disk" => disk(bodies, n),
        "galaxy" => galaxy(bodies, n),
        _ => uniform(bodies, n),
    }
}

async fn build_gpu_backend(
    n: usize,
    preset: &str,
    p_order: usize,
    eps: f32,
) -> Result<(SimBackend, BodiesSoA), JsValue> {
    let mut bodies = BodiesSoA::new(n);
    choose_init(preset, &mut bodies);

    let tree = build_constrained(&mut bodies, 32, -50.0, 50.0, -50.0, 50.0);
    let m2l_lists = InteractionLists::build(&tree);
    let p2p_lists = build_p2p_lists(&tree);

    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
    let adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: None,
            force_fallback_adapter: false,
        })
        .await
        .map_err(|e| JsValue::from_str(&format!("no adapter: {}", e)))?;

    let device_limits = adapter.limits();
    if device_limits.max_storage_buffers_per_shader_stage < NEEDED_STORAGE_BUFFERS {
        return Err(JsValue::from_str(&format!(
            "need {} storage buffers, device supports {}",
            NEEDED_STORAGE_BUFFERS, device_limits.max_storage_buffers_per_shader_stage,
        )));
    }

    let (device, queue) = adapter
        .request_device(
            &wgpu::DeviceDescriptor {
                label: Some("galax-wasm"),
                required_features: wgpu::Features::empty(),
                required_limits: device_limits,
                ..Default::default()
            },
        )
        .await
        .map_err(|e| JsValue::from_str(&format!("device request: {}", e)))?;

    let cfg = GpuConfig {
        max_n: n.max(1024) as u32,
        max_nodes: tree.nodes.len().max(1024) as u32,
        max_interactions: (tree.nodes.len() * 64).max(4096) as u32,
        p: p_order as u32,
        eps,
    };
    let mut gpu_ctx = GpuContext::new(Arc::new(device), Arc::new(queue), cfg)
        .map_err(|e| JsValue::from_str(&format!("GPU init: {}", e)))?;
    gpu_ctx.upload_tree_data(&tree, &m2l_lists, &p2p_lists);

    gpu_ctx.step(&bodies);
    gpu_ctx.read_accelerations_async(&mut bodies.ax, &mut bodies.ay).await.map_err(map_gpu_err)?;

    let dt = compute_dt(&bodies);

    Ok((
        SimBackend::Gpu {
            gpu_ctx,
            tree,
            m2l_lists,
            p2p_lists,
            dt,
        },
        bodies,
    ))
}

async fn build_cpu_backend(
    n: usize,
    preset: &str,
    p_order: usize,
    eps: f32,
) -> Result<(SimBackend, BodiesSoA), JsValue> {
    let mut bodies = BodiesSoA::new(n);
    choose_init(preset, &mut bodies);

    let tree = build_constrained(&mut bodies, 32, -50.0, 50.0, -50.0, 50.0);
    let m2l_lists = InteractionLists::build(&tree);
    let p2p_lists = build_p2p_lists(&tree);
    let softening = eps as f64;

    compute_fmm_force(&mut bodies, &tree, &m2l_lists, &p2p_lists, p_order, softening);
    let dt = compute_dt(&bodies);

    Ok((
        SimBackend::Cpu {
            tree,
            m2l_lists,
            p2p_lists,
            p: p_order,
            softening,
            dt,
        },
        bodies,
    ))
}

impl SimState {
    pub async fn new(n: usize, preset: &str, p_order: usize, eps: f32) -> Result<SimState, JsValue> {
        let (backend, bodies) = match build_gpu_backend(n, preset, p_order, eps).await {
            Ok(ok) => ok,
            Err(_) => build_cpu_backend(n, preset, p_order, eps).await?,
        };

        Ok(SimState {
            bodies,
            step_count: 0,
            n,
            preset: preset.to_string(),
            backend,
        })
    }

    pub async fn step(&mut self) -> Result<(), JsValue> {
        let dt = match &self.backend {
            SimBackend::Gpu { dt, .. } => *dt,
            SimBackend::Cpu { dt, .. } => *dt,
        };
        let n = self.bodies.len();

        for i in 0..n {
            self.bodies.vx[i] += self.bodies.ax[i] * dt / 2.0;
            self.bodies.vy[i] += self.bodies.ay[i] * dt / 2.0;
        }
        for i in 0..n {
            self.bodies.x[i] += self.bodies.vx[i] * dt;
            self.bodies.y[i] += self.bodies.vy[i] * dt;
        }

        match &mut self.backend {
            SimBackend::Gpu { gpu_ctx, .. } => {
                gpu_ctx.step(&self.bodies);
                gpu_ctx.read_accelerations_async(&mut self.bodies.ax, &mut self.bodies.ay).await.map_err(map_gpu_err)?;
            }
            SimBackend::Cpu { tree, m2l_lists, p2p_lists, p, softening, .. } => {
                compute_fmm_force(&mut self.bodies, tree, m2l_lists, p2p_lists, *p, *softening);
            }
        }

        for i in 0..n {
            self.bodies.vx[i] += self.bodies.ax[i] * dt / 2.0;
            self.bodies.vy[i] += self.bodies.ay[i] * dt / 2.0;
        }

        self.step_count += 1;
        Ok(())
    }

    pub async fn resize(&mut self, n: usize, preset: &str) -> Result<(), JsValue> {
        let p_order = 4;
        let eps = 0.1;
        let (backend, bodies) = match build_gpu_backend(n, preset, p_order, eps).await {
            Ok(ok) => ok,
            Err(_) => build_cpu_backend(n, preset, p_order, eps).await?,
        };
        self.bodies = bodies;
        self.backend = backend;
        self.n = n;
        self.preset = preset.to_string();
        self.step_count = 0;
        Ok(())
    }
}

#[wasm_bindgen]
pub struct SimStateHandle {
    inner: SimState,
    camera: Camera,
    bounds: WorldBounds,
}

#[wasm_bindgen]
pub async fn create_sim_state(n: usize, preset: &str) -> Result<SimStateHandle, JsValue> {
    let state = SimState::new(n, preset, 4, 0.1).await?;
    Ok(SimStateHandle {
        inner: state,
        camera: Camera::new(),
        bounds: WorldBounds::new(),
    })
}

#[wasm_bindgen]
impl SimStateHandle {
    pub async fn step(&mut self) -> Result<(), JsValue> {
        self.inner.step().await
    }

    pub async fn resize(&mut self, n: usize, preset: &str) -> Result<(), JsValue> {
        self.inner.resize(n, preset).await
    }

    pub fn body_count(&self) -> usize {
        self.inner.n
    }

    pub fn step_count(&self) -> u64 {
        self.inner.step_count
    }

    pub fn camera_zoom(&mut self, delta: f64, mx: f64, my: f64, w: f64, h: f64) {
        self.camera.zoom_at(delta, mx, my, w, h);
    }

    pub fn camera_pan(&mut self, dx: f64, dy: f64, w: f64, h: f64) {
        self.camera.pan(dx, dy, w, h);
    }

    pub fn render(&self, ctx: &CanvasRenderingContext2d, width: u32, height: u32, elapsed_ms: f64, steps_per_sec: f64) -> Result<(), JsValue> {
        render_frame(
            ctx,
            width,
            height,
            &self.inner.bodies,
            &self.camera,
            &self.bounds,
            self.inner.step_count,
            elapsed_ms,
            steps_per_sec,
        )
    }
}
