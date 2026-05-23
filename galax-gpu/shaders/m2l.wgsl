@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) id: vec3<u32>) {
    let node = id.x;
    if (node >= params.num_nodes) { return; }
    let p = params.p;
    let stride = num_moments(p);
    let off = m2l_data[params.m2l_num_indices + node];
    let next_off = m2l_data[params.m2l_num_indices + node + 1u];
    if (next_off == off) { return; }
    let ctr = centers[node];
    let base_local = (params.num_nodes + node) * stride;
    // Precompute all kernel derivatives for each partner
    for (var k = off; k < next_off; k = k + 1u) {
        let partner = m2l_data[k];
        let pc = centers[partner];
        let dx = ctr.x - pc.x;
        let dy = ctr.y - pc.y;
        // Evaluate kernel derivatives G^{(k,l)} for k+l ≤ 2p
        let order = 2u * p;
        let r2 = dx * dx + dy * dy + params.eps * params.eps;
        let inv_sqrt = 1.0 / sqrt(r2);
        var invR: array<f32, 36>;
        invR[1] = inv_sqrt;
        for (var n = 3u; n <= 1u + 2u * order; n = n + 2u) {
            invR[n] = invR[n - 2u] / r2;
        }
        let stride_sq = stride * stride;
        var G: array<f32, 171>;
        for (var kk = 0u; kk <= order; kk = kk + 1u) {
            for (var ll = 0u; ll <= order - kk; ll = ll + 1u) {
                let mi = moment_index(kk, ll);
                let base_coeff = mi * stride_sq;
                var val = 0.0;
                for (var a = 0u; a < stride; a = a + 1u) {
                    for (var b = 0u; b < stride - a; b = b + 1u) {
                        let c = deriv_coeffs[base_coeff + a * stride + b];
                        if (c != 0.0) {
                            val += c * pow(dx, f32(a)) * pow(dy, f32(b));
                        }
                    }
                }
                let exp = 1u + 2u * kk + 2u * ll;
                G[mi] = val * invR[exp];
            }
        }
        // M2L accumulation: C_{I,J} += sum_{i,j} M_{i,j} * (-1)^{i+j} * G^{(i+I, j+J)} / (i! j! I! J!)
        let base_mp = partner * stride;
        for (var I = 0u; I <= p; I = I + 1u) {
            for (var J = 0u; J <= p - I; J = J + 1u) {
                var sum = 0.0;
                for (var i = 0u; i <= p; i = i + 1u) {
                    for (var j = 0u; j <= p - i; j = j + 1u) {
                        let mp_val = expansions[base_mp + moment_index(i, j)];
                        if (mp_val == 0.0) { continue; }
                        let kk = i + I;
                        let ll = j + J;
                        if (kk > order || ll > order - kk) { continue; }
                        let sign = select(1.0, -1.0, (i + j) % 2u == 1u);
                        let fac = sign / (fact(I) * fact(J));
                        sum += mp_val * G[moment_index(kk, ll)] * fac;
                    }
                }
                expansions[base_local + moment_index(I, J)] += sum;
            }
        }
    }
}
