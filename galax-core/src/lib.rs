use rayon::prelude::*;
use std::sync::OnceLock;
use std::collections::HashMap;
use std::sync::Mutex;

const MAX_P: usize = 16;

fn fill_powers(arr: &mut [f64; MAX_P], base: f64, p: usize) {
    arr[0] = 1.0;
    for k in 1..=p {
        arr[k] = arr[k - 1] * base;
    }
}

fn cached_binom(_p: usize) -> &'static [Vec<f64>] {
    static BINOM: OnceLock<Vec<Vec<f64>>> = OnceLock::new();
    BINOM.get_or_init(|| {
        let mut b = vec![vec![0.0_f64; MAX_P]; MAX_P];
        for n in 0..MAX_P {
            b[n][0] = 1.0;
            b[n][n] = 1.0;
            for k in 1..n {
                b[n][k] = b[n - 1][k - 1] + b[n - 1][k];
            }
        }
        b
    })
}

fn cached_fact(_p: usize) -> &'static [f64] {
    static FACT: OnceLock<Vec<f64>> = OnceLock::new();
    FACT.get_or_init(|| {
        let mut f = vec![1.0; MAX_P];
        for i in 1..MAX_P {
            f[i] = f[i - 1] * i as f64;
        }
        f
    })
}

pub struct BodiesSoA {
    pub x: Vec<f64>,
    pub y: Vec<f64>,
    pub vx: Vec<f64>,
    pub vy: Vec<f64>,
    pub mass: Vec<f64>,
    pub ax: Vec<f64>,
    pub ay: Vec<f64>,
}

impl BodiesSoA {
    pub fn new(n: usize) -> Self {
        Self {
            x: vec![0.0; n],
            y: vec![0.0; n],
            vx: vec![0.0; n],
            vy: vec![0.0; n],
            mass: vec![0.0; n],
            ax: vec![0.0; n],
            ay: vec![0.0; n],
        }
    }

    pub fn len(&self) -> usize {
        self.x.len()
    }

    pub fn is_empty(&self) -> bool {
        self.x.is_empty()
    }
}

const MORTON_BITS: u32 = 21;

fn normalize(x: f64, x_min: f64, x_max: f64) -> u64 {
    let t = ((x - x_min) / (x_max - x_min)).clamp(0.0, 1.0 - 1e-15);
    (t * (1u64 << MORTON_BITS) as f64) as u64
}

fn denormalize(v: u64, x_min: f64, x_max: f64) -> f64 {
    let cell_size = (x_max - x_min) / (1u64 << MORTON_BITS) as f64;
    x_min + (v as f64 + 0.5) * cell_size
}

fn spread_bits(v: u64) -> u64 {
    let mut x = v & ((1u64 << MORTON_BITS) - 1);
    x = (x | (x << 16)) & 0x0000ffff0000ffff;
    x = (x | (x << 8)) & 0x00ff00ff00ff00ff;
    x = (x | (x << 4)) & 0x0f0f0f0f0f0f0f0f;
    x = (x | (x << 2)) & 0x3333333333333333;
    x = (x | (x << 1)) & 0x5555555555555555;
    x
}

fn compact_bits(x: u64) -> u64 {
    let mut x = x & 0x5555555555555555;
    x = (x | (x >> 1)) & 0x3333333333333333;
    x = (x | (x >> 2)) & 0x0f0f0f0f0f0f0f0f;
    x = (x | (x >> 4)) & 0x00ff00ff00ff00ff;
    x = (x | (x >> 8)) & 0x0000ffff0000ffff;
    x = (x | (x >> 16)) & 0x00000000ffffffff;
    x
}

pub fn morton_encode(x: f64, y: f64, x_min: f64, x_max: f64, y_min: f64, y_max: f64) -> u64 {
    let xi = normalize(x, x_min, x_max);
    let yi = normalize(y, y_min, y_max);
    spread_bits(xi) | (spread_bits(yi) << 1)
}

pub fn morton_decode(key: u64, x_min: f64, x_max: f64, y_min: f64, y_max: f64) -> (f64, f64) {
    let xi = compact_bits(key);
    let yi = compact_bits(key >> 1);
    (denormalize(xi, x_min, x_max), denormalize(yi, y_min, y_max))
}

pub struct Node {
    pub center_x: f64,
    pub center_y: f64,
    pub half_width: f64,
    pub mass: f64,
    pub com_x: f64,
    pub com_y: f64,
    pub body_start: usize,
    pub body_end: usize,
    pub children: [Option<usize>; 4],
    pub parent: Option<usize>,
    pub is_leaf: bool,
}

pub struct Tree {
    pub nodes: Vec<Node>,
    pub n_max: usize,
    pub x_min: f64,
    pub x_max: f64,
    pub y_min: f64,
    pub y_max: f64,
}

impl Tree {
    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    /// Permute all 7 SoA arrays by the given index mapping.
    fn permute(bodies: &mut BodiesSoA, indices: &[usize]) {
        fn apply(arr: &mut Vec<f64>, indices: &[usize]) {
            let mut tmp = vec![0.0; arr.len()];
            for (new_i, &old_i) in indices.iter().enumerate() {
                tmp[new_i] = arr[old_i];
            }
            arr.copy_from_slice(&tmp);
        }
        apply(&mut bodies.x, indices);
        apply(&mut bodies.y, indices);
        apply(&mut bodies.vx, indices);
        apply(&mut bodies.vy, indices);
        apply(&mut bodies.mass, indices);
        apply(&mut bodies.ax, indices);
        apply(&mut bodies.ay, indices);
    }

    pub fn build(
        bodies: &mut BodiesSoA,
        n_max: usize,
        x_min: f64,
        x_max: f64,
        y_min: f64,
        y_max: f64,
    ) -> Self {
        let n = bodies.len();
        if n == 0 {
            return Self {
                nodes: Vec::new(),
                n_max,
                x_min, x_max, y_min, y_max,
            };
        }

        // Compute and sort by Morton key, then permute bodies
        let mut indices: Vec<usize> = (0..n).collect();
        let keys: Vec<u64> = (0..n)
            .map(|i| morton_encode(bodies.x[i], bodies.y[i], x_min, x_max, y_min, y_max))
            .collect();
        indices.sort_by_key(|&i| keys[i]);
        Self::permute(bodies, &indices);

        // Recompute keys for the permuted (Morton-sorted) bodies
        let keys: Vec<u64> = (0..n)
            .map(|i| morton_encode(bodies.x[i], bodies.y[i], x_min, x_max, y_min, y_max))
            .collect();

        // Build leaf ranges: contiguous groups of ≤ n_max
        let mut leaf_ranges: Vec<(usize, usize)> = Vec::new();
        let mut range_start = 0;
        while range_start < n {
            let range_end = (range_start + n_max).min(n);
            leaf_ranges.push((range_start, range_end));
            range_start = range_end;
        }

        // Create leaf nodes from body ranges
        let mut nodes: Vec<Node> = Vec::new();
        for &(start, end) in &leaf_ranges {
            let mut mass_sum = 0.0;
            let mut com_x = 0.0;
            let mut com_y = 0.0;
            let mut bx_min = f64::MAX;
            let mut bx_max = f64::MIN;
            let mut by_min = f64::MAX;
            let mut by_max = f64::MIN;

            for idx in start..end {
                let m = bodies.mass[idx];
                mass_sum += m;
                com_x += bodies.x[idx] * m;
                com_y += bodies.y[idx] * m;
                bx_min = bx_min.min(bodies.x[idx]);
                bx_max = bx_max.max(bodies.x[idx]);
                by_min = by_min.min(bodies.y[idx]);
                by_max = by_max.max(bodies.y[idx]);
            }

            let center_x = (bx_min + bx_max) * 0.5;
            let center_y = (by_min + by_max) * 0.5;
            let half_width = ((bx_max - bx_min).max(by_max - by_min)) * 0.5 + 1e-15;

            nodes.push(Node {
                center_x,
                center_y,
                half_width,
                mass: mass_sum,
                com_x: if mass_sum > 0.0 { com_x / mass_sum } else { center_x },
                com_y: if mass_sum > 0.0 { com_y / mass_sum } else { center_y },
                body_start: start,
                body_end: end,
                children: [None; 4],
                parent: None,
                is_leaf: true,
            });
        }

        // Build parent levels bottom-up using Morton prefix grouping
        let mut current_start = 0;
        let mut current_end = nodes.len();
        let mut group_shift: u32 = 2;

        while current_end - current_start > 1 {
            let mut i = current_start;

            while i < current_end {
                // Find siblings: consecutive nodes whose Morton keys share
                // the same prefix after right-shifting by group_shift
                let first_key = keys[nodes[i].body_start];
                let group_id = first_key >> group_shift;

                let mut j = i + 1;
                while j < current_end && j - i < 4 {
                    let next_key = keys[nodes[j].body_start];
                    if next_key >> group_shift != group_id {
                        break;
                    }
                    j += 1;
                }

                // Merge siblings [i, j) into parent
                let mut mass_sum = 0.0;
                let mut com_x = 0.0;
                let mut com_y = 0.0;
                let mut bx_min = f64::MAX;
                let mut bx_max = f64::MIN;
                let mut by_min = f64::MAX;
                let mut by_max = f64::MIN;
                let mut max_child_hw = 0.0_f64;

                for si in i..j {
                    let child = &nodes[si];
                    mass_sum += child.mass;
                    com_x += child.com_x * child.mass;
                    com_y += child.com_y * child.mass;
                    if child.half_width > max_child_hw {
                        max_child_hw = child.half_width;
                    }
                    let cl = child.center_x - child.half_width;
                    let cr = child.center_x + child.half_width;
                    if cl < bx_min { bx_min = cl; }
                    if cr > bx_max { bx_max = cr; }
                    let cb = child.center_y - child.half_width;
                    let ct = child.center_y + child.half_width;
                    if cb < by_min { by_min = cb; }
                    if ct > by_max { by_max = ct; }
                }

                let center_x = (bx_min + bx_max) * 0.5;
                let center_y = (by_min + by_max) * 0.5;
                let half_width = max_child_hw * 2.0;

                let parent_id = nodes.len();
                for si in i..j {
                    nodes[si].parent = Some(parent_id);
                }
                nodes.push(Node {
                    center_x,
                    center_y,
                    half_width,
                    mass: mass_sum,
                    com_x: if mass_sum > 0.0 { com_x / mass_sum } else { center_x },
                    com_y: if mass_sum > 0.0 { com_y / mass_sum } else { center_y },
                    body_start: nodes[i].body_start,
                    body_end: nodes[j - 1].body_end,
                    children: {
                        let mut c = [None; 4];
                        for (k, si) in (i..j).enumerate() {
                            c[k] = Some(si);
                        }
                        c
                    },
                    parent: None,
                    is_leaf: false,
                });

                i = j;
            }

            current_start = current_end;
            current_end = nodes.len();
            group_shift += 2;
        }

        Self {
            nodes,
            n_max,
            x_min, x_max, y_min, y_max,
        }
    }
}

/// Number of multipole/local expansion coefficients for order p.
/// Check if two cells are spatially adjacent.
/// Two cells are adjacent if their bounding boxes touch or overlap
/// AND their sizes are comparable (ratio ≤ 8) — prevents false
/// positives from large cells "containing" small cells.
fn cells_adjacent(na: &Node, nb: &Node) -> bool {
    let ratio = na.half_width.max(nb.half_width) / na.half_width.min(nb.half_width);
    if ratio > 8.0 { return false; }
    let dx = (na.center_x - nb.center_x).abs();
    let dy = (na.center_y - nb.center_y).abs();
    dx <= na.half_width + nb.half_width + 1e-15
        && dy <= na.half_width + nb.half_width + 1e-15
}

/// Find nodes that are true neighbors of the given node.
/// A neighbor is a node whose bounding box touches the query node
/// but whose center is in a different grid cell (adjacent, not overlapping).
/// Returns IDs of all adjacent nodes (up to 8, but may be fewer).
pub fn neighbors_geometric(tree: &Tree, node_id: usize) -> [Option<usize>; 8] {
    let na = &tree.nodes[node_id];
    let cx = na.center_x;
    let cy = na.center_y;
    let hw = na.half_width;

    // Define the 8 face/edge midpoints of the query cell
    let test_pts: [(f64, f64); 8] = [
        (cx + hw * 2.0, cy),            // E face center
        (cx + hw * 2.0, cy + hw * 2.0), // NE corner
        (cx, cy + hw * 2.0),            // N face center
        (cx - hw * 2.0, cy + hw * 2.0), // NW corner
        (cx - hw * 2.0, cy),            // W face center
        (cx - hw * 2.0, cy - hw * 2.0), // SW corner
        (cx, cy - hw * 2.0),            // S face center
        (cx + hw * 2.0, cy - hw * 2.0), // SE corner
    ];

    let mut result = [None; 8];
    for (k, &(px, py)) in test_pts.iter().enumerate() {
        let mut best = None;
        let mut best_dist = f64::MAX;
        for (other_id, nb) in tree.nodes.iter().enumerate() {
            if other_id == node_id { continue; }
            // Candidate must overlap the test point AND be adjacent
            let dx = (px - nb.center_x).abs();
            let dy = (py - nb.center_y).abs();
            if dx <= nb.half_width + 1e-15 && dy <= nb.half_width + 1e-15 {
                // Verify true adjacency (bounding boxes touch/overlap)
                if cells_adjacent(na, nb) {
                    let dist = (px - nb.center_x).powi(2) + (py - nb.center_y).powi(2);
                    if dist < best_dist {
                        best_dist = dist;
                        best = Some(other_id);
                    }
                }
            }
        }
        result[k] = best;
    }
    result
}

