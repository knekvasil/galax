use std::borrow::Cow;
use std::mem;
use std::sync::Arc;

use galax_core::{BodiesSoA, InteractionLists, Tree};

#[repr(C)]
#[derive(Copy, Clone)]
struct SimParamsRaw {
    n: u32,
    num_nodes: u32,
    num_leaves: u32,
    p: u32,
    eps: f32,
    m2l_num_indices: u32,
    current_level: u32,
    p2p_num_indices: u32,
}

const UNIFORM_PADDED_SIZE: u64 = 512;

pub struct GpuConfig {
    pub max_n: u32,
    pub max_nodes: u32,
    pub max_interactions: u32,
    pub p: u32,
    pub eps: f32,
}

impl Default for GpuConfig {
    fn default() -> Self {
        Self {
            max_n: 262_144,
            max_nodes: 524_288,
            max_interactions: 1 << 24,
            p: 4,
            eps: 0.1,
        }
    }
}

#[derive(Debug)]
pub enum GpuError {
    InitFailed(String),
    ShaderCompilation(String),
    BufferAllocation(String),
}

impl std::fmt::Display for GpuError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GpuError::InitFailed(msg) => write!(f, "GPU init failed: {}", msg),
            GpuError::ShaderCompilation(msg) => write!(f, "shader compilation: {}", msg),
            GpuError::BufferAllocation(msg) => write!(f, "buffer allocation: {}", msg),
        }
    }
}

impl std::error::Error for GpuError {}

fn bytes_of<T>(val: &T) -> &[u8] {
    let size = mem::size_of::<T>();
    unsafe { std::slice::from_raw_parts(val as *const T as *const u8, size) }
}

fn bytes_of_slice<T>(val: &[T]) -> &[u8] {
    let size = val.len().checked_mul(mem::size_of::<T>()).unwrap();
    unsafe { std::slice::from_raw_parts(val.as_ptr() as *const u8, size) }
}

fn compact_interaction_lists(lists: &InteractionLists) -> (Vec<u32>, Vec<u32>) {
    let n_nodes = lists.lengths.len();
    let mut offsets = Vec::with_capacity(n_nodes + 1);
    let mut indices = Vec::new();
    offsets.push(0u32);
    for node in 0..n_nodes {
        let len = lists.lengths[node] as usize;
        for &partner in lists.data[node][..len].iter() {
            if partner == u32::MAX {
                break;
            }
            indices.push(partner);
        }
        offsets.push(indices.len() as u32);
    }
    (indices, offsets)
}

fn build_leaf_id(tree: &Tree) -> Vec<u32> {
    let n = tree.nodes.iter().filter(|n| n.is_leaf).fold(0usize, |max, n| max.max(n.body_end));
    let mut leaf_id = vec![0u32; n];
    for (node_id, node) in tree.nodes.iter().enumerate() {
        if node.is_leaf {
            for i in node.body_start..node.body_end {
                leaf_id[i] = node_id as u32;
            }
        }
    }
    leaf_id
}

fn build_leaf_ranges(tree: &Tree) -> Vec<[u32; 2]> {
    let mut ranges = vec![[0u32, 0u32]; tree.nodes.len()];
    for (node_id, node) in tree.nodes.iter().enumerate() {
        if node.is_leaf {
            ranges[node_id] = [node.body_start as u32, node.body_end as u32];
        }
    }
    ranges
}

// ── Kernel polynomial precomputation ────────────────────────────

fn build_kernel_polys(p: usize, eps: f32) -> Vec<Vec<Vec<f32>>> {
    let order = 2 * p;
    let stride = order + 1;
    let eps2 = eps * eps;
    let idx = |k: usize, l: usize| k * stride + l;
    let mut poly = vec![vec![vec![0.0f32; stride]; stride]; stride * stride];
    poly[idx(0, 0)][0][0] = -1.0;
    for n in 1..=order {
        for k in 0..=n {
            let l = n - k;
            let mut coeff = vec![vec![0.0f32; stride]; stride];
            if k > 0 {
                let prev = &poly[idx(k - 1, l)];
                let m = (1 + 2 * (k - 1) + 2 * l) as f32;
                for a in 0..stride {
                    for b in 0..(stride - a) {
                        let c = prev[a][b];
                        if c == 0.0 { continue; }
                        if a > 0 {
                            let dc = c * a as f32;
                            if a + 1 < stride { coeff[a + 1][b] += dc; }
                            if b + 2 < stride { coeff[a - 1][b + 2] += dc; }
                            coeff[a - 1][b] += dc * eps2;
                        }
                    }
                }
                for a in 0..stride {
                    for b in 0..(stride - a) {
                        let c = prev[a][b];
                        if c == 0.0 { continue; }
                        if a + 1 < stride { coeff[a + 1][b] -= m * c; }
                    }
                }
            } else if l > 0 {
                let prev = &poly[idx(k, l - 1)];
                let m = (1 + 2 * k + 2 * (l - 1)) as f32;
                for a in 0..stride {
                    for b in 0..(stride - a) {
                        let c = prev[a][b];
                        if c == 0.0 { continue; }
                        if b > 0 {
                            let dc = c * b as f32;
                            if a + 2 < stride { coeff[a + 2][b - 1] += dc; }
                            if b + 1 < stride { coeff[a][b + 1] += dc; }
                            coeff[a][b - 1] += dc * eps2;
                        }
                    }
                }
                for a in 0..stride {
                    for b in 0..(stride - a) {
                        let c = prev[a][b];
                        if c == 0.0 { continue; }
                        if b + 1 < stride { coeff[a][b + 1] -= m * c; }
                    }
                }
            }
            poly[idx(k, l)] = coeff;
        }
    }
    poly
}