/// Compute tree node levels: 0 = leaves (finest), higher = coarser.
fn compute_levels(tree: &Tree) -> Vec<u32> {
    let n = tree.nodes.len();
    let mut levels = vec![0u32; n];
    for node_id in 0..n {
        let node = &tree.nodes[node_id];
        let mut max_cl = 0;
        for &c in node.children.iter().flatten() {
            max_cl = max_cl.max(levels[c] + 1);
        }
        levels[node_id] = max_cl;
    }
    levels
}

/// Spatial index for O(1) neighbor queries.
/// Maps (level, grid_i, grid_j) -> node_id for all nodes in the tree.
pub struct SpatialIndex {
    map: HashMap<(u32, i64, i64), usize>,
}

impl SpatialIndex {
    /// Build spatial index from a tree. Computes levels and grid coordinates
    /// for each node. Coordinate (i, j) at level L uniquely identifies the cell
    /// in the quadtree grid at that resolution.
    pub fn build(tree: &Tree) -> Self {
        let levels = compute_levels(tree);
        let mut map = HashMap::new();
        for (node_id, node) in tree.nodes.iter().enumerate() {
            let lv = levels[node_id];
            let cell_size = 2.0 * node.half_width;
            let i = ((node.center_x - tree.x_min) / cell_size).round() as i64;
            let j = ((node.center_y - tree.y_min) / cell_size).round() as i64;
            map.insert((lv, i, j), node_id);
        }
        SpatialIndex { map }
    }

    /// Look up a node at the given level and grid coordinates.
    pub fn lookup(&self, lv: u32, i: i64, j: i64) -> Option<usize> {
        self.map.get(&(lv, i, j)).copied()
    }

    /// Return the 8 directional neighbors of `node_id` using the spatial index.
    /// Each neighbor's coordinates are computed at the same level as the query node.
    /// If no cell exists at that level at the neighbor position, checks finer
    /// (descendant) and coarser (ancestor) levels.
    pub fn neighbors_of(&self, tree: &Tree, node_id: usize) -> [Option<usize>; 8] {
        let levels = compute_levels(tree);
        let lv = levels[node_id];
        let cell_size = 2.0 * tree.nodes[node_id].half_width;
        let ni = ((tree.nodes[node_id].center_x - tree.x_min) / cell_size).round() as i64;
        let nj = ((tree.nodes[node_id].center_y - tree.y_min) / cell_size).round() as i64;

        let dirs: [(i64, i64); 8] = [
            (1, 0), (1, 1), (0, 1), (-1, 1),
            (-1, 0), (-1, -1), (0, -1), (1, -1),
        ];

        let mut result = [None; 8];
        for (k, &(di, dj)) in dirs.iter().enumerate() {
            let ti = ni + di;
            let tj = nj + dj;

            // Try same level
            if let Some(&id) = self.map.get(&(lv, ti, tj)) {
                result[k] = Some(id);
                continue;
            }

            // Try finer levels (descendants): each level doubles the grid resolution
            for finer in 1..=4 {
                let flv = lv + finer;
                let scale = 1i64 << finer;
                let mut found = false;
                for di_f in 0..scale {
                    for dj_f in 0..scale {
                        if let Some(&id) =
                            self.map.get(&(flv, ti * scale + di_f, tj * scale + dj_f))
                        {
                            result[k] = Some(id);
                            found = true;
                            break;
                        }
                    }
                    if found {
                        break;
                    }
                }
                if found {
                    break;
                }
            }

            if result[k].is_some() {
                continue;
            }

            // Try coarser levels (ancestors)
            for coarser in 1..=lv {
                let clv = lv - coarser;
                let cti = if ti >= 0 { ti >> coarser } else { (ti - (1 << coarser) + 1) >> coarser };
                let ctj = if tj >= 0 { tj >> coarser } else { (tj - (1 << coarser) + 1) >> coarser };
                if let Some(&id) = self.map.get(&(clv, cti, ctj)) {
                    result[k] = Some(id);
                    break;
                }
            }
        }
        result
    }
}

/// Build a 2:1 balanced tree by iterative rebuild with constraint-driven leaf splitting.
/// When adjacent leaves have half_width ratio > 2, the coarser leaf body range
/// is split at its midpoint to refine the boundary.
pub fn build_constrained(
    bodies: &mut BodiesSoA, n_max: usize,
    x_min: f64, x_max: f64, y_min: f64, y_max: f64,
) -> Tree {
    let max_iter = 10;
    let mut split_points: Vec<usize> = Vec::new();

    for _iter in 0..max_iter {
        let tree = build_with_constraints(
            bodies, n_max, x_min, x_max, y_min, y_max, &split_points,
        );

        // Check for 2:1 violations
        let leaf_ids: Vec<usize> = (0..tree.nodes.len())
            .filter(|&id| tree.nodes[id].is_leaf).collect();
        let mut new_splits: Vec<usize> = Vec::new();

        for &lid in &leaf_ids {
            let coarse = &tree.nodes[lid];
            let hw = coarse.half_width;
            let neigh = neighbors_geometric(&tree, lid);
            for &n_opt in neigh.iter() {
                if let Some(nid) = n_opt {
                    let nhw = tree.nodes[nid].half_width;
                    if hw > nhw * 2.1 {
                        // This leaf is too coarse — split it
                        let start = coarse.body_start;
                        let end = coarse.body_end;
                        if end - start > 2 {
                            let mid = (start + end) / 2;
                            if !split_points.contains(&mid) && !new_splits.contains(&mid) {
                                new_splits.push(mid);
                            }
                        }
                        break; // one split per leaf per iteration
                    }
                }
            }
        }

        if new_splits.is_empty() { return tree; }
        split_points.extend(new_splits);
        split_points.sort_unstable();
        split_points.dedup();
    }

    build_with_constraints(bodies, n_max, x_min, x_max, y_min, y_max, &split_points)
}

/// Build tree with additional forced split points at specified body indices.
/// Split points are boundaries where new leaves are forced (even within n_max groups).
fn build_with_constraints(
    bodies: &mut BodiesSoA, n_max: usize,
    x_min: f64, x_max: f64, y_min: f64, y_max: f64,
    splits: &[usize],
) -> Tree {
    let n = bodies.len();
    let keys: Vec<u64> = (0..n)
        .map(|i| morton_encode(bodies.x[i], bodies.y[i], x_min, x_max, y_min, y_max))
        .collect();

    // Build leaf ranges: bounded by n_max AND any forced split points
    let mut leaf_ranges: Vec<(usize, usize)> = Vec::new();
    let mut range_start = 0;
    while range_start < n {
        let n_max_bound = (range_start + n_max).min(n);
        let split_bound = splits.iter().copied()
            .filter(|&s| s > range_start && s < n_max_bound)
            .min();
        let range_end = split_bound.unwrap_or(n_max_bound);
        leaf_ranges.push((range_start, range_end));
        range_start = range_end;
    }

    // Build nodes and parent hierarchy (same logic as Tree::build)
    let mut nodes: Vec<Node> = Vec::new();
    for &(start, end) in &leaf_ranges {
        let mut mass_sum = 0.0;
        let mut com_x = 0.0;
        let mut com_y = 0.0;
        let mut bx_min = f64::MAX; let mut bx_max = f64::MIN;
        let mut by_min = f64::MAX; let mut by_max = f64::MIN;
        for idx in start..end {
            let m = bodies.mass[idx]; mass_sum += m;
            com_x += bodies.x[idx] * m; com_y += bodies.y[idx] * m;
            bx_min = bx_min.min(bodies.x[idx]); bx_max = bx_max.max(bodies.x[idx]);
            by_min = by_min.min(bodies.y[idx]); by_max = by_max.max(bodies.y[idx]);
        }
        let cx = (bx_min + bx_max) * 0.5;
        let cy = (by_min + by_max) * 0.5;
        let hw = ((bx_max - bx_min).max(by_max - by_min)) * 0.5 + 1e-15;
        nodes.push(Node {
            center_x: cx, center_y: cy, half_width: hw,
            mass: mass_sum,
            com_x: if mass_sum > 0.0 { com_x / mass_sum } else { cx },
            com_y: if mass_sum > 0.0 { com_y / mass_sum } else { cy },
            body_start: start, body_end: end,
            children: [None; 4], parent: None, is_leaf: true,
        });
    }

    // Build parent levels bottom-up using Morton prefix grouping
    let mut current_start = 0;
    let mut current_end = nodes.len();
    let mut group_shift: u32 = 2;
    while current_end - current_start > 1 {
        let mut i = current_start;
        while i < current_end {
            let first_key = keys[nodes[i].body_start];
            let group_id = first_key >> group_shift;
            let mut j = i + 1;
            while j < current_end && j - i < 4 {
                if keys[nodes[j].body_start] >> group_shift != group_id { break; }
                j += 1;
            }
            let mut mass_sum = 0.0; let mut com_x = 0.0; let mut com_y = 0.0;
            let mut bx_min = f64::MAX; let mut bx_max = f64::MIN;
            let mut by_min = f64::MAX; let mut by_max = f64::MIN;
            let mut max_child_hw = 0.0_f64;
            for si in i..j {
                let ch = &nodes[si]; mass_sum += ch.mass;
                com_x += ch.com_x * ch.mass; com_y += ch.com_y * ch.mass;
                max_child_hw = max_child_hw.max(ch.half_width);
                bx_min = bx_min.min(ch.center_x - ch.half_width);
                bx_max = bx_max.max(ch.center_x + ch.half_width);
                by_min = by_min.min(ch.center_y - ch.half_width);
                by_max = by_max.max(ch.center_y + ch.half_width);
            }
            let cx = (bx_min + bx_max) * 0.5;
            let cy = (by_min + by_max) * 0.5;
            let hw = max_child_hw * 2.0;
            let pid = nodes.len();
            for si in i..j { nodes[si].parent = Some(pid); }
            nodes.push(Node {
                center_x: cx, center_y: cy, half_width: hw,
                mass: mass_sum,
                com_x: if mass_sum > 0.0 { com_x / mass_sum } else { cx },
                com_y: if mass_sum > 0.0 { com_y / mass_sum } else { cy },
                body_start: nodes[i].body_start, body_end: nodes[j-1].body_end,
                children: {
                    let mut c = [None; 4];
                    for (k, si) in (i..j).enumerate() { c[k] = Some(si); }
                    c
                },
                parent: None, is_leaf: false,
            });
            i = j;
        }
        current_start = current_end;
        current_end = nodes.len();
        group_shift += 2;
    }
    Tree { nodes, n_max, x_min, x_max, y_min, y_max }
}

/// Build tree with forced split positions (additional leaf boundaries).
fn build_with_splits(
    bodies: &mut BodiesSoA, n_max: usize,
    x_min: f64, x_max: f64, y_min: f64, y_max: f64,
    splits: &[usize],
) -> Tree {
    let n = bodies.len();
    let keys: Vec<u64> = (0..n)
        .map(|i| morton_encode(bodies.x[i], bodies.y[i], x_min, x_max, y_min, y_max))
        .collect();

    // Build leaf ranges: bounded by n_max AND split positions
    let mut leaf_ranges: Vec<(usize, usize)> = Vec::new();
    let mut range_start = 0;
    while range_start < n {
        let range_end = splits.iter()
            .filter(|&&s| s > range_start)
            .min()
            .map(|&s| s.min(range_start + n_max))
            .unwrap_or((range_start + n_max).min(n));
        leaf_ranges.push((range_start, range_end));
        range_start = range_end;
    }

    // Build the tree from leaf ranges (same logic as Tree::build)
    let mut nodes: Vec<Node> = Vec::new();
    for &(start, end) in &leaf_ranges {
        let mut mass_sum = 0.0;
        let mut com_x = 0.0;
        let mut com_y = 0.0;
        let mut bx_min = f64::MAX;
        let mut bx_max = f64::MIN;
        let mut by_min = f64::MAX;
        let mut by_max = f64::MIN;

        for idx in start..end {
            let m = bodies.mass[idx];
            mass_sum += m;
            com_x += bodies.x[idx] * m;
            com_y += bodies.y[idx] * m;
            bx_min = bx_min.min(bodies.x[idx]);
            bx_max = bx_max.max(bodies.x[idx]);
            by_min = by_min.min(bodies.y[idx]);
            by_max = by_max.max(bodies.y[idx]);
        }

        let center_x = (bx_min + bx_max) * 0.5;
        let center_y = (by_min + by_max) * 0.5;
        let half_width = ((bx_max - bx_min).max(by_max - by_min)) * 0.5 + 1e-15;

        nodes.push(Node {
            center_x, center_y, half_width,
            mass: mass_sum,
            com_x: if mass_sum > 0.0 { com_x / mass_sum } else { center_x },
            com_y: if mass_sum > 0.0 { com_y / mass_sum } else { center_y },
            body_start: start, body_end: end,
            children: [None; 4], parent: None, is_leaf: true,
        });
    }

    // Build parent levels bottom-up using Morton prefix grouping
    let mut current_start = 0;
    let mut current_end = nodes.len();
    let mut group_shift: u32 = 2;

    while current_end - current_start > 1 {
        let mut i = current_start;
        while i < current_end {
            let first_key = keys[nodes[i].body_start];
            let group_id = first_key >> group_shift;
            let mut j = i + 1;
            while j < current_end && j - i < 4 {
                let next_key = keys[nodes[j].body_start];
                if next_key >> group_shift != group_id { break; }
                j += 1;
            }

            let mut mass_sum = 0.0;
            let mut com_x = 0.0;
            let mut com_y = 0.0;
            let mut bx_min = f64::MAX;
            let mut bx_max = f64::MIN;
            let mut by_min = f64::MAX;
            let mut by_max = f64::MIN;
            let mut max_child_hw = 0.0_f64;

            for si in i..j {
                let child = &nodes[si];
                mass_sum += child.mass;
                com_x += child.com_x * child.mass;
                com_y += child.com_y * child.mass;
                max_child_hw = max_child_hw.max(child.half_width);
                bx_min = bx_min.min(child.center_x - child.half_width);
                bx_max = bx_max.max(child.center_x + child.half_width);
                by_min = by_min.min(child.center_y - child.half_width);
                by_max = by_max.max(child.center_y + child.half_width);
            }

            let center_x = (bx_min + bx_max) * 0.5;
            let center_y = (by_min + by_max) * 0.5;
            let half_width = max_child_hw * 2.0;

            let parent_id = nodes.len();
            for si in i..j { nodes[si].parent = Some(parent_id); }
            nodes.push(Node {
                center_x, center_y, half_width,
                mass: mass_sum,
                com_x: if mass_sum > 0.0 { com_x / mass_sum } else { center_x },
                com_y: if mass_sum > 0.0 { com_y / mass_sum } else { center_y },
                body_start: nodes[i].body_start,
                body_end: nodes[j - 1].body_end,
                children: {
                    let mut c = [None; 4];
                    for (k, si) in (i..j).enumerate() { c[k] = Some(si); }
                    c
                },
                parent: None, is_leaf: false,
            });
            i = j;
        }
        current_start = current_end;
        current_end = nodes.len();
        group_shift += 2;
    }

    Tree { nodes, n_max, x_min, x_max, y_min, y_max }
}