fn flatten_deriv_coeffs(polys: &[Vec<Vec<f32>>], order: usize, stride: usize) -> Vec<f32> {
    let n_derivs = (order + 1) * (order + 2) / 2;
    let mut flat = vec![0.0f32; n_derivs * stride * stride];
    let idx = |k: usize, l: usize| {
        let n = k + l;
        n * (n + 1) / 2 + l
    };
    for k in 0..=order {
        for l in 0..=(order - k) {
            let di = idx(k, l);
            let poly = &polys[k * stride + l];
            for a in 0..stride {
                for b in 0..(stride - a) {
                    let c = poly[a][b];
                    if c != 0.0 {
                        flat[di * stride * stride + a * stride + b] = c;
                    }
                }
            }
        }
    }
    flat
}

fn compute_node_levels(tree: &Tree) -> Vec<u32> {
    let root_hw = tree.nodes.last().map(|n| n.half_width).unwrap_or(50.0);
    let mut levels = Vec::with_capacity(tree.nodes.len());
    for node in &tree.nodes {
        let lvl = (root_hw / node.half_width).log2().round() as u32;
        levels.push(lvl);
    }
    levels
}

fn build_children_array(tree: &Tree) -> Vec<[i32; 4]> {
    let mut arr = Vec::with_capacity(tree.nodes.len());
    for node in &tree.nodes {
        let mut c = [-1i32, -1, -1, -1];
        for (i, child) in node.children.iter().enumerate() {
            if let Some(id) = child {
                c[i] = *id as i32;
            }
        }
        arr.push(c);
    }
    arr
}

fn build_centers_array(tree: &Tree) -> Vec<[f32; 2]> {
    tree.nodes.iter().map(|n| [n.center_x as f32, n.center_y as f32]).collect()
}

// ── GpuContext ───────────────────────────────────────────────────

pub struct GpuContext {
    pub(crate) device: Arc<wgpu::Device>,
    pub(crate) queue: Arc<wgpu::Queue>,
    config: GpuConfig,

    // Preallocated buffers
    body_data_buffer: wgpu::Buffer,
    dynamics_buffer: wgpu::Buffer,
    expansion_buffer: wgpu::Buffer,
    m2l_data_buffer: wgpu::Buffer,
    p2p_data_buffer: wgpu::Buffer,
    leaf_data_buffer: wgpu::Buffer,
    centers_buffer: wgpu::Buffer,
    children_buffer: wgpu::Buffer,
    deriv_coeffs_buffer: wgpu::Buffer,
    uniform_buffer: wgpu::Buffer,
    readback_buffer: wgpu::Buffer,

    // Pipelines (all share the same bind group layout and bind group)
    bind_group: wgpu::BindGroup,
    pipelines: Vec<wgpu::ComputePipeline>, // [p2m, m2m, m2l, l2l, l2p, p2p]

    // Cached level information for M2M/L2L dispatch
    level_ranges: Vec<(u32, u32)>,  // (start, end) for each level
    max_level: u32,
    num_leaves: u32,
    num_nodes: u32,
    m2l_num_indices: u32,
    p2p_num_indices: u32,

    // Sizes
    _dynamics_byte_size: u64,
    _readback_byte_size: u64,
    _expansion_byte_size: u64,
    stride: usize,
}

impl GpuContext {
    pub fn new(
        device: Arc<wgpu::Device>,
        queue: Arc<wgpu::Queue>,
        config: GpuConfig,
    ) -> Result<Self, GpuError> {
        let stride = (config.p as usize + 1) * (config.p as usize + 2) / 2;
        let body_data_byte_size = config.max_n as u64 * 16; // vec4<f32> = 16 bytes per body
        let dynamics_byte_size = config.max_n as u64 * 8;
        let expansion_byte_size = config.max_nodes as u64 * stride as u64 * 4 * 2;
        let combined_mi_size = (config.max_interactions as u64 + config.max_nodes as u64 + 1) * 4; // indices + offsets
        let leaf_data_byte_size = config.max_n.max(config.max_nodes) as u64 * 16; // vec4<u32> = 16 bytes per entry
        let centers_size = config.max_nodes as u64 * 8;
        let children_size = config.max_nodes as u64 * 16;
        let n_derivs = (2 * config.p as usize + 1) * (2 * config.p as usize + 2) / 2;
        let deriv_coeffs_size = n_derivs as u64 * stride as u64 * stride as u64 * 4;
        let readback_byte_size = dynamics_byte_size;
        let copy_alignment = 256u64;

        fn create_buf(d: &wgpu::Device, label: &str, size: u64, usage: wgpu::BufferUsages) -> wgpu::Buffer {
            d.create_buffer(&wgpu::BufferDescriptor { label: Some(label), size, usage, mapped_at_creation: false })
        }

        let body_data_buffer = create_buf(&device, "body_data", body_data_byte_size, wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST);
        let dynamics_buffer = create_buf(&device, "dynamics", dynamics_byte_size, wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC | wgpu::BufferUsages::COPY_DST);
        let expansion_buffer = create_buf(&device, "expansions", expansion_byte_size, wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST);
        let m2l_data_buffer = create_buf(&device, "m2l_data", combined_mi_size, wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST);
        let p2p_data_buffer = create_buf(&device, "p2p_data", combined_mi_size, wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST);
        let leaf_data_buffer = create_buf(&device, "leaf_data", leaf_data_byte_size, wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST);
        let centers_buffer = create_buf(&device, "centers", centers_size, wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST);
        let children_buffer = create_buf(&device, "children", children_size, wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST);
        let deriv_coeffs_buffer = create_buf(&device, "deriv_coeffs", deriv_coeffs_size, wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST);
        let uniform_buffer = create_buf(&device, "uniform", UNIFORM_PADDED_SIZE, wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST);
        let readback_buffer = create_buf(&device, "readback", (readback_byte_size + copy_alignment - 1) & !(copy_alignment - 1), wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST);

        // ── Shader modules ────────────────────────────────────
        let p2m_src = include_str!(concat!(env!("OUT_DIR"), "/p2m.wgsl"));
        let m2m_src = include_str!(concat!(env!("OUT_DIR"), "/m2m.wgsl"));
        let m2l_src = include_str!(concat!(env!("OUT_DIR"), "/m2l.wgsl"));
        let l2l_src = include_str!(concat!(env!("OUT_DIR"), "/l2l.wgsl"));
        let l2p_src = include_str!(concat!(env!("OUT_DIR"), "/l2p.wgsl"));
        let p2p_src = include_str!(concat!(env!("OUT_DIR"), "/p2p.wgsl"));
        let n2_src = include_str!(concat!(env!("OUT_DIR"), "/n2_debug.wgsl"));
        let shader_srcs = [p2m_src, m2m_src, m2l_src, l2l_src, l2p_src, p2p_src, n2_src];
        let shader_names = ["p2m", "m2m", "m2l", "l2l", "l2p", "p2p", "n2_debug"];
        let mut shader_modules = Vec::new();
        for (src, name) in shader_srcs.iter().zip(shader_names.iter()) {
            let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some(name),
                source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(src)),
            });
            shader_modules.push(module);
        }

        // ── Unified bind group layout (9 storage + 1 uniform) ──
        let bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("fmm_unified"),
            entries: &[
                wgpu::BindGroupLayoutEntry { binding: 0, visibility: wgpu::ShaderStages::COMPUTE, ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Storage { read_only: true }, has_dynamic_offset: false, min_binding_size: None }, count: None },
                wgpu::BindGroupLayoutEntry { binding: 1, visibility: wgpu::ShaderStages::COMPUTE, ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Storage { read_only: false }, has_dynamic_offset: false, min_binding_size: None }, count: None },
                wgpu::BindGroupLayoutEntry { binding: 2, visibility: wgpu::ShaderStages::COMPUTE, ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Storage { read_only: false }, has_dynamic_offset: false, min_binding_size: None }, count: None },
                wgpu::BindGroupLayoutEntry { binding: 3, visibility: wgpu::ShaderStages::COMPUTE, ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: false, min_binding_size: None }, count: None },
                wgpu::BindGroupLayoutEntry { binding: 4, visibility: wgpu::ShaderStages::COMPUTE, ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Storage { read_only: true }, has_dynamic_offset: false, min_binding_size: None }, count: None },
                wgpu::BindGroupLayoutEntry { binding: 5, visibility: wgpu::ShaderStages::COMPUTE, ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Storage { read_only: true }, has_dynamic_offset: false, min_binding_size: None }, count: None },
                wgpu::BindGroupLayoutEntry { binding: 6, visibility: wgpu::ShaderStages::COMPUTE, ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Storage { read_only: true }, has_dynamic_offset: false, min_binding_size: None }, count: None },
                wgpu::BindGroupLayoutEntry { binding: 7, visibility: wgpu::ShaderStages::COMPUTE, ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Storage { read_only: true }, has_dynamic_offset: false, min_binding_size: None }, count: None },
                wgpu::BindGroupLayoutEntry { binding: 8, visibility: wgpu::ShaderStages::COMPUTE, ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Storage { read_only: true }, has_dynamic_offset: false, min_binding_size: None }, count: None },
                wgpu::BindGroupLayoutEntry { binding: 9, visibility: wgpu::ShaderStages::COMPUTE, ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Storage { read_only: true }, has_dynamic_offset: false, min_binding_size: None }, count: None },
            ],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("fmm"),
            bind_group_layouts: &[Some(&bgl)],
            immediate_size: 0,
        });

        let mut pipelines = Vec::new();
        for (module, name) in shader_modules.into_iter().zip(shader_names.iter()) {
            let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some(name),
                layout: Some(&pipeline_layout),
                module: &module,
                entry_point: Some("main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                cache: None,
            });
            pipelines.push(pipeline);
        }

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("fmm"),
            layout: &bgl,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding { buffer: &body_data_buffer, offset: 0, size: None }) },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding { buffer: &dynamics_buffer, offset: 0, size: None }) },
                wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding { buffer: &expansion_buffer, offset: 0, size: None }) },
                wgpu::BindGroupEntry { binding: 3, resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding { buffer: &uniform_buffer, offset: 0, size: None }) },
                wgpu::BindGroupEntry { binding: 4, resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding { buffer: &m2l_data_buffer, offset: 0, size: None }) },
                wgpu::BindGroupEntry { binding: 5, resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding { buffer: &p2p_data_buffer, offset: 0, size: None }) },
                wgpu::BindGroupEntry { binding: 6, resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding { buffer: &leaf_data_buffer, offset: 0, size: None }) },
                wgpu::BindGroupEntry { binding: 7, resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding { buffer: &centers_buffer, offset: 0, size: None }) },
                wgpu::BindGroupEntry { binding: 8, resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding { buffer: &children_buffer, offset: 0, size: None }) },
                wgpu::BindGroupEntry { binding: 9, resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding { buffer: &deriv_coeffs_buffer, offset: 0, size: None }) },
            ],
        });

        Ok(Self {
            device, queue, config,
            body_data_buffer, dynamics_buffer, expansion_buffer,
            m2l_data_buffer, p2p_data_buffer, leaf_data_buffer,
            centers_buffer, children_buffer, deriv_coeffs_buffer,
            uniform_buffer, readback_buffer,
            bind_group, pipelines,
            level_ranges: Vec::new(),
            max_level: 0,
            num_leaves: 0, num_nodes: 0,
            m2l_num_indices: 0, p2p_num_indices: 0,
            _dynamics_byte_size: dynamics_byte_size,
            _readback_byte_size: readback_byte_size,
            _expansion_byte_size: expansion_byte_size,
            stride,
        })
    }

    pub fn upload_tree_data(&mut self, tree: &Tree, m2l_lists: &InteractionLists, p2p_lists: &InteractionLists) {
        // Compute and upload derivative coefficients
        let p = self.config.p as usize;
        let eps = self.config.eps;
        let order = 2 * p;
        let stride = order + 1;
        let polys = build_kernel_polys(p, eps);
        let flat_coeffs = flatten_deriv_coeffs(&polys, order, stride);
        self.queue.write_buffer(&self.deriv_coeffs_buffer, 0, bytes_of_slice(&flat_coeffs));

        // M2L lists (combined: indices then offsets)
        let (m2l_idx, m2l_off) = compact_interaction_lists(m2l_lists);
        {
            let mut combined = m2l_idx.clone();
            combined.extend_from_slice(&m2l_off);
            self.queue.write_buffer(&self.m2l_data_buffer, 0, bytes_of_slice(&combined));
            self.m2l_num_indices = m2l_idx.len() as u32;
        }

        // P2P lists (combined: indices then offsets)
        let (p2p_idx, p2p_off) = compact_interaction_lists(p2p_lists);
        {
            let mut combined = p2p_idx.clone();
            combined.extend_from_slice(&p2p_off);
            self.queue.write_buffer(&self.p2p_data_buffer, 0, bytes_of_slice(&combined));
            self.p2p_num_indices = p2p_idx.len() as u32;
        }

        // Leaf data: combined leaf_id + leaf_ranges + node_levels
        {
            let leaf_id = build_leaf_id(tree);
            let leaf_ranges = build_leaf_ranges(tree);
            let node_levels = compute_node_levels(tree);
            let n_body = leaf_id.len();
            let n_node = tree.nodes.len();
            let buf_len = n_body.max(n_node);
            let mut flat: Vec<u32> = vec![0u32; buf_len * 4];
            for i in 0..n_body { flat[i * 4] = leaf_id[i]; }
            for j in 0..n_node {
                flat[j * 4 + 1] = leaf_ranges[j][0];
                flat[j * 4 + 2] = leaf_ranges[j][1];
                flat[j * 4 + 3] = node_levels[j];
            }
            self.queue.write_buffer(&self.leaf_data_buffer, 0, bytes_of_slice(&flat));
        }

        // Node centers
        let centers = build_centers_array(tree);
        self.queue.write_buffer(&self.centers_buffer, 0, bytes_of_slice(&centers));

        // Node children
        let children = build_children_array(tree);
        self.queue.write_buffer(&self.children_buffer, 0, bytes_of_slice(&children));

        // Node levels for M2M/L2L level-by-level dispatch
        let node_levels = compute_node_levels(tree);

        // Build level ranges
        self.max_level = *node_levels.iter().max().unwrap_or(&0);
        let num_nodes = tree.nodes.len();
        let mut level_ranges = vec![(num_nodes as u32, 0u32); self.max_level as usize + 1];
        for (i, &lvl) in node_levels.iter().enumerate() {
            let l = lvl as usize;
            let ii = i as u32;
            if ii < level_ranges[l].0 { level_ranges[l].0 = ii; }
            if ii >= level_ranges[l].1 { level_ranges[l].1 = ii + 1; }
        }
        self.level_ranges = level_ranges;
        self.num_leaves = tree.nodes.iter().filter(|n| n.is_leaf).count() as u32;
        self.num_nodes = num_nodes as u32;
    }

    pub fn step(&self, bodies: &BodiesSoA) {
        let n = bodies.len() as u32;
        let nn = self.num_nodes;
        let nl = self.num_leaves;
        let stride = self.stride as u32;
        let expansion_sz = nn * stride;

        let p = self.config.p;
        let eps = self.config.eps;
        let m2l_ni = self.m2l_num_indices;
        let p2p_ni = self.p2p_num_indices;

        // ── Upload body data (x, y, mass) ─────────────────────────
        {
            let mut flat: Vec<f32> = Vec::with_capacity(bodies.len() * 4);
            for i in 0..bodies.len() {
                flat.push(bodies.x[i] as f32);
                flat.push(bodies.y[i] as f32);
                flat.push(bodies.mass[i] as f32);
                flat.push(0.0); // pad for vec4
            }
            self.queue.write_buffer(&self.body_data_buffer, 0, bytes_of_slice(&flat));
        }
        // ── Zero expansions ───────────────────────────────────────
        {
            let zeros: Vec<f32> = vec![0.0f32; (expansion_sz * 2) as usize];
            self.queue.write_buffer(&self.expansion_buffer, 0, bytes_of_slice(&zeros));
        }
        // ── Zero accelerations ────────────────────────────────────
        {
            let zeros: Vec<f32> = vec![0.0f32; n as usize * 2];
            self.queue.write_buffer(&self.dynamics_buffer, 0, bytes_of_slice(&zeros));
        }

        // ── Write uniform params helper ──────────────────────────
        let write_params = |queue: &wgpu::Queue, level: u32| {
            let par = SimParamsRaw { n, num_nodes: nn, num_leaves: nl, p, eps, m2l_num_indices: m2l_ni, current_level: level, p2p_num_indices: p2p_ni };
            queue.write_buffer(&self.uniform_buffer, 0, bytes_of(&par));
        };

        // ── Encoder + dispatch helper ────────────────────────────
        let dispatch_pass = |device: &wgpu::Device, queue: &wgpu::Queue, pipeline_idx: usize, count: u32| {
            let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("fmm_sub") });
            {
                let mut cp = enc.begin_compute_pass(&wgpu::ComputePassDescriptor { label: Some("sub"), timestamp_writes: None });
                cp.set_pipeline(&self.pipelines[pipeline_idx]);
                cp.set_bind_group(0, &self.bind_group, &[]);
                let wg = (count + 63) / 64;
                cp.dispatch_workgroups(wg, 1, 1);
            }
            queue.submit(std::iter::once(enc.finish()));
        };

        // 1. P2M
        write_params(&self.queue, 0);
        dispatch_pass(&self.device, &self.queue, 0, nl);

        // 2. M2M (level-by-level, bottom-up)
        for lvl in 0..=self.max_level {
            write_params(&self.queue, lvl);
            dispatch_pass(&self.device, &self.queue, 1, nn);
        }

        // 3. M2L
        write_params(&self.queue, 0);
        dispatch_pass(&self.device, &self.queue, 2, nn);

        // 4. L2L (level-by-level, top-down)
        for lvl in (0..=self.max_level).rev() {
            write_params(&self.queue, lvl);
            dispatch_pass(&self.device, &self.queue, 3, nn);
        }

        // 5. L2P
        write_params(&self.queue, 0);
        dispatch_pass(&self.device, &self.queue, 4, n);

        // 6. P2P
        write_params(&self.queue, 0);
        dispatch_pass(&self.device, &self.queue, 5, n);
    }

    pub fn read_accelerations(&self, ax: &mut [f64], ay: &mut [f64]) {
        let n = ax.len().min(ay.len());
        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("readback") });
        encoder.copy_buffer_to_buffer(&self.dynamics_buffer, 0, &self.readback_buffer, 0, n as u64 * 8);
        self.queue.submit(std::iter::once(encoder.finish()));

        let (tx, rx) = std::sync::mpsc::channel();
        let slice = self.readback_buffer.slice(..(n as u64 * 8));
        slice.map_async(wgpu::MapMode::Read, move |r| { let _ = tx.send(r); });
        let _ = self.device.poll(wgpu::PollType::Wait { submission_index: None, timeout: None });
    match rx.recv() { Ok(Ok(())) => {} _ => panic!("GPU readback mapping failed"), }
    let data = slice.get_mapped_range();
    for i in 0..n {
            let offset = i * 8;
            let x = f32::from_ne_bytes([data[offset], data[offset + 1], data[offset + 2], data[offset + 3]]);
            let y = f32::from_ne_bytes([data[offset + 4], data[offset + 5], data[offset + 6], data[offset + 7]]);
            ax[i] = x as f64; ay[i] = y as f64;
        }
        drop(data);
        self.readback_buffer.unmap();
    }

    /// Async readback that works on both native and WASM.
    /// On native, this is a thin wrapper around the blocking [`read_accelerations`].
    /// On WASM, it polls the device and yields to the browser event loop.
    pub async fn read_accelerations_async(&self, ax: &mut [f64], ay: &mut [f64]) -> Result<(), crate::GpuError> {
        let n = ax.len().min(ay.len());
        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("readback_async") });
        encoder.copy_buffer_to_buffer(&self.dynamics_buffer, 0, &self.readback_buffer, 0, n as u64 * 8);
        self.queue.submit(std::iter::once(encoder.finish()));

        let slice = self.readback_buffer.slice(..(n as u64 * 8));
        let (tx, rx) = std::sync::mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |r| { let _ = tx.send(r); });

        #[cfg(not(target_arch = "wasm32"))]
        {
            let _ = self.device.poll(wgpu::PollType::Wait { submission_index: None, timeout: None });
            match rx.recv() { Ok(Ok(())) => {} _ => return Err(GpuError::InitFailed("readback mapping failed".into())), }
        }

        #[cfg(target_arch = "wasm32")]
        {
            // `device.poll()` is a no-op on WebGPU in wgpu 29.
            // `Promise.resolve()` yields to micro-tasks, but WebGPU callbacks
            // fire on macrotasks. We need a `setTimeout(0)` macrotask yield.
            use wasm_bindgen_futures::JsFuture;
            loop {
                if let Ok(Ok(())) = rx.try_recv() {
                    break;
                }
                // Create an actual macrotask yield so the browser processes
                // pending WebGPU work and fires the map_async callback.
                let p = js_sys::Promise::new(&mut |resolve, _reject| {
                    web_sys::window()
                        .expect("no window for setTimeout")
                        .set_timeout_with_callback_and_timeout_and_arguments_0(&resolve, 0)
                        .expect("setTimeout failed");
                });
                JsFuture::from(p).await
                    .map_err(|_| GpuError::InitFailed("async yield failed".into()))?;
            }
        }

        let data = slice.get_mapped_range();
        for i in 0..n {
            let offset = i * 8;
            let x = f32::from_ne_bytes([data[offset], data[offset + 1], data[offset + 2], data[offset + 3]]);
            let y = f32::from_ne_bytes([data[offset + 4], data[offset + 5], data[offset + 6], data[offset + 7]]);
            ax[i] = x as f64;
            ay[i] = y as f64;
        }
        drop(data);
        self.readback_buffer.unmap();
        Ok(())
    }

    /// Run GPU FMM followed by CPU FMM and compare per-body accelerations.
    ///
    /// Returns `(max_relative_error, mean_relative_error, sample_count)`.
    /// Panics if `max_rel > threshold` (use a loose threshold like 0.5;
    /// tight thresholds are better checked with mean or RMS).
    pub fn crosscheck(
        &self,
        bodies: &mut BodiesSoA,
        tree: &Tree,
        m2l_lists: &InteractionLists,
        p2p_lists: &InteractionLists,
        p: usize,
        softening: f64,
        threshold: f64,
    ) -> (f64, f64, usize) {
        // GPU FMM
        self.step(bodies);
        self.read_accelerations(&mut bodies.ax, &mut bodies.ay);
        let gpu_ax: Vec<f64> = bodies.ax.iter().copied().collect();
        let gpu_ay: Vec<f64> = bodies.ay.iter().copied().collect();

        // CPU FMM
        for a in bodies.ax.iter_mut() { *a = 0.0; }
        for a in bodies.ay.iter_mut() { *a = 0.0; }
        galax_core::compute_fmm_force(bodies, tree, m2l_lists, p2p_lists, p, softening);

        let n = bodies.len();
        let mut max_rel = 0.0f64;
        let mut sum_rel = 0.0f64;
        for i in 0..n {
            let cpu_mag = (bodies.ax[i].powi(2) + bodies.ay[i].powi(2)).sqrt();
            let gpu_mag = (gpu_ax[i].powi(2) + gpu_ay[i].powi(2)).sqrt();
            let denom = cpu_mag.max(gpu_mag).max(1e-30);
            let dx = gpu_ax[i] - bodies.ax[i];
            let dy = gpu_ay[i] - bodies.ay[i];
            let rel = (dx * dx + dy * dy).sqrt() / denom;
            max_rel = max_rel.max(rel);
            sum_rel += rel;
        }
        let mean_rel = sum_rel / n as f64;
        if max_rel > threshold {
            panic!("GPU crosscheck max relative error {:.2e} exceeds threshold {:.2e}", max_rel, threshold);
        }
        (max_rel, mean_rel, n)
    }

    /// Dispatch the O(N²) brute-force GPU kernel and read back accelerations.
    pub fn n2_debug(&self, bodies: &BodiesSoA) -> (Vec<f64>, Vec<f64>) {
        let n = bodies.len();
        // Upload body data (x, y, mass)
        {
            let mut flat: Vec<f32> = Vec::with_capacity(n * 4);
            for i in 0..n {
                flat.push(bodies.x[i] as f32);
                flat.push(bodies.y[i] as f32);
                flat.push(bodies.mass[i] as f32);
                flat.push(0.0);
            }
            self.queue.write_buffer(&self.body_data_buffer, 0, bytes_of_slice(&flat));
        }
        {
            let zeros: Vec<f32> = vec![0.0f32; n * 2];
            self.queue.write_buffer(&self.dynamics_buffer, 0, bytes_of_slice(&zeros));
        }

        let params = SimParamsRaw {
            n: n as u32,
            num_nodes: self.num_nodes,
            num_leaves: self.num_leaves,
            p: self.config.p,
            eps: self.config.eps,
            m2l_num_indices: self.m2l_num_indices,
            current_level: 0,
            p2p_num_indices: self.p2p_num_indices,
        };
        self.queue.write_buffer(&self.uniform_buffer, 0, bytes_of(&params));

        let mut enc = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("n2_debug") });
        {
            let mut cp = enc.begin_compute_pass(&wgpu::ComputePassDescriptor { label: Some("n2"), timestamp_writes: None });
            cp.set_pipeline(&self.pipelines[6]); // n2_debug is index 6
            cp.set_bind_group(0, &self.bind_group, &[]);
            let wg = (n as u32 + 63) / 64;
            cp.dispatch_workgroups(wg, 1, 1);
        }
        self.queue.submit(std::iter::once(enc.finish()));

        let mut ax = vec![0.0f64; n];
        let mut ay = vec![0.0f64; n];
        self.read_accelerations(&mut ax, &mut ay);
        (ax, ay)
    }

    pub fn device(&self) -> &wgpu::Device { &self.device }
    pub fn queue(&self) -> &wgpu::Queue { &self.queue }
}