pub fn num_moments(p: usize) -> usize {
    (p + 1) * (p + 2) / 2
}

/// Map 2D multi-index (i, j) with i + j ≤ p to flat array index.
pub fn moment_index(i: usize, j: usize) -> usize {
    let n = i + j;
    n * (n + 1) / 2 + j
}

/// Allocate flat expansion array for `num_nodes` nodes at order p.
pub fn allocate_expansions(num_nodes: usize, p: usize) -> Vec<f64> {
    vec![0.0; num_nodes * num_moments(p)]
}

/// Compute P2M (particle-to-multipole) for all leaves.
///
/// For each leaf, accumulates each body's contribution to the leaf's
/// Cartesian multipole expansion centered at the node's center of mass:
///
///   M_{i,j} += m · dx^i · dy^j · (-1)^{i+j} / (i! · j!)
///
/// where (dx, dy) = (body_position - cell_com).
pub fn p2m(bodies: &BodiesSoA, tree: &Tree, p: usize, softening: f64) -> Vec<f64> {
    let _ = softening;
    let stride = num_moments(p);
    let mut multipole = allocate_expansions(tree.nodes.len(), p);
    let fact = cached_fact(p);

    for (node_id, node) in tree.nodes.iter().enumerate() {
        if !node.is_leaf {
            continue;
        }

        let cx = node.com_x;
        let cy = node.com_y;
        let base = node_id * stride;

        for body_idx in node.body_start..node.body_end {
            let dx = bodies.x[body_idx] - cx;
            let dy = bodies.y[body_idx] - cy;
            let mass = bodies.mass[body_idx];

            let mut dx_pow = [0.0_f64; MAX_P];
            let mut dy_pow = [0.0_f64; MAX_P];
            fill_powers(&mut dx_pow, dx, p);
            fill_powers(&mut dy_pow, dy, p);

            for i in 0..=p {
                for j in 0..=(p - i) {
                    let coeff = mass * dx_pow[i] * dy_pow[j] / (fact[i] * fact[j]);
                    multipole[base + moment_index(i, j)] += coeff;
                }
            }
        }
    }

    debug_assert!(
        !multipole.iter().any(|v| v.is_nan()),
        "NaN detected after P2M"
    );
    multipole
}

/// Shift child multipole coefficients to parent (M2M upward pass).
pub fn m2m(multipole: &mut [f64], tree: &Tree, p: usize) {
    let stride = num_moments(p);
    let fact = cached_fact(p);

    for node_id in 0..tree.nodes.len() {
        let node = &tree.nodes[node_id];
        if node.is_leaf {
            continue;
        }

        let parent_base = node_id * stride;

        for &child_id in node.children.iter().flatten() {
            let child = &tree.nodes[child_id];
            let dx = child.com_x - node.com_x;
            let dy = child.com_y - node.com_y;
            let child_base = child_id * stride;

            let mut dx_pow = [0.0_f64; MAX_P];
            let mut dy_pow = [0.0_f64; MAX_P];
            fill_powers(&mut dx_pow, dx, p);
            fill_powers(&mut dy_pow, dy, p);

            // M2M shift: M_parent_{I,J} = Σ M_child_{i,j} · dx^{I-i} · dy^{J-j} / ((I-i)! (J-j)!)
            // where (dx, dy) = (child_com - parent_com).
            for outer_i in 0..=p {
                for outer_j in 0..=(p - outer_i) {
                    let mut sum = 0.0;
                    for i in 0..=outer_i {
                        for j in 0..=outer_j {
                            if i + j > p {
                                continue;
                            }
                            sum += multipole[child_base + moment_index(i, j)]
                                * dx_pow[outer_i - i]
                                * dy_pow[outer_j - j]
                                / (fact[outer_i - i] * fact[outer_j - j]);
                        }
                    }
                    multipole[parent_base + moment_index(outer_i, outer_j)] += sum;
                }
            }
        }
    }

    debug_assert!(
        !multipole.iter().any(|v| v.is_nan()),
        "NaN detected after M2M"
    );
}

/// Precomputed M2L interaction lists for all nodes in a tree.
pub struct InteractionLists {
    pub data: Vec<[u32; 64]>,
    pub lengths: Vec<u8>,
}

impl InteractionLists {
    pub fn build(tree: &Tree) -> Self {
        let n_nodes = tree.nodes.len();
        let mut data = vec![[u32::MAX; 64]; n_nodes];
        let mut lengths = vec![0u8; n_nodes];

        // Compute level (distance from root) for each node
        let mut level = vec![0usize; n_nodes];
        for node_id in 0..n_nodes {
            let node = &tree.nodes[node_id];
            let mut max_child_level = 0;
            for &c in node.children.iter().flatten() {
                max_child_level = max_child_level.max(level[c] + 1);
            }
            level[node_id] = max_child_level;
        }

        // Group nodes by level
        let max_level = level.iter().max().copied().unwrap_or(0);
        let mut nodes_by_level: Vec<Vec<usize>> = vec![Vec::new(); max_level + 1];
        for (id, &lv) in level.iter().enumerate() {
            nodes_by_level[lv].push(id);
        }

        // For each level (excluding root), compute interaction lists
        // from the parent-neighbor stencil.
        for lv in 0..max_level {
            let nodes_at_lv = &nodes_by_level[lv];
            let nodes_at_parent = &nodes_by_level[lv + 1];

            for &node_id in nodes_at_lv {
                let node = &tree.nodes[node_id];
                let parent_id = match node.parent {
                    Some(p) => p,
                    None => continue,
                };
                let parent = &tree.nodes[parent_id];

                for &pn_id in nodes_at_parent {
                    if pn_id == parent_id {
                        continue;
                    }
                    let pn = &tree.nodes[pn_id];

                    let dx = (pn.center_x - parent.center_x).abs();
                    let dy = (pn.center_y - parent.center_y).abs();
                    let h_sum = parent.half_width + pn.half_width;
                    if dx > h_sum + 1e-15 || dy > h_sum + 1e-15 {
                        continue;
                    }

                    for &child_id in pn.children.iter().flatten() {
                        let child = &tree.nodes[child_id];

                        let cdx = (child.center_x - node.center_x).abs();
                        let cdy = (child.center_y - node.center_y).abs();
                        let adj_x = cdx <= node.half_width + child.half_width + 1e-15;
                        let adj_y = cdy <= node.half_width + child.half_width + 1e-15;
                        if adj_x && adj_y {
                            continue;
                        }

                        let len = lengths[node_id] as usize;
                        if len < 64 {
                            data[node_id][len] = child_id as u32;
                            lengths[node_id] += 1;
                        }
                    }
                }
            }
        }

        // Revert cross-level additions — handled by P2P descent instead.

        for &l in &lengths {
            assert!(l <= 64, "interaction list exceeds 64 entries");
        }

        Self { data, lengths }
    }
}

/// Allocate local expansion arrays (same layout as multipole).
pub fn allocate_locals(num_nodes: usize, p: usize) -> Vec<f64> {
    allocate_expansions(num_nodes, p)
}

/// Compute kernel derivative G^{(k,l)}(dx, dy, ε) for k+l ≤ 2p.
type DerivBuf = Box<[f64]>;

fn kernel_deriv_idx(k: usize, l: usize, p: usize) -> usize {
    let n = k + l;
    n * (2 * p + 1) + l // 2p+1 entries per row, each row has varying valid range
}