// ── Tests ────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use galax_core::{build_p2p_lists, InteractionLists, Tree as CoreTree};

    #[cfg(not(target_arch = "wasm32"))]
    fn create_gpu_context(max_n: u32) -> GpuContext {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::LowPower,
            compatible_surface: None, force_fallback_adapter: false,
        })).expect("no GPU adapter");
        let (device, queue) = pollster::block_on(adapter.request_device(
            &wgpu::DeviceDescriptor {
                label: Some("test"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits {
                    max_storage_buffers_per_shader_stage: 16,
                    ..Default::default()
                },
                ..Default::default()
            },
        )).expect("failed to create GPU device");
        let config = GpuConfig { max_n, max_nodes: max_n.max(1024), max_interactions: (max_n * 64).max(4096), p: 4, eps: 0.1 };
        GpuContext::new(Arc::new(device), Arc::new(queue), config).expect("failed to init GPU context")
    }

    #[test]
    fn test_interaction_list_compaction() {
        let n_nodes = 5;
        let mut data = vec![[u32::MAX; 64]; n_nodes];
        let mut lengths = vec![0u8; n_nodes];
        data[0][..3].copy_from_slice(&[1, 2, 3]); lengths[0] = 3;
        data[1][..2].copy_from_slice(&[0, 4]); lengths[1] = 2;
        data[2][..0].copy_from_slice(&[]); lengths[2] = 0;
        data[3][..1].copy_from_slice(&[0]); lengths[3] = 1;
        data[4][..1].copy_from_slice(&[1]); lengths[4] = 1;
        let lists = InteractionLists { data, lengths };
        let (indices, offsets) = compact_interaction_lists(&lists);
        assert_eq!(&indices[0..3], &[1, 2, 3]);
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn test_gpu_full_fmm_vs_cpu_fmm() {
        fastrand::seed(42);
        let n = 200u32;
        let mut ctx = create_gpu_context(n * 2);
        let mut bodies = BodiesSoA::new(n as usize);
        for i in 0..n as usize {
            bodies.x[i] = fastrand::f64() * 100.0 - 50.0;
            bodies.y[i] = fastrand::f64() * 100.0 - 50.0;
            bodies.mass[i] = fastrand::f64() * 10.0 + 0.1;
        }
        let tree = CoreTree::build(&mut bodies, 32, -50.0, 50.0, -50.0, 50.0);
        let p2p_lists = build_p2p_lists(&tree);
        let m2l_lists = InteractionLists::build(&tree);

        // CPU FMM reference
        let mut cpu_bodies = BodiesSoA::new(n as usize);
        cpu_bodies.x.copy_from_slice(&bodies.x);
        cpu_bodies.y.copy_from_slice(&bodies.y);
        cpu_bodies.mass.copy_from_slice(&bodies.mass);
        galax_core::compute_fmm_force(&mut cpu_bodies, &tree, &m2l_lists, &p2p_lists, 4, 0.1);

        // GPU FMM
        ctx.upload_tree_data(&tree, &m2l_lists, &p2p_lists);
        ctx.step(&bodies);
        ctx.read_accelerations(&mut bodies.ax, &mut bodies.ay);

        let mut max_rel = 0.0f64;
        for i in 0..n as usize {
            let ca = (cpu_bodies.ax[i].powi(2) + cpu_bodies.ay[i].powi(2)).sqrt();
            let ga = (bodies.ax[i].powi(2) + bodies.ay[i].powi(2)).sqrt();
            let denom = ca.max(ga).max(1e-30);
            let err = ((bodies.ax[i] - cpu_bodies.ax[i]).powi(2) + (bodies.ay[i] - cpu_bodies.ay[i]).powi(2)).sqrt() / denom;
            max_rel = max_rel.max(err);
        }
        eprintln!("GPU FMM vs CPU FMM: max relative error = {:.2e}", max_rel);

        // Also compute RMS error
        let mut sum_sq = 0.0f64;
        for i in 0..n as usize {
            let err = ((bodies.ax[i] - cpu_bodies.ax[i]).powi(2) + (bodies.ay[i] - cpu_bodies.ay[i]).powi(2)).sqrt();
            sum_sq += err * err;
        }
        let rms = (sum_sq / n as f64).sqrt();
        let cpu_rms = {
            let mut s = 0.0;
            for i in 0..n as usize {
                s += cpu_bodies.ax[i].powi(2) + cpu_bodies.ay[i].powi(2);
            }
            (s / n as f64).sqrt()
        };
        eprintln!("  RMS error: {:.2e}, CPU RMS: {:.2e}, relative RMS: {:.2e}", rms, cpu_rms, rms / cpu_rms.max(1e-30));
        let rel_rms = rms / cpu_rms.max(1e-30);
        assert!(rel_rms < 0.05, "GPU FMM relative RMS error {:.2e} exceeds 5%", rel_rms);
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn test_gpu_p2p_component() {
        // P2P-only comparison to verify the P2P shader still works
        // in the unified bind group layout
        fastrand::seed(42);
        let n = 100u32;
        let mut ctx = create_gpu_context(n * 2);
        let mut bodies = BodiesSoA::new(n as usize);
        for i in 0..n as usize {
            bodies.x[i] = fastrand::f64() * 100.0 - 50.0;
            bodies.y[i] = fastrand::f64() * 100.0 - 50.0;
            bodies.mass[i] = fastrand::f64() * 10.0 + 0.1;
        }
        // Build tree with bodies arranged to force P2P-only:
        // Very small tree where all nodes are leaves
        let tree = CoreTree::build(&mut bodies, n as usize, -50.0, 50.0, -50.0, 50.0);
        let p2p_lists = build_p2p_lists(&tree);
        let m2l_lists = InteractionLists::build(&tree);

        let mut cpu_bodies = BodiesSoA::new(n as usize);
        cpu_bodies.x.copy_from_slice(&bodies.x);
        cpu_bodies.y.copy_from_slice(&bodies.y);
        cpu_bodies.mass.copy_from_slice(&bodies.mass);
        galax_core::p2p(&mut cpu_bodies, &tree, &p2p_lists, 0.1);

        ctx.upload_tree_data(&tree, &m2l_lists, &p2p_lists);
        // Run full FMM but compare P2P-only (the expansions will be zeros
        // since we're the only leaf, so M2L/L2L/L2P produce nothing,
        // and P2M produces multipoles that don't affect forces)
        ctx.step(&bodies);
        ctx.read_accelerations(&mut bodies.ax, &mut bodies.ay);

        let mut max_rel = 0.0f64;
        for i in 0..n as usize {
            let cpu_ax = cpu_bodies.ax[i];
            let cpu_ay = cpu_bodies.ay[i];
            let gpu_ax = bodies.ax[i];
            let gpu_ay = bodies.ay[i];
            let cpu_mag = (cpu_ax*cpu_ax + cpu_ay*cpu_ay).sqrt();
            let gpu_mag = (gpu_ax*gpu_ax + gpu_ay*gpu_ay).sqrt();
            let denom = cpu_mag.max(gpu_mag).max(1e-30);
            let err = ((gpu_ax - cpu_ax).powi(2) + (gpu_ay - cpu_ay).powi(2)).sqrt() / denom;
            max_rel = max_rel.max(err);
        }
        eprintln!("GPU P2P component: max relative error = {:.2e}", max_rel);
        assert!(max_rel < 1e-4, "P2P component error {:.2e}", max_rel);
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn test_crosscheck_layer_a_matches() {
        fastrand::seed(42);
        let n = 200u32;
        let mut ctx = create_gpu_context(n * 2);
        let mut bodies = BodiesSoA::new(n as usize);
        for i in 0..n as usize {
            bodies.x[i] = fastrand::f64() * 100.0 - 50.0;
            bodies.y[i] = fastrand::f64() * 100.0 - 50.0;
            bodies.mass[i] = fastrand::f64() * 10.0 + 0.1;
        }
        let tree = CoreTree::build(&mut bodies, 32, -50.0, 50.0, -50.0, 50.0);
        let p2p_lists = build_p2p_lists(&tree);
        let m2l_lists = InteractionLists::build(&tree);

        ctx.upload_tree_data(&tree, &m2l_lists, &p2p_lists);

        // crosscheck runs both GPU and CPU FMM, compares
        let (max_err, mean_err, n_samp) = ctx.crosscheck(
            &mut bodies, &tree, &m2l_lists, &p2p_lists, 4, 0.1, 1.0,
        );
        eprintln!("Layer A crosscheck: max={:.2e}, mean={:.2e}, n={}", max_err, mean_err, n_samp);
        assert!(mean_err < 0.1, "crosscheck mean error {:.2e} exceeds 10%", mean_err);
    }

    #[test]
    fn test_deriv_coeffs() {
        let p = 4;
        let eps = 0.1;
        let order = 2 * p;
        let stride = order + 1;
        let polys = build_kernel_polys(p, eps);
        let flat = flatten_deriv_coeffs(&polys, order, stride);
        let n_derivs = (order + 1) * (order + 2) / 2;
        // Spot check: N_{0,0}[0][0] = -1
        assert!((flat[0 * stride * stride + 0 * stride + 0] - (-1.0)).abs() < 1e-6);
        // N_{1,0}[1][0] = 1 (G^{(1,0)} = x)
        let di = { let n = 1; n * (n + 1) / 2 + 0 }; // deriv_idx for (1,0)
        assert!((flat[di * stride * stride + 1 * stride + 0] - 1.0).abs() < 1e-6);
    }
}