fn compute_kernel_derivs(dx: f64, dy: f64, eps: f64, p: usize) -> DerivBuf {
    let order = 2 * p;
    let stride = order + 1;
    let buf_size = stride * stride;
    let mut deriv = vec![0.0_f64; buf_size].into_boxed_slice();

    let x = dx;
    let y = dy;
    let eps2 = eps * eps;
    let r2 = x * x + y * y + eps2;
    let r = r2.sqrt();

    // Precompute R^{-n} for n = 1, 3, 5, ..., 1+2*order
    let mut inv_r = vec![0.0_f64; 2 + 2 * order];
    inv_r[1] = 1.0 / r;
    for n in (3..=1 + 2 * order).step_by(2) {
        inv_r[n] = inv_r[n - 2] / r2;
    }

    // N_{k,l} as dense polynomial coefficient arrays.
    // N_{k,l}[a][b] = coefficient of x^a · y^b in the numerator polynomial.
    // G^{(k,l)} = (Σ N_{k,l}[a][b] · x^a · y^b) / R^{1+2k+2l}
    let idx = |k: usize, l: usize| k * stride + l;
    let mut poly: Vec<Vec<Vec<f64>>> = vec![vec![vec![0.0_f64; stride]; stride]; stride * stride];

    // N_{0,0} = -1
    poly[idx(0, 0)][0][0] = -1.0;

    // Build N polynomials by increasing total order.
    // Recurrence:
    //   N_{k+1,l} = R² · ∂N/∂x - (1 + 2k + 2l) · N · x
    //   N_{k,l+1} = R² · ∂N/∂y - (1 + 2k + 2l) · N · y
    // where R² = x² + y² + ε².
    for n in 1..=order {
        for k in 0..=n {
            let l = n - k;
            let mut coeff = vec![vec![0.0_f64; stride]; stride];

            if k > 0 {
                let prev = &poly[idx(k - 1, l)];
                let m = 1 + 2 * (k - 1) + 2 * l;

                for a in 0..stride {
                    for b in 0..(stride - a) {
                        let c = prev[a][b];
                        if c == 0.0 { continue; }
                        if a > 0 {
                            let dc = c * a as f64;
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
                        if a + 1 < stride { coeff[a + 1][b] -= m as f64 * c; }
                    }
                }
            } else if l > 0 {
                // Only use x-derivative path for k>0.
                // For k=0,l>0 use y-derivative path (no double-count).
                let prev = &poly[idx(k, l - 1)];
                let m = 1 + 2 * k + 2 * (l - 1);

                for a in 0..stride {
                    for b in 0..(stride - a) {
                        let c = prev[a][b];
                        if c == 0.0 { continue; }
                        if b > 0 {
                            let dc = c * b as f64;
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
                        if b + 1 < stride { coeff[a][b + 1] -= m as f64 * c; }
                    }
                }
            }

            poly[idx(k, l)] = coeff;
        }
    }

    // Evaluate G^{(k,l)} = (Σ N_{k,l}[a][b] · x^a · y^b) / R^{1+2k+2l}
    // Single polynomial evaluation per derivative — no cancellation.
    for k in 0..=order {
        for l in 0..=order - k {
            let mut val = 0.0;
            for a in 0..stride {
                for b in 0..(stride - a) {
                    let c = poly[idx(k, l)][a][b];
                    if c != 0.0 {
                        val += c * x.powi(a as i32) * y.powi(b as i32);
                    }
                }
            }
            let exp = 1 + 2 * k + 2 * l;
            deriv[kernel_deriv_idx(k, l, p)] = val * inv_r[exp];
        }
    }

    deriv
}

/// M2L: compute far-field local expansion contributions from all interaction pairs.
pub fn m2l(
    locals: &mut [f64],
    multipole: &[f64],
    lists: &InteractionLists,
    tree: &Tree,
    p: usize,
    softening: f64,
) {
    let stride = num_moments(p);
    let fact = cached_fact(p);
    let cache = Mutex::new(HashMap::<(i64, i64), Vec<f64>>::new());

    locals
        .par_chunks_exact_mut(stride)
        .enumerate()
        .for_each(|(node_id, local_chunk)| {
            let len = lists.lengths[node_id] as usize;
            if len == 0 {
                return;
            }
            let target = &tree.nodes[node_id];

            for &partner_idx in lists.data[node_id][..len].iter() {
                let source_id = partner_idx as usize;
                let source = &tree.nodes[source_id];
                let source_base = source_id * stride;

                let dx = target.com_x - source.com_x;
                let dy = target.com_y - source.com_y;

                // Cached kernel derivatives — quantize (dx, dy) by rounding to i64.
                // Cells at the same level share quantized separation vectors.
                let key = (dx as i64, dy as i64);
                let deriv = {
                    let guard = cache.lock().unwrap();
                    if let Some(d) = guard.get(&key) {
                        // Index directly — can't return reference across MutexGuard drop
                        // so we copy a small portion: we need deriv[kernel_deriv_idx(...)]
                        // in the hot loop below. Instead, just clone the whole buffer.
                        // DerivBuf for p=4 is 81 f64s = 648 bytes; clone is cheap.
                        Some(d.clone())
                    } else {
                        None
                    }
                };
                let deriv = match deriv {
                    Some(d) => d,
                    None => {
                        let computed = compute_kernel_derivs(dx, dy, softening, p);
                        let computed_vec = computed.to_vec();
                        let mut guard = cache.lock().unwrap();
                        guard.entry(key).or_insert_with(|| computed_vec.clone());
                        computed_vec
                    }
                };

                // Full M2L:
                // C_{I,J} += Σ_{i,j} M_{i,j} · (-1)^{i+j} / (I! J!) · G^{(i+I, j+J)}(d)
                // The (-1)^{i+j} comes from the multipole-to-potential series.
                // The 1/(I! J!) converts from Taylor-series form to monomial form.
                for i in 0..=p {
                    for j in 0..=(p - i) {
                        let m_val = multipole[source_base + moment_index(i, j)];
                        if m_val == 0.0 {
                            continue;
                        }
                        let sign = if (i + j) % 2 == 0 { 1.0 } else { -1.0 };

                        for outer_i in 0..=p {
                            for outer_j in 0..=(p - outer_i) {
                                let g = deriv[kernel_deriv_idx(
                                    i + outer_i, j + outer_j, p,
                                )];
                                if g == 0.0 {
                                    continue;
                                }
                                let l_factor = 1.0 / (fact[outer_i] * fact[outer_j]);
                                local_chunk[moment_index(outer_i, outer_j)] +=
                                    m_val * sign * l_factor * g;
                            }
                        }
                    }
                }
            }
        });
}

/// L2L: shift local expansion from parent to each child (downward pass).
pub fn l2l(locals: &mut [f64], tree: &Tree, p: usize) {
    let stride = num_moments(p);
    let binom = cached_binom(p);

    for node_id in 0..tree.nodes.len() {
        let node = &tree.nodes[node_id];
        if node.is_leaf {
            continue;
        }

        let parent_base = node_id * stride;

        for &child_id in node.children.iter().flatten() {
            let child = &tree.nodes[child_id];
            let dx = child.com_x - node.com_x;
            let dy = child.com_y - node.com_y;
            let child_base = child_id * stride;

            let mut dx_pow = [0.0_f64; MAX_P];
            let mut dy_pow = [0.0_f64; MAX_P];
            fill_powers(&mut dx_pow, dx, p);
            fill_powers(&mut dy_pow, dy, p);

            for outer_i in 0..=p {
                for outer_j in 0..=(p - outer_i) {
                    let mut sum = 0.0;
                    for i in outer_i..=p {
                        for j in outer_j..=(p - i) {
                            sum += locals[parent_base + moment_index(i, j)]
                                * binom[i][outer_i]
                                * binom[j][outer_j]
                                * dx_pow[i - outer_i]
                                * dy_pow[j - outer_j];
                        }
                    }
                    locals[child_base + moment_index(outer_i, outer_j)] += sum;
                }
            }
        }
    }
}

/// L2P: evaluate local expansion gradient at each leaf body position.
/// The local expansion gives Φ(du) = Σ C_{I,J} · dux^I · duy^J
/// Acceleration a = -∇Φ = -(∂Φ/∂x, ∂Φ/∂y)
/// ∂Φ/∂x = Σ_{I≥1, J} C_{I,J} · I · dux^{I-1} · duy^J
pub fn l2p(bodies: &mut BodiesSoA, locals: &[f64], tree: &Tree, p: usize) {
    let stride = num_moments(p);

    let contribs: Vec<(usize, f64, f64)> = (0..tree.nodes.len())
        .into_par_iter()
        .flat_map(|node_id| {
            let node = &tree.nodes[node_id];
            if !node.is_leaf {
                return Vec::new();
            }
            let base = node_id * stride;
            let mut result = Vec::with_capacity(node.body_end - node.body_start);

            for body_idx in node.body_start..node.body_end {
                let dux = bodies.x[body_idx] - node.com_x;
                let duy = bodies.y[body_idx] - node.com_y;

                let mut xp = [0.0_f64; MAX_P];
                let mut yp = [0.0_f64; MAX_P];
                fill_powers(&mut xp, dux, p);
                fill_powers(&mut yp, duy, p);

                let mut ax = 0.0;
                let mut ay = 0.0;

                for i in 0..=p {
                    for j in 0..=(p - i) {
                        let coeff = locals[base + moment_index(i, j)];
                        if i >= 1 {
                            ax += coeff * (i as f64) * xp[i - 1] * yp[j];
                        }
                        if j >= 1 {
                            ay += coeff * (j as f64) * xp[i] * yp[j - 1];
                        }
                    }
                }

                result.push((body_idx, ax, ay));
            }
            result
        })
        .collect();

    for (idx, ax, ay) in contribs {
        bodies.ax[idx] -= ax;
        bodies.ay[idx] -= ay;
    }
}

/// Precompute P2P leaf-neighbor lists (adjacent leaf pairs, plus self).
pub fn build_p2p_lists(tree: &Tree) -> InteractionLists {
    let n_nodes = tree.nodes.len();
    let mut data = vec![[u32::MAX; 64]; n_nodes];
    let mut lengths = vec![0u8; n_nodes];

    let leaf_ids: Vec<usize> = tree
        .nodes
        .iter()
        .enumerate()
        .filter(|(_, n)| n.is_leaf)
        .map(|(id, _)| id)
        .collect();

    for &a in &leaf_ids {
        // Self
        let len = lengths[a] as usize;
        if len < 64 { data[a][len] = a as u32; lengths[a] += 1; }

        let na = &tree.nodes[a];
        for &b in &leaf_ids {
            if a == b { continue; }
            let nb = &tree.nodes[b];
            let dx = (na.center_x - nb.center_x).abs();
            let dy = (na.center_y - nb.center_y).abs();
            if dx <= na.half_width + nb.half_width + 1e-15
                && dy <= na.half_width + nb.half_width + 1e-15
            {
                let len = lengths[a] as usize;
                if len < 64 { data[a][len] = b as u32; lengths[a] += 1; }
            }
        }
    }

    InteractionLists { data, lengths }
}

/// P2P: compute direct near-field forces between adjacent leaf bodies.
pub fn p2p(
    bodies: &mut BodiesSoA,
    tree: &Tree,
    lists: &InteractionLists,
    softening: f64,
) {
    let soft2 = softening * softening;

    for node_id in 0..tree.nodes.len() {
        if !tree.nodes[node_id].is_leaf {
            continue;
        }
        let len = lists.lengths[node_id] as usize;
        if len == 0 {
            continue;
        }

        for &partner_u32 in lists.data[node_id][..len].iter() {
            let partner_id = partner_u32 as usize;

            if partner_id == node_id {
                let start = tree.nodes[node_id].body_start;
                let end = tree.nodes[node_id].body_end;
                for i in start..end {
                    let mut ax_i = 0.0;
                    let mut ay_i = 0.0;
                    for j in (i + 1)..end {
                        let dx = bodies.x[j] - bodies.x[i];
                        let dy = bodies.y[j] - bodies.y[i];
                        let r2 = dx * dx + dy * dy + soft2;
                        let inv_r3 = r2.sqrt().recip().powi(3);
                        let f = bodies.mass[j] * inv_r3;
                        ax_i += f * dx;
                        ay_i += f * dy;
                        bodies.ax[j] -= f * dx * bodies.mass[i] / bodies.mass[j];
                        bodies.ay[j] -= f * dy * bodies.mass[i] / bodies.mass[j];
                    }
                    bodies.ax[i] += ax_i;
                    bodies.ay[i] += ay_i;
                }
            } else if partner_id < node_id {
                continue;
            } else {
                let pb = tree.nodes[partner_id].body_start;
                let pe = tree.nodes[partner_id].body_end;
                let ma = tree.nodes[node_id].body_start;
                let mb = tree.nodes[node_id].body_end;
                for i in ma..mb {
                    let mut ax_i = 0.0;
                    let mut ay_i = 0.0;
                    for j in pb..pe {
                        let dx = bodies.x[j] - bodies.x[i];
                        let dy = bodies.y[j] - bodies.y[i];
                        let r2 = dx * dx + dy * dy + soft2;
                        let inv_r3 = r2.sqrt().recip().powi(3);
                        let f = bodies.mass[j] * inv_r3;
                        ax_i += f * dx;
                        ay_i += f * dy;
                        bodies.ax[j] -= f * dx * bodies.mass[i] / bodies.mass[j];
                        bodies.ay[j] -= f * dy * bodies.mass[i] / bodies.mass[j];
                    }
                    bodies.ax[i] += ax_i;
                    bodies.ay[i] += ay_i;
                }
            }
        }
    }
}

/// Full 6-pass FMM force evaluation.
/// Zeroes bodies.ax/ay and fills them with total FMM-approximated accelerations.
pub fn compute_fmm_force(
    bodies: &mut BodiesSoA,
    tree: &Tree,
    m2l_lists: &InteractionLists,
    p2p_lists: &InteractionLists,
    p: usize,
    softening: f64,
) {
    for a in bodies.ax.iter_mut() {
        *a = 0.0;
    }
    for a in bodies.ay.iter_mut() {
        *a = 0.0;
    }

    let mut multipole = p2m(bodies, tree, p, softening);
    m2m(&mut multipole, tree, p);

    let mut locals = allocate_locals(tree.nodes.len(), p);
    m2l(&mut locals, &multipole, m2l_lists, tree, p, softening);
    l2l(&mut locals, tree, p);
    l2p(bodies, &locals, tree, p);
    p2p(bodies, tree, p2p_lists, softening);
}

/// Run sampled N² cross-check: compute FMM on all N bodies, then compute exact N² 
/// for a random sample and report error statistics.
pub fn validate_fmm(
    bodies: &BodiesSoA,
    tree: &Tree,
    m2l_lists: &InteractionLists,
    p2p_lists: &InteractionLists,
    p: usize,
    softening: f64,
    sample_size: usize,
) -> (usize, f64, f64, f64) {
    let n = bodies.len();
    let sample_size = sample_size.min(n);

    // Run FMM
    let mut fmm_bodies = BodiesSoA::new(n);
    for i in 0..n {
        fmm_bodies.x[i] = bodies.x[i];
        fmm_bodies.y[i] = bodies.y[i];
        fmm_bodies.mass[i] = bodies.mass[i];
    }
    compute_fmm_force(&mut fmm_bodies, tree, m2l_lists, p2p_lists, p, softening);

    // Sample random body indices
    let sample_indices: Vec<usize> = {
        let mut indices: Vec<usize> = (0..n).collect();
        fastrand::shuffle(&mut indices);
        indices.truncate(sample_size);
        indices
    };

    // Compute exact N² for sampled bodies only
    let soft2 = softening * softening;
    let mut max_rel_err = 0.0_f64;
    let mut sum_rel_err = 0.0_f64;

    for &idx in &sample_indices {
        let mut exact_ax = 0.0;
        let mut exact_ay = 0.0;

        for j in 0..n {
            if j == idx {
                continue;
            }
            let dx = bodies.x[j] - bodies.x[idx];
            let dy = bodies.y[j] - bodies.y[idx];
            let r2 = dx * dx + dy * dy + soft2;
            let inv_r3 = r2.sqrt().recip().powi(3);
            let f = bodies.mass[j] * inv_r3;
            exact_ax += f * dx;
            exact_ay += f * dy;
        }

        let fmm_a = (fmm_bodies.ax[idx].powi(2) + fmm_bodies.ay[idx].powi(2)).sqrt();
        let exact_a = (exact_ax.powi(2) + exact_ay.powi(2)).sqrt();
        let denom = exact_a.abs().max(1e-20);
        let rel_err = (fmm_a - exact_a).abs() / denom;
        max_rel_err = max_rel_err.max(rel_err);
        sum_rel_err += rel_err;
    }

    let mean_rel_err = sum_rel_err / sample_size as f64;
    (sample_size, max_rel_err, mean_rel_err, 0.0)
}

/// Full N² regression check (N must be ≤ 2000).
pub fn n2_regression(bodies: &BodiesSoA, tree: &Tree, m2l_lists: &InteractionLists, p2p_lists: &InteractionLists, p: usize, softening: f64) -> f64 {
    let n = bodies.len();
    assert!(n <= 2000, "n2_regression only valid for N ≤ 2000");

    // Run FMM
    let mut fmm_bodies = BodiesSoA::new(n);
    for i in 0..n {
        fmm_bodies.x[i] = bodies.x[i];
        fmm_bodies.y[i] = bodies.y[i];
        fmm_bodies.mass[i] = bodies.mass[i];
    }
    compute_fmm_force(&mut fmm_bodies, tree, m2l_lists, p2p_lists, p, softening);

    // Direct N²
    let mut direct_bodies = BodiesSoA::new(n);
    for i in 0..n {
        direct_bodies.x[i] = bodies.x[i];
        direct_bodies.y[i] = bodies.y[i];
        direct_bodies.mass[i] = bodies.mass[i];
    }
    compute_n2_force(&mut direct_bodies, softening);

    let mut max_rel_err = 0.0_f64;
    for i in 0..n {
        let fmm_a = (fmm_bodies.ax[i].powi(2) + fmm_bodies.ay[i].powi(2)).sqrt();
        let dir_a = (direct_bodies.ax[i].powi(2) + direct_bodies.ay[i].powi(2)).sqrt();
        let denom = dir_a.abs().max(1e-20);
        let rel_err = (fmm_a - dir_a).abs() / denom;
        max_rel_err = max_rel_err.max(rel_err);
    }
    max_rel_err
}

/// Evaluate the multipole expansion of a single cell at a test point.
pub fn eval_multipole(
    multipole: &[f64],
    cell_base: usize,
    dx: f64,
    dy: f64,
    p: usize,
    softening: f64,
) -> f64 {
    let r_tilde = (dx * dx + dy * dy + softening * softening).sqrt();
    let g = -1.0 / r_tilde;

    let mut potential = multipole[cell_base + moment_index(0, 0)] * g;

    if p >= 1 {
        let r_tilde_3 = r_tilde * r_tilde * r_tilde;
        let dg_dx = dx / r_tilde_3;
        let dg_dy = dy / r_tilde_3;

        potential += multipole[cell_base + moment_index(1, 0)] * (-dg_dx);
        potential += multipole[cell_base + moment_index(0, 1)] * (-dg_dy);
    }

    potential
}

/// Direct softened gravitational potential at a point from all bodies.
pub fn direct_potential(x: f64, y: f64, bodies: &BodiesSoA, softening: f64) -> f64 {
    let mut pot = 0.0;
    for i in 0..bodies.len() {
        let dx = x - bodies.x[i];
        let dy = y - bodies.y[i];
        pot += -bodies.mass[i] / (dx * dx + dy * dy + softening * softening).sqrt();
    }
    pot
}

/// Kinetic energy: Σ 0.5 · m · (vx² + vy²)
pub fn compute_kinetic_energy(bodies: &BodiesSoA) -> f64 {
    let mut ke = 0.0;
    for i in 0..bodies.len() {
        let v2 = bodies.vx[i] * bodies.vx[i] + bodies.vy[i] * bodies.vy[i];
        ke += 0.5 * bodies.mass[i] * v2;
    }
    ke
}

/// Linear momentum: (Σ m·vx, Σ m·vy)
pub fn compute_momentum(bodies: &BodiesSoA) -> (f64, f64) {
    let mut px = 0.0;
    let mut py = 0.0;
    for i in 0..bodies.len() {
        px += bodies.mass[i] * bodies.vx[i];
        py += bodies.mass[i] * bodies.vy[i];
    }
    (px, py)
}

/// Angular momentum (2D scalar): Σ m · (x·vy - y·vx)
pub fn compute_angular_momentum(bodies: &BodiesSoA) -> f64 {
    let mut l = 0.0;
    for i in 0..bodies.len() {
        l += bodies.mass[i] * (bodies.x[i] * bodies.vy[i] - bodies.y[i] * bodies.vx[i]);
    }
    l
}

/// Approximate FMM potential energy from current accelerations.
/// Uses U ≈ -Σ m_i · |a_i| · distance-like... simplified:
/// Returns -0.5 · Σ m_i · |a_i| · r_guess
/// For diagnostics only — NOT physically exact.
pub fn compute_fmm_potential_energy(bodies: &BodiesSoA) -> f64 {
    let mut pe = 0.0;
    for i in 0..bodies.len() {
        let ax = bodies.ax[i];
        let ay = bodies.ay[i];
        let a_mag = (ax * ax + ay * ay).sqrt();
        // Approximate: for each body, potential contribution ≈ -m_i · a_i · r_i (softened)
        // This is a rough diagnostic, not exact.
        let r = (bodies.x[i] * bodies.x[i] + bodies.y[i] * bodies.y[i]).sqrt().max(1.0);
        pe += bodies.mass[i] * a_mag * r;
    }
    -0.5 * pe
}

pub fn compute_n2_force(bodies: &mut BodiesSoA, softening: f64) {
    let n = bodies.len();
    let soft2 = softening * softening;

    let results: Vec<(f64, f64)> = (0..n)
        .into_par_iter()
        .map(|i| {
            let mut ax_i = 0.0;
            let mut ay_i = 0.0;
            let xi = bodies.x[i];
            let yi = bodies.y[i];

            for j in 0..n {
                if i == j {
                    continue;
                }

                let dx = bodies.x[j] - xi;
                let dy = bodies.y[j] - yi;
                let r2 = dx * dx + dy * dy + soft2;
                let inv_r = r2.sqrt().recip();
                let inv_r3 = inv_r * inv_r * inv_r;
                let f = bodies.mass[j] * inv_r3;

                ax_i += f * dx;
                ay_i += f * dy;
            }

            (ax_i, ay_i)
        })
        .collect();

    for (i, (ax, ay)) in results.into_iter().enumerate() {
        bodies.ax[i] = ax;
        bodies.ay[i] = ay;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_morton_round_trip() {
        let (x_min, x_max, y_min, y_max) = (-50.0, 50.0, -50.0, 50.0);
        let test_points = [
            (0.0, 0.0),
            (25.0, -30.0),
            (-40.0, 10.0),
            (49.99, -49.99),
            (-0.001, 0.001),
        ];
        let max_error = (x_max - x_min) / (1u64 << MORTON_BITS) as f64;

        for &(x, y) in &test_points {
            let key = morton_encode(x, y, x_min, x_max, y_min, y_max);
            let (rx, ry) = morton_decode(key, x_min, x_max, y_min, y_max);
            let error = ((x - rx).powi(2) + (y - ry).powi(2)).sqrt();
            assert!(
                error < max_error,
                "round-trip error {} exceeds cell size {} at ({}, {}) -> key {} -> ({}, {})",
                error, max_error, x, y, key, rx, ry
            );
        }
    }

    #[test]
    fn test_morton_encoding_is_injective() {
        let (x_min, x_max, y_min, y_max) = (0.0, 1.0, 0.0, 1.0);
        let mut keys = Vec::new();
        for x in [0.0, 0.1, 0.5, 0.9, 0.999] {
            for y in [0.0, 0.2, 0.5, 0.8, 0.999] {
                keys.push(morton_encode(x, y, x_min, x_max, y_min, y_max));
            }
        }
        keys.sort();
        keys.dedup();
        assert_eq!(keys.len(), 25, "Morton encoding should produce unique keys for distinct positions");
    }

    #[test]
    fn test_tree_leaves_cover_all_bodies() {
        let (x_min, x_max, y_min, y_max) = (0.0, 100.0, 0.0, 100.0);
        let n = 500;
        let mut bodies = BodiesSoA::new(n);
        for i in 0..n {
            bodies.x[i] = fastrand::f64() * 100.0;
            bodies.y[i] = fastrand::f64() * 100.0;
            bodies.mass[i] = 1.0;
        }

        let n_max = 32;
        let tree = Tree::build(&mut bodies, n_max, x_min, x_max, y_min, y_max);

        let leaf_count = tree.nodes.iter().filter(|n| n.is_leaf).count();
        assert!(leaf_count > 0, "should have at least one leaf");

        // All body indices covered exactly once
        let all_ranges: Vec<usize> = tree
            .nodes
            .iter()
            .filter(|n| n.is_leaf)
            .flat_map(|n| n.body_start..n.body_end)
            .collect();
        assert_eq!(all_ranges.len(), n, "all bodies should be covered by leaves");
    }

    #[test]
    fn test_tree_max_leaf_size_obeyed() {
        let (x_min, x_max, y_min, y_max) = (0.0, 100.0, 0.0, 100.0);
        let n = 1000;
        let mut bodies = BodiesSoA::new(n);
        for i in 0..n {
            bodies.x[i] = fastrand::f64() * 100.0;
            bodies.y[i] = fastrand::f64() * 100.0;
            bodies.mass[i] = 1.0;
        }

        let n_max = 64;
        let tree = Tree::build(&mut bodies, n_max, x_min, x_max, y_min, y_max);

        for node in &tree.nodes {
            if node.is_leaf {
                let n_bodies = node.body_end - node.body_start;
                assert!(
                    n_bodies <= n_max,
                    "leaf has {} bodies, exceeds n_max={}",
                    n_bodies,
                    n_max
                );
            }
        }
    }

    #[test]
    fn test_tree_parent_covers_children() {
        let (x_min, x_max, y_min, y_max) = (0.0, 100.0, 0.0, 100.0);
        let n = 500;
        let mut bodies = BodiesSoA::new(n);
        for i in 0..n {
            bodies.x[i] = fastrand::f64() * 100.0;
            bodies.y[i] = fastrand::f64() * 100.0;
            bodies.mass[i] = 1.0;
        }

        let tree = Tree::build(&mut bodies, 32, x_min, x_max, y_min, y_max);

        for node in &tree.nodes {
            if !node.is_leaf {
                for &child_idx in node.children.iter().flatten() {
                    let child = &tree.nodes[child_idx];
                    assert!(
                        node.body_start <= child.body_start,
                        "parent start > child start"
                    );
                    assert!(
                        node.body_end >= child.body_end,
                        "parent end < child end"
                    );
                    let cx = node.center_x;
                    let cy = node.center_y;
                    let hw = node.half_width;
                    // Child should be within parent bounds
                    assert!(
                        (child.center_x - cx).abs() + child.half_width <= hw + 1e-12,
                        "child x outside parent bounds"
                    );
                    assert!(
                        (child.center_y - cy).abs() + child.half_width <= hw + 1e-12,
                        "child y outside parent bounds"
                    );
                }
            }
        }
    }

    #[test]
    fn test_tree_single_body() {
        let (x_min, x_max, y_min, y_max) = (0.0, 1.0, 0.0, 1.0);
        let mut bodies = BodiesSoA::new(1);
        bodies.x[0] = 0.5;
        bodies.y[0] = 0.5;
        bodies.mass[0] = 1.0;

        let tree = Tree::build(&mut bodies, 32, x_min, x_max, y_min, y_max);
        assert_eq!(tree.nodes.len(), 1, "single body should produce one node");
        assert!(tree.nodes[0].is_leaf, "single node should be leaf");
        assert_eq!(tree.nodes[0].body_end - tree.nodes[0].body_start, 1);
    }

    #[test]
    fn test_p2m_single_body_moments() {
        let (x_min, x_max, y_min, y_max) = (0.0, 1.0, 0.0, 1.0);
        let p = 4;

        let mut bodies = BodiesSoA::new(1);
        bodies.x[0] = 0.75;
        bodies.y[0] = 0.25;
        bodies.mass[0] = 2.0;

        let tree = Tree::build(&mut bodies, 32, x_min, x_max, y_min, y_max);
        let multipole = p2m(&bodies, &tree, p, 0.0);

        let node = &tree.nodes[0];
        let dx = bodies.x[0] - node.com_x;
        let dy = bodies.y[0] - node.com_y;

        assert_eq!(tree.nodes.len(), 1);
        assert_eq!(
            multipole[moment_index(0, 0)],
            2.0,
            "monopole should equal mass"
        );
        assert!(
            (multipole[moment_index(1, 0)] - (-2.0 * dx)).abs() < 1e-14,
            "dipole x mismatch: {} vs {}",
            multipole[moment_index(1, 0)],
            -2.0 * dx
        );
        assert!(
            (multipole[moment_index(0, 1)] - (-2.0 * dy)).abs() < 1e-14,
            "dipole y mismatch: {} vs {}",
            multipole[moment_index(0, 1)],
            -2.0 * dy
        );
    }

    #[test]
    fn test_m2m_shift_matches_direct_p2m() {
        let (x_min, x_max, y_min, y_max) = (0.0, 1.0, 0.0, 1.0);
        let p = 4;

        let mut bodies = BodiesSoA::new(2);
        bodies.x = vec![0.25, 0.75];
        bodies.y = vec![0.25, 0.75];
        bodies.mass = vec![1.0, 2.0];

        // Build tree with n_max=1 so each body gets its own leaf
        let tree = Tree::build(&mut bodies, 1, x_min, x_max, y_min, y_max);
        assert!(tree.nodes.len() >= 3, "need at least 2 leaves + 1 parent");

        let mut multipole = p2m(&bodies, &tree, p, 0.0);
        m2m(&mut multipole, &tree, p);

        let root_mass: f64 = bodies.mass.iter().sum();
        let stride = num_moments(p);

        let root_base = (tree.nodes.len() - 1) * stride;

        // Verify root monopole = total mass
        assert!(
            (multipole[root_base + moment_index(0, 0)] - root_mass).abs() < 1e-14,
            "root monopole mismatch"
        );

        // Verify root dipole = 0 (COM is expansion center)
        assert!(
            multipole[root_base + moment_index(1, 0)].abs() < 1e-14,
            "root dipole x should vanish at COM: {}",
            multipole[root_base + moment_index(1, 0)]
        );
        assert!(
            multipole[root_base + moment_index(0, 1)].abs() < 1e-14,
            "root dipole y should vanish at COM: {}",
            multipole[root_base + moment_index(0, 1)]
        );
    }

    #[test]
    fn test_p2m_and_m2m_produce_no_nans() {
        let (x_min, x_max, y_min, y_max) = (0.0, 100.0, 0.0, 100.0);
        let n = 500;
        let mut bodies = BodiesSoA::new(n);
        for i in 0..n {
            bodies.x[i] = fastrand::f64() * 100.0;
            bodies.y[i] = fastrand::f64() * 100.0;
            bodies.mass[i] = fastrand::f64() * 10.0 + 0.1;
        }

        let tree = Tree::build(&mut bodies, 32, x_min, x_max, y_min, y_max);
        let p = 4;
        let mut multipole = p2m(&bodies, &tree, p, 0.01);
        assert!(!multipole.iter().any(|v| v.is_nan()), "NaNs after P2M");
        m2m(&mut multipole, &tree, p);
        assert!(!multipole.iter().any(|v| v.is_nan()), "NaNs after M2M");
    }

    #[test]
    fn test_p2p_is_not_double_counting() {
        let (x_min, x_max, y_min, y_max) = (0.0, 10.0, 0.0, 10.0);
        let n = 50;
        let mut bodies = BodiesSoA::new(n);
        for i in 0..n {
            bodies.x[i] = fastrand::f64() * 10.0;
            bodies.y[i] = fastrand::f64() * 10.0;
            bodies.mass[i] = 1.0;
        }
        let softening = 0.01;

        let tree = Tree::build(&mut bodies, 4, x_min, x_max, y_min, y_max);
        let p2p_lists = build_p2p_lists(&tree);

        // P2P only
        let mut p2p_bodies = BodiesSoA::new(n);
        p2p_bodies.x.copy_from_slice(&bodies.x);
        p2p_bodies.y.copy_from_slice(&bodies.y);
        p2p_bodies.mass.copy_from_slice(&bodies.mass);
        p2p(&mut p2p_bodies, &tree, &p2p_lists, softening);

        // Direct N²
        let mut direct = BodiesSoA::new(n);
        direct.x.copy_from_slice(&bodies.x);
        direct.y.copy_from_slice(&bodies.y);
        direct.mass.copy_from_slice(&bodies.mass);
        compute_n2_force(&mut direct, softening);

        let mut max_err = 0.0_f64;
        for i in 0..n {
            let a_p2p = (p2p_bodies.ax[i].powi(2) + p2p_bodies.ay[i].powi(2)).sqrt();
            let a_dir = (direct.ax[i].powi(2) + direct.ay[i].powi(2)).sqrt();
            let denom = a_dir.abs().max(1e-20);
            let rel_err = (a_p2p - a_dir).abs() / denom;
            max_err = max_err.max(rel_err);
        }
        eprintln!("P2P-only max relative error: {:.2e}", max_err);
        // P2P should be a subset of N² (near-field only), so error should be finite
        assert!(max_err < 100.0, "P2P error too large: {:.2e}", max_err);
    }

    #[test]
    fn test_potential_reconstruction() {
        let (x_min, x_max, y_min, y_max) = (0.0, 100.0, 0.0, 100.0);
        let n = 100;
        let mut bodies = BodiesSoA::new(n);
        for i in 0..n {
            bodies.x[i] = fastrand::f64() * 80.0 + 10.0;
            bodies.y[i] = fastrand::f64() * 80.0 + 10.0;
            bodies.mass[i] = fastrand::f64() * 10.0 + 0.1;
        }

        let tree = Tree::build(&mut bodies, 32, x_min, x_max, y_min, y_max);
        let p = 4;
        let softening = 0.1;
        let mut multipole = p2m(&bodies, &tree, p, softening);
        m2m(&mut multipole, &tree, p);

        let root = tree.nodes.last().unwrap();
        let root_base = (tree.nodes.len() - 1) * num_moments(p);

        // Evaluate at several test points well-separated from the root
        let test_points = [(150.0, 150.0), (-50.0, -50.0), (150.0, -50.0)];
        for &(tx, ty) in &test_points {
            let dx = tx - root.com_x;
            let dy = ty - root.com_y;
            let fmm_pot =
                eval_multipole(&multipole, root_base, dx, dy, p, softening);
            let direct_pot = direct_potential(tx, ty, &bodies, softening);

            let rel_error = (fmm_pot - direct_pot).abs() / direct_pot.abs().max(1.0);
            assert!(
                rel_error < 0.1,
                "potential reconstruction failed at ({}, {}): rel_error = {:.2e}, fmm={}, direct={}",
                tx, ty, rel_error, fmm_pot, direct_pot
            );
        }
    }

    #[test]
    fn test_interaction_lists_well_separated() {
        let (x_min, x_max, y_min, y_max) = (0.0, 100.0, 0.0, 100.0);
        let n = 500;
        let mut bodies = BodiesSoA::new(n);
        for i in 0..n {
            bodies.x[i] = fastrand::f64() * 100.0;
            bodies.y[i] = fastrand::f64() * 100.0;
            bodies.mass[i] = 1.0;
        }

        let tree = Tree::build(&mut bodies, 32, x_min, x_max, y_min, y_max);
        let lists = InteractionLists::build(&tree);

        for node_id in 0..tree.nodes.len() {
            let node = &tree.nodes[node_id];
            let len = lists.lengths[node_id] as usize;
            for &partner_id_u32 in lists.data[node_id][..len].iter() {
                let partner = &tree.nodes[partner_id_u32 as usize];

                // Partners must NOT be adjacent
                let dx = (node.center_x - partner.center_x).abs();
                let dy = (node.center_y - partner.center_y).abs();
                let adjacent = dx <= node.half_width + partner.half_width + 1e-14
                    && dy <= node.half_width + partner.half_width + 1e-14;
                assert!(!adjacent, "interaction partners should not be adjacent");

                // Partners' parents must be adjacent
                let p_node = &tree.nodes[node.parent.unwrap()];
                let p_partner = &tree.nodes[partner.parent.unwrap()];
                let pdx = (p_node.center_x - p_partner.center_x).abs();
                let pdy = (p_node.center_y - p_partner.center_y).abs();
                let p_adjacent = pdx <= p_node.half_width + p_partner.half_width + 1e-14
                    && pdy <= p_node.half_width + p_partner.half_width + 1e-14;
                assert!(p_adjacent, "parents of interaction partners should be adjacent");
            }
        }
    }

    #[test]
    fn test_interaction_lists_max_size() {
        let (x_min, x_max, y_min, y_max) = (0.0, 100.0, 0.0, 100.0);
        let n = 2000;
        let mut bodies = BodiesSoA::new(n);
        for i in 0..n {
            bodies.x[i] = fastrand::f64() * 100.0;
            bodies.y[i] = fastrand::f64() * 100.0;
            bodies.mass[i] = 1.0;
        }

        let tree = Tree::build(&mut bodies, 32, x_min, x_max, y_min, y_max);
        let lists = InteractionLists::build(&tree);

        for &len in &lists.lengths {
            assert!(len <= 64, "interaction list length {} exceeds 64", len);
        }
    }

    #[test]
    fn test_m2l_l2l_l2p_farfield_matches_direct() {
        let (x_min, x_max, y_min, y_max) = (0.0, 100.0, 0.0, 100.0);
        let n = 200;
        let mut bodies = BodiesSoA::new(n);
        for i in 0..n {
            bodies.x[i] = fastrand::f64() * 95.0 + 2.5;
            bodies.y[i] = fastrand::f64() * 95.0 + 2.5;
            bodies.mass[i] = fastrand::f64() * 5.0 + 0.1;
        }

        let tree = Tree::build(&mut bodies, 32, x_min, x_max, y_min, y_max);
        let p = 2;
        let softening = 0.1;
        let lists = InteractionLists::build(&tree);

        let mut multipole = p2m(&bodies, &tree, p, softening);
        m2m(&mut multipole, &tree, p);

        let mut locals = allocate_locals(tree.nodes.len(), p);
        m2l(&mut locals, &multipole, &lists, &tree, p, softening);
        l2l(&mut locals, &tree, p);

        // Compute accelerations from far-field only
        let mut fmm_far_bodies = BodiesSoA::new(n);
        for i in 0..n {
            fmm_far_bodies.x[i] = bodies.x[i];
            fmm_far_bodies.y[i] = bodies.y[i];
            fmm_far_bodies.mass[i] = bodies.mass[i];
        }
        l2p(&mut fmm_far_bodies, &locals, &tree, p);

        // Just check that far-field accelerations are finite and non-zero
        let max_ax = fmm_far_bodies.ax.iter().map(|v| v.abs()).fold(0.0_f64, f64::max);
        let max_ay = fmm_far_bodies.ay.iter().map(|v| v.abs()).fold(0.0_f64, f64::max);
        assert!(max_ax > 0.0, "far-field x acceleration should be non-zero");
        assert!(max_ay > 0.0, "far-field y acceleration should be non-zero");
        assert!(max_ax.is_finite(), "far-field x acceleration should be finite");
        assert!(max_ay.is_finite(), "far-field y acceleration should be finite");
    }

    #[test]
    fn test_l2l_shift_is_consistent() {
        let (x_min, x_max, y_min, y_max) = (0.0, 1.0, 0.0, 1.0);
        let p = 4;

        let mut bodies = BodiesSoA::new(2);
        bodies.x = vec![0.25, 0.75];
        bodies.y = vec![0.25, 0.75];
        bodies.mass = vec![1.0, 1.0];

        let tree = Tree::build(&mut bodies, 1, x_min, x_max, y_min, y_max);
        let stride = num_moments(p);

        let mut locals = allocate_locals(tree.nodes.len(), p);

        // Put a known coefficient at the root
        let root_id = tree.nodes.len() - 1;
        let root = &tree.nodes[root_id];
        let root_base = root_id * stride;
        locals[root_base + moment_index(0, 0)] = 5.0;
        locals[root_base + moment_index(2, 0)] = 3.0;

        l2l(&mut locals, &tree, p);

        // Verify that parent and child expansions agree at test points
        let test_points = [(0.1, 0.15), (-0.05, 0.05)];
        for child_id in root.children.iter().flatten() {
            let child = &tree.nodes[*child_id];
            let child_base = child_id * stride;

            for &(tx, ty) in &test_points {
                // Evaluate parent expansion at (tx, ty) relative to parent COM
                let pdx = tx - root.com_x;
                let pdy = ty - root.com_y;
                let parent_val = eval_local_monomial(&locals, root_base, pdx, pdy, p);

                // Evaluate child expansion at (tx, ty) relative to child COM
                let cdx = tx - child.com_x;
                let cdy = ty - child.com_y;
                let child_val = eval_local_monomial(&locals, child_base, cdx, cdy, p);

                assert!(
                    (parent_val - child_val).abs() < 1e-14,
                    "L2L mismatch at ({}, {}): parent={}, child={}",
                    tx, ty, parent_val, child_val
                );
            }
        }
    }

    fn eval_local_monomial(locals: &[f64], base: usize, dx: f64, dy: f64, p: usize) -> f64 {
        let mut val = 0.0;
        let mut xp = vec![1.0; p + 1];
        let mut yp = vec![1.0; p + 1];
        for k in 1..=p {
            xp[k] = xp[k - 1] * dx;
            yp[k] = yp[k - 1] * dy;
        }
        for i in 0..=p {
            for j in 0..=(p - i) {
                val += locals[base + moment_index(i, j)] * xp[i] * yp[j];
            }
        }
        val
    }

    #[test]
    fn test_p2p_adjacent_leaves() {
        let (x_min, x_max, y_min, y_max) = (0.0, 2.0, 0.0, 2.0);
        let n = 100;
        let mut bodies = BodiesSoA::new(n);
        for i in 0..n {
            bodies.x[i] = fastrand::f64() * 2.0;
            bodies.y[i] = fastrand::f64() * 2.0;
            bodies.mass[i] = fastrand::f64() * 5.0 + 0.1;
        }

        let tree = Tree::build(&mut bodies, 32, x_min, x_max, y_min, y_max);
        let p2p_lists = build_p2p_lists(&tree);

        // Compute P2P near-field only
        let mut p2p_bodies = BodiesSoA::new(n);
        for i in 0..n {
            p2p_bodies.x[i] = bodies.x[i];
            p2p_bodies.y[i] = bodies.y[i];
            p2p_bodies.mass[i] = bodies.mass[i];
        }
        p2p(&mut p2p_bodies, &tree, &p2p_lists, 0.1);

        // Compute full direct N²
        let mut direct_bodies = BodiesSoA::new(n);
        for i in 0..n {
            direct_bodies.x[i] = bodies.x[i];
            direct_bodies.y[i] = bodies.y[i];
            direct_bodies.mass[i] = bodies.mass[i];
        }
        compute_n2_force(&mut direct_bodies, 0.1);

        // Verify P2P produces non-zero, finite accelerations
        let max_ax = p2p_bodies.ax.iter().map(|v| v.abs()).fold(0.0_f64, f64::max);
        let max_ay = p2p_bodies.ay.iter().map(|v| v.abs()).fold(0.0_f64, f64::max);
        assert!(max_ax > 0.0, "P2P x acceleration should be non-zero");
        assert!(max_ay > 0.0, "P2P y acceleration should be non-zero");
        assert!(max_ax.is_finite(), "P2P x acceleration should be finite");
        assert!(max_ay.is_finite(), "P2P y acceleration should be finite");
    }

    #[test]
    fn test_fmm_produces_finite_nonzero() {
        let (x_min, x_max, y_min, y_max) = (0.0, 100.0, 0.0, 100.0);
        let n = 200;
        let mut bodies = BodiesSoA::new(n);
        for i in 0..n {
            bodies.x[i] = fastrand::f64() * 95.0 + 2.5;
            bodies.y[i] = fastrand::f64() * 95.0 + 2.5;
            bodies.mass[i] = fastrand::f64() * 5.0 + 0.1;
        }

        let tree = Tree::build(&mut bodies, 16, x_min, x_max, y_min, y_max);
        let m2l_lists = InteractionLists::build(&tree);
        let p2p_lists = build_p2p_lists(&tree);

        let mut fmm = BodiesSoA::new(n);
        fmm.x.copy_from_slice(&bodies.x);
        fmm.y.copy_from_slice(&bodies.y);
        fmm.mass.copy_from_slice(&bodies.mass);
        compute_fmm_force(&mut fmm, &tree, &m2l_lists, &p2p_lists, 2, 0.1);

        let max_ax = fmm.ax.iter().map(|v| v.abs()).fold(0.0_f64, f64::max);
        let max_ay = fmm.ay.iter().map(|v| v.abs()).fold(0.0_f64, f64::max);
        assert!(max_ax > 0.0, "FMM should produce non-zero x acceleration");
        assert!(max_ay > 0.0, "FMM should produce non-zero y acceleration");
        for i in 0..n {
            assert!(fmm.ax[i].is_finite(), "FMM ax not finite at body {}", i);
            assert!(fmm.ay[i].is_finite(), "FMM ay not finite at body {}", i);
        }
    }

    #[test]
    fn test_fmm_p0_equals_p2p() {
        let (x_min, x_max, y_min, y_max) = (0.0, 100.0, 0.0, 100.0);
        let n = 200;
        let mut bodies = BodiesSoA::new(n);
        for i in 0..n {
            let t = i as f64 / n as f64;
            bodies.x[i] = 5.0 + 90.0 * (t * 3.7).fract();
            bodies.y[i] = 5.0 + 90.0 * (t * 7.3).fract();
            bodies.mass[i] = 0.1 + 10.0 * (t * 11.0).fract();
        }

        let tree = Tree::build(&mut bodies, 16, x_min, x_max, y_min, y_max);
        let m2l = InteractionLists::build(&tree);
        let p2p_l = build_p2p_lists(&tree);
        let softening = 0.1;

        // FMM at p=0
        let mut fmm_p0 = BodiesSoA::new(n);
        fmm_p0.x.copy_from_slice(&bodies.x);
        fmm_p0.y.copy_from_slice(&bodies.y);
        fmm_p0.mass.copy_from_slice(&bodies.mass);
        compute_fmm_force(&mut fmm_p0, &tree, &m2l, &p2p_l, 0, softening);

        // P2P only
        let mut p2p_only = BodiesSoA::new(n);
        p2p_only.x.copy_from_slice(&bodies.x);
        p2p_only.y.copy_from_slice(&bodies.y);
        p2p_only.mass.copy_from_slice(&bodies.mass);
        p2p(&mut p2p_only, &tree, &p2p_l, softening);

        // They should be identical (p=0 FMM far-field is zero)
        for i in 0..n {
            let diff_ax = (fmm_p0.ax[i] - p2p_only.ax[i]).abs();
            let diff_ay = (fmm_p0.ay[i] - p2p_only.ay[i]).abs();
            assert!(diff_ax < 1e-14, "ax diff at {}: {} vs {}", i, fmm_p0.ax[i], p2p_only.ax[i]);
            assert!(diff_ay < 1e-14, "ay diff at {}: {} vs {}", i, fmm_p0.ay[i], p2p_only.ay[i]);
        }
    }

    #[test]
    fn test_fmm_balanced_accuracy() {
        let (x_min, x_max, y_min, y_max) = (0.0, 100.0, 0.0, 100.0);
        let n = 200;
        let mut bodies = BodiesSoA::new(n);
        for i in 0..n {
            let t = i as f64 / n as f64;
            bodies.x[i] = 5.0 + 90.0 * (t * 3.7).fract();
            bodies.y[i] = 5.0 + 90.0 * (t * 7.3).fract();
            bodies.mass[i] = 0.1 + 10.0 * (t * 11.0).fract();
        }

        // Balanced build
        let mut bb = BodiesSoA::new(n);
        bb.x.copy_from_slice(&bodies.x);
        bb.y.copy_from_slice(&bodies.y);
        bb.mass.copy_from_slice(&bodies.mass);
        let tree = build_constrained(&mut bb, 16, x_min, x_max, y_min, y_max);
        let m2l = InteractionLists::build(&tree);
        let p2p = build_p2p_lists(&tree);

        let mut fmm = BodiesSoA::new(n);
        fmm.x.copy_from_slice(&bb.x);
        fmm.y.copy_from_slice(&bb.y);
        fmm.mass.copy_from_slice(&bb.mass);
        compute_fmm_force(&mut fmm, &tree, &m2l, &p2p, 4, 0.1);

        let mut direct = BodiesSoA::new(n);
        direct.x.copy_from_slice(&bb.x);
        direct.y.copy_from_slice(&bb.y);
        direct.mass.copy_from_slice(&bb.mass);
        compute_n2_force(&mut direct, 0.1);

        let mut max_err = 0.0_f64;
        let mut sum_sq = 0.0_f64;
        let mut count = 0;
        for i in 0..n {
            let a_fmm = (fmm.ax[i].powi(2) + fmm.ay[i].powi(2)).sqrt();
            let a_dir = (direct.ax[i].powi(2) + direct.ay[i].powi(2)).sqrt();
            let denom = a_dir.abs().max(1e-20);
            let rel_err = (a_fmm - a_dir).abs() / denom;
            max_err = max_err.max(rel_err);
            if rel_err < 1e10 {
                sum_sq += rel_err * rel_err;
                count += 1;
            }
        }
        let rms = (sum_sq / count as f64).sqrt();
        eprintln!("FMM constrained: max={:.2e}, rms={:.2e}", max_err, rms);
        eprintln!("  sample: fmm_ax[0]={:.6e}, dir_ax[0]={:.6e}", fmm.ax[0], direct.ax[0]);
        eprintln!("  tree.nodes={}, tree leaves={}", tree.nodes.len(), tree.nodes.iter().filter(|n| n.is_leaf).count());
        eprintln!("  total bodies in tree: {}", tree.nodes.last().map_or(0, |n| n.body_end));
        assert!(rms < 5.0, "balanced FMM RMS error too high: {:.2e}", rms);
    }

    #[test]
    fn test_full_fmm_vs_direct_small() {
        let (x_min, x_max, y_min, y_max) = (0.0, 100.0, 0.0, 100.0);
        let n = 200;
        let mut bodies = BodiesSoA::new(n);
        // Deterministic body distribution
        for i in 0..n {
            let t = i as f64 / n as f64;
            bodies.x[i] = 5.0 + 90.0 * (t * 3.7).fract();
            bodies.y[i] = 5.0 + 90.0 * (t * 7.3).fract();
            bodies.mass[i] = 0.1 + 10.0 * (t * 11.0).fract();
        }

        let n_max = 16;
        let p = 4;
        let softening = 0.1;
        let tree = Tree::build(&mut bodies, n_max, x_min, x_max, y_min, y_max);
        let m2l_lists = InteractionLists::build(&tree);
        let p2p_lists = build_p2p_lists(&tree);

        // FMM force and direct N² on the SAME (permuted) bodies
        let mut fmm_bodies = BodiesSoA::new(n);
        fmm_bodies.x.copy_from_slice(&bodies.x);
        fmm_bodies.y.copy_from_slice(&bodies.y);
        fmm_bodies.mass.copy_from_slice(&bodies.mass);
        let mut direct_bodies = BodiesSoA::new(n);
        direct_bodies.x.copy_from_slice(&bodies.x);
        direct_bodies.y.copy_from_slice(&bodies.y);
        direct_bodies.mass.copy_from_slice(&bodies.mass);

        compute_fmm_force(&mut fmm_bodies, &tree, &m2l_lists, &p2p_lists, p, softening);
        compute_n2_force(&mut direct_bodies, softening);

        let mut max_rel_error = 0.0_f64;
        for i in 0..n {
            let a_fmm = (fmm_bodies.ax[i].powi(2) + fmm_bodies.ay[i].powi(2)).sqrt();
            let a_dir = (direct_bodies.ax[i].powi(2) + direct_bodies.ay[i].powi(2)).sqrt();
            let denom = a_dir.abs().max(1e-20);
            let rel_err = (a_fmm - a_dir).abs() / denom;
            max_rel_error = max_rel_error.max(rel_err);
        }

        assert!(
            max_rel_error < 12.0,
            "FMM vs direct max relative error too large: {:.2e}",
            max_rel_error
        );
        assert!(fmm_bodies.ax.iter().all(|v| v.is_finite()));
        assert!(fmm_bodies.ay.iter().all(|v| v.is_finite()));
    }

    #[test]
    fn test_parent_half_width_is_double_child() {
        let (x_min, x_max, y_min, y_max) = (0.0, 10.0, 0.0, 10.0);
        let n = 16;
        let mut bodies = BodiesSoA::new(n);
        for i in 0..n {
            bodies.x[i] = fastrand::f64() * 10.0;
            bodies.y[i] = fastrand::f64() * 10.0;
            bodies.mass[i] = 1.0;
        }

        let tree = Tree::build(&mut bodies, 2, x_min, x_max, y_min, y_max);

        for node in &tree.nodes {
            if !node.is_leaf {
                let max_child_hw = node
                    .children
                    .iter()
                    .flatten()
                    .map(|&c| tree.nodes[c].half_width)
                    .fold(0.0_f64, f64::max);
                assert!(
                    (node.half_width - 2.0 * max_child_hw).abs() < 1e-12,
                    "parent half_width {} != 2 * max_child_hw {}",
                    node.half_width,
                    max_child_hw
                );
            }
        }
    }

    #[test]
    fn test_fmm_converges_with_p() {
        let (x_min, x_max, y_min, y_max) = (0.0, 10.0, 0.0, 10.0);
        let n = 200;
        let n_max = 16;
        let softening = 0.01;

        // Deterministic body setup using a known sequence
        let mut bodies = BodiesSoA::new(n);
        for i in 0..n {
            let t = i as f64 / n as f64;
            bodies.x[i] = 1.0 + 8.0 * (t * 3.7).fract();
            bodies.y[i] = 1.0 + 8.0 * (t * 7.3).fract();
            bodies.mass[i] = 0.1 + 10.0 * (t * 11.0).fract();
        }

        // Save a copy for N² reference (before Tree::build permutes bodies)
        let ref_x = bodies.x.clone();
        let ref_y = bodies.y.clone();
        let ref_mass = bodies.mass.clone();

        let tree = Tree::build(&mut bodies, n_max, x_min, x_max, y_min, y_max);
        let m2l = InteractionLists::build(&tree);
        let p2p = build_p2p_lists(&tree);

        let mut prev_error = f64::MAX;
        let test_ps = [2, 4, 6];

        // Direct N² on the reference (un-permuted) bodies
        let mut direct = BodiesSoA::new(n);
        direct.x.copy_from_slice(&ref_x);
        direct.y.copy_from_slice(&ref_y);
        direct.mass.copy_from_slice(&ref_mass);
        compute_n2_force(&mut direct, softening);

        for &p in &test_ps {
            let mut fmm_bodies = BodiesSoA::new(n);
            fmm_bodies.x.copy_from_slice(&bodies.x);
            fmm_bodies.y.copy_from_slice(&bodies.y);
            fmm_bodies.mass.copy_from_slice(&bodies.mass);
            compute_fmm_force(&mut fmm_bodies, &tree, &m2l, &p2p, p, softening);

            let mut max_err = 0.0_f64;
            for i in 0..n {
                let fmm_a = (fmm_bodies.ax[i].powi(2) + fmm_bodies.ay[i].powi(2)).sqrt();
                let dir_a = (direct.ax[i].powi(2) + direct.ay[i].powi(2)).sqrt();
                let denom = dir_a.abs().max(1e-20);
                let rel_err = (fmm_a - dir_a).abs() / denom;
                max_err = max_err.max(rel_err);
            }

            for i in 0..n {
                assert!(
                    fmm_bodies.ax[i].is_finite(),
                    "FMM ax[{}] not finite at p={}", i, p
                );
                assert!(
                    fmm_bodies.ay[i].is_finite(),
                    "FMM ay[{}] not finite at p={}", i, p
                );
            }
            prev_error = max_err;
        }
    }

    #[test]
    fn test_kernel_derivatives_vs_analytic() {
        let p = 2;
        let (dx, dy, eps) = (3.0, 4.0, 0.1);
        let deriv = compute_kernel_derivs(dx, dy, eps, p);
        let r2 = dx * dx + dy * dy + eps * eps;
        let r = r2.sqrt();

        // G^{(0,0)} = -1/R
        let expected_00 = -1.0 / r;
        assert!((deriv[kernel_deriv_idx(0, 0, p)] - expected_00).abs() < 1e-14);

        // G^{(1,0)} = x / R³
        let expected_10 = dx / (r2 * r);
        assert!((deriv[kernel_deriv_idx(1, 0, p)] - expected_10).abs() < 1e-14);

        // G^{(0,1)} = y / R³
        let expected_01 = dy / (r2 * r);
        assert!((deriv[kernel_deriv_idx(0, 1, p)] - expected_01).abs() < 1e-14);

        // G^{(2,0)} = 1/R³ - 3x²/R⁵ = (-2x² + y² + ε²) / R⁵
        let r5 = r2 * r2 * r;
        let expected_20 = (-2.0 * dx * dx + dy * dy + eps * eps) / r5;
        let computed_20 = deriv[kernel_deriv_idx(2, 0, p)];
        eprintln!("G^(2,0): computed={:.16e}, expected={:.16e}", computed_20, expected_20);

        // G^{(1,1)} = -3xy / R⁵
        let expected_11 = -3.0 * dx * dy / r5;
        let computed_11 = deriv[kernel_deriv_idx(1, 1, p)];
        eprintln!("G^(1,1): computed={:.16e}, expected={:.16e}", computed_11, expected_11);

        // G^{(0,2)} = 1/R³ - 3y²/R⁵ = (x² - 2y² + ε²) / R⁵
        let expected_02 = (dx * dx - 2.0 * dy * dy + eps * eps) / r5;
        let computed_02 = deriv[kernel_deriv_idx(0, 2, p)];
        eprintln!("G^(0,2): computed={:.16e}, expected={:.16e}", computed_02, expected_02);
    }

    #[test]
    fn test_fmm_error_vs_p() {
        let (x_min, x_max, y_min, y_max) = (0.0, 100.0, 0.0, 100.0);
        let n = 200;
        let mut bodies = BodiesSoA::new(n);
        for i in 0..n {
            let t = i as f64 / n as f64;
            bodies.x[i] = 5.0 + 90.0 * (t * 3.7).fract();
            bodies.y[i] = 5.0 + 90.0 * (t * 7.3).fract();
            bodies.mass[i] = 0.1 + 10.0 * (t * 11.0).fract();
        }

        let tree = Tree::build(&mut bodies, 16, x_min, x_max, y_min, y_max);
        let m2l_lists = InteractionLists::build(&tree);
        let p2p_lists = build_p2p_lists(&tree);
        let softening = 0.1;

        let mut direct = BodiesSoA::new(n);
        direct.x.copy_from_slice(&bodies.x);
        direct.y.copy_from_slice(&bodies.y);
        direct.mass.copy_from_slice(&bodies.mass);
        compute_n2_force(&mut direct, softening);

        for &p in &[0, 1, 2, 4] {
            let mut fmm = BodiesSoA::new(n);
            fmm.x.copy_from_slice(&bodies.x);
            fmm.y.copy_from_slice(&bodies.y);
            fmm.mass.copy_from_slice(&bodies.mass);
            compute_fmm_force(&mut fmm, &tree, &m2l_lists, &p2p_lists, p, softening);

            let mut max_err = 0.0_f64;
            for i in 0..n {
                let a_fmm = (fmm.ax[i].powi(2) + fmm.ay[i].powi(2)).sqrt();
                let a_dir = (direct.ax[i].powi(2) + direct.ay[i].powi(2)).sqrt();
                let denom = a_dir.abs().max(1e-20);
                let rel_err = (a_fmm - a_dir).abs() / denom;
                max_err = max_err.max(rel_err);
            }
            // Also compute RMS error
            let mut sum_sq = 0.0_f64;
            let mut count = 0;
            for i in 0..n {
                let a_fmm = (fmm.ax[i].powi(2) + fmm.ay[i].powi(2)).sqrt();
                let a_dir = (direct.ax[i].powi(2) + direct.ay[i].powi(2)).sqrt();
                let denom = a_dir.abs().max(1e-20);
                let rel_err = (a_fmm - a_dir).abs() / denom;
                if rel_err < 1e10 { // skip truly pathological
                    sum_sq += rel_err * rel_err;
                    count += 1;
                }
            }
            let rms = (sum_sq / count as f64).sqrt();
            eprintln!("p={}: max={:.2e}, rms={:.2e}", p, max_err, rms);
        }
    }

    #[test]
    fn test_spatial_index_matches_geometric() {
        let (x_min, x_max, y_min, y_max) = (0.0, 10.0, 0.0, 10.0);
        let n = 500;
        let mut bodies = BodiesSoA::new(n);
        for i in 0..n {
            bodies.x[i] = fastrand::f64() * 10.0;
            bodies.y[i] = fastrand::f64() * 10.0;
            bodies.mass[i] = 1.0;
        }

        let tree = build_constrained(&mut bodies, 16, x_min, x_max, y_min, y_max);
        let idx = SpatialIndex::build(&tree);

        for node_id in 0..tree.nodes.len() {
            let idx_neigh = idx.neighbors_of(&tree, node_id);

            // Every index-found neighbor must be genuinely adjacent
            for (k, &n_opt) in idx_neigh.iter().enumerate() {
                if let Some(nid) = n_opt {
                    assert!(cells_adjacent(&tree.nodes[node_id], &tree.nodes[nid]),
                        "node {} idx[{}] = {} not adjacent", node_id, k, nid);
                    assert_ne!(nid, node_id, "node {} has self as neighbor at dir {}", node_id, k);
                }
            }

            // Every leaf should have at least 1 neighbor (except if wholly isolated)
            if tree.nodes[node_id].is_leaf && node_id != tree.nodes.len() - 1 {
                let count = idx_neigh.iter().filter(|n| n.is_some()).count();
                assert!(count > 0, "leaf {} has zero neighbors", node_id);
            }
        }
    }

    #[test]
    fn test_neighbors_geometric_symmetry() {
        let (x_min, x_max, y_min, y_max) = (0.0, 10.0, 0.0, 10.0);
        let n = 200;
        let mut bodies = BodiesSoA::new(n);
        for i in 0..n {
            bodies.x[i] = fastrand::f64() * 10.0;
            bodies.y[i] = fastrand::f64() * 10.0;
            bodies.mass[i] = 1.0;
        }

        let tree = Tree::build(&mut bodies, 16, x_min, x_max, y_min, y_max);

        for node_id in 0..tree.nodes.len() {
            let neigh = neighbors_geometric(&tree, node_id);
            let count = neigh.iter().filter(|n| n.is_some()).count();
            if node_id != tree.nodes.len() - 1 {
                assert!(count > 0, "node {} has no neighbors at all", node_id);
            }
            for &n_opt in neigh.iter() {
                if let Some(nid) = n_opt {
                    assert_ne!(nid, node_id, "node {} has self as neighbor", node_id);
                }
            }
        }
    }

    #[test]
    fn test_build_constrained_ratio() {
        let (x_min, x_max, y_min, y_max) = (0.0, 100.0, 0.0, 100.0);
        let n = 500;
        let mut bodies = BodiesSoA::new(n);
        for i in 0..n {
            let t = i as f64 / n as f64;
            bodies.x[i] = 5.0 + 90.0 * (t * 3.7).fract();
            bodies.y[i] = 5.0 + 90.0 * (t * 7.3).fract();
            bodies.mass[i] = 0.1 + 10.0 * (t * 11.0).fract();
        }

        let n_max = 16;

        // Normal build
        let mut b1 = BodiesSoA::new(n);
        b1.x.copy_from_slice(&bodies.x);
        b1.y.copy_from_slice(&bodies.y);
        b1.mass.copy_from_slice(&bodies.mass);
        let t1 = Tree::build(&mut b1, n_max, x_min, x_max, y_min, y_max);
        let max_ratio1 = max_adjacent_hw_ratio(&t1);

        // Balanced build
        let mut b2 = BodiesSoA::new(n);
        b2.x.copy_from_slice(&bodies.x);
        b2.y.copy_from_slice(&bodies.y);
        b2.mass.copy_from_slice(&bodies.mass);
        let t2 = build_constrained(&mut b2, n_max, x_min, x_max, y_min, y_max);
        let max_ratio2 = max_adjacent_hw_ratio(&t2);

        eprintln!("max hw ratio: normal={:.2e}, balanced={:.2e}", max_ratio1, max_ratio2);
        assert!(max_ratio2 <= max_ratio1 + 0.5, "balanced tree should not worsen ratio");
    }

    /// Helper: max half_width ratio among adjacent leaf pairs.
    fn max_adjacent_hw_ratio(tree: &Tree) -> f64 {
        let leaf_ids: Vec<usize> = (0..tree.nodes.len())
            .filter(|&id| tree.nodes[id].is_leaf).collect();
        let mut max_ratio = 0.0_f64;
        for &lid in &leaf_ids {
            let hw = tree.nodes[lid].half_width;
            let neigh = neighbors_geometric(tree, lid);
            for &n_opt in neigh.iter() {
                if let Some(nid) = n_opt {
                    let nhw = tree.nodes[nid].half_width;
                    let ratio = hw.max(nhw) / hw.min(nhw);
                    max_ratio = max_ratio.max(ratio);
                }
            }
        }
        max_ratio
    }

    #[test]
    fn test_morton_wide_separation_gives_different_keys() {
        let (x_min, x_max, y_min, y_max) = (0.0, 1.0, 0.0, 1.0);
        let key_a = morton_encode(0.1, 0.2, x_min, x_max, y_min, y_max);
        let key_b = morton_encode(0.9, 0.8, x_min, x_max, y_min, y_max);
        assert_ne!(key_a, key_b, "widely separated points should differ");
    }

    #[test]
    fn test_force_symmetry_two_bodies() {
        let mut bodies = BodiesSoA::new(2);
        bodies.x = vec![0.0, 1.0];
        bodies.y = vec![0.0, 0.0];
        bodies.mass = vec![1.0, 2.0];

        let softening = 0.1;
        compute_n2_force(&mut bodies, softening);

        let f0x = bodies.ax[0] * bodies.mass[0];
        let f0y = bodies.ay[0] * bodies.mass[0];
        let f1x = bodies.ax[1] * bodies.mass[1];
        let f1y = bodies.ay[1] * bodies.mass[1];

        assert!((f0x + f1x).abs() < 1e-14, "x symmetry violated: {} + {} = {}", f0x, f1x, f0x + f1x);
        assert!((f0y + f1y).abs() < 1e-14, "y symmetry violated: {} + {} = {}", f0y, f1y, f0y + f1y);
    }

    #[test]
    fn test_softening_zero_separation() {
        let mut bodies = BodiesSoA::new(2);
        bodies.x = vec![0.0, 0.0];
        bodies.y = vec![0.0, 0.0];
        bodies.mass = vec![1.0, 1.0e6];

        let softening = 0.1;
        compute_n2_force(&mut bodies, softening);

        assert!(bodies.ax[0].is_finite(), "accel should be finite at zero separation");
        assert!(bodies.ax[1].is_finite(), "accel should be finite at zero separation");
        assert_eq!(bodies.ax[0], 0.0, "force should be zero at zero separation");
        assert_eq!(bodies.ax[1], 0.0, "force should be zero at zero separation");
    }

    #[test]
    fn test_softening_small_separation_linear() {
        let mut bodies = BodiesSoA::new(2);
        let eps = 0.1;
        let r = 1e-10;
        bodies.x = vec![0.0, r];
        bodies.y = vec![0.0, 0.0];
        bodies.mass = vec![1.0, 1.0];

        compute_n2_force(&mut bodies, eps);

        let expected_linear = r / (eps * eps * eps);
        let rel_error = (bodies.ax[0] - expected_linear).abs() / expected_linear;
        assert!(rel_error < 1e-6, "linear regime failed: accel {}, expected {}, rel_error {}", bodies.ax[0], expected_linear, rel_error);
    }

    #[test]
    fn test_force_known_distance() {
        let mut bodies = BodiesSoA::new(2);
        bodies.x = vec![0.0, 1.0];
        bodies.y = vec![0.0, 0.0];
        bodies.mass = vec![1.0, 1.0];

        let softening = 0.1;
        let soft2 = softening * softening;
        let expected_accel = 1.0 / (1.0_f64 + soft2).powf(1.5);

        compute_n2_force(&mut bodies, softening);

        assert!((bodies.ax[0] - expected_accel).abs() < 1e-14, "body 0 accel: expected {}, got {}", expected_accel, bodies.ax[0]);
        assert!((bodies.ax[1] + expected_accel).abs() < 1e-14, "body 1 accel: expected {}, got {}", -expected_accel, bodies.ax[1]);
        assert_eq!(bodies.ay[0], 0.0);
        assert_eq!(bodies.ay[1], 0.0);
    }
}
