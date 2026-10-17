//! Trees, bushes, flowers, rocks and garden pieces. All stand on `plant`
//! sockets; trunks and rocks collide, foliage does not.
use super::*;
use pal::*;

/// A box with its twelve edges bevelled by `r` voxels.
pub(super) fn rounded(b: &mut B, min: [i32; 3], max: [i32; 3], r: i32, c: Rgb) {
    let r = r
        .min((max[0] - min[0]) / 3)
        .min((max[1] - min[1]) / 3)
        .min((max[2] - min[2]) / 3);
    b.fill(
        [min[0] + r, min[1], min[2] + r],
        [max[0] - r, max[1], max[2] - r],
        c,
    );
    b.fill(
        [min[0], min[1] + r, min[2] + r],
        [max[0], max[1] - r, max[2] - r],
        c,
    );
    b.fill(
        [min[0] + r, min[1] + r, min[2]],
        [max[0] - r, max[1] - r, max[2]],
        c,
    );
}

/// A leafy blob: bevelled, with a sunlit crown, shaded skirt and speckled
/// leaves on every face.
fn leaves(b: &mut B, min: [i32; 3], max: [i32; 3], c: Rgb) {
    let r = if max[0] - min[0] >= 12 { 2 } else { 1 };
    rounded(b, min, max, r, c);
    let light = mix(c, [1., 1., 0.75], 0.25);
    let dark = shade(c, 0.78);
    // Crown highlight and a darker underside band (both inside the volume).
    b.fill(
        [min[0] + r + 1, max[1] - 1, min[2] + r + 1],
        [max[0] - r - 1, max[1], max[2] - r - 1],
        light,
    );
    b.fill(
        [min[0] + r, min[1], min[2] + r],
        [max[0] - r, min[1] + 1, max[2] - r],
        dark,
    );
    let (w, h, d) = (max[0] - min[0], max[1] - min[1], max[2] - min[2]);
    let count = ((w + d) * h / 9).max(4);
    for _ in 0..count {
        let side = (b.rng.next_u64() % 4) as u8;
        let along = if side.is_multiple_of(2) { w } else { d };
        let u = b.rng.range(r, along - r - 1);
        let v = b.rng.range(min[1] + r, max[1] - r - 1);
        let s = b.rng.range(1, 3);
        let col = *b.rng.pick(&[
            dark,
            light,
            dark,
            mix(c, [0.85, 1.0, 0.45], 0.3),
            shade(c, 0.9),
        ]);
        let (lo, hi) = match side {
            0 => ([min[0] + u, v, max[2] - 1], [min[0] + u + s, v + s, max[2]]),
            2 => ([min[0] + u, v, min[2]], [min[0] + u + s, v + s, min[2] + 1]),
            1 => ([max[0] - 1, v, min[2] + u], [max[0], v + s, min[2] + u + s]),
            _ => ([min[0], v, min[2] + u], [min[0] + 1, v + s, min[2] + u + s]),
        };
        b.fill(
            [lo[0].max(min[0]), lo[1], lo[2].max(min[2])],
            [hi[0].min(max[0]), hi[1].min(max[1] - r), hi[2].min(max[2])],
            col,
        );
    }
    for _ in 0..(w * d / 18).max(3) {
        let x = b.rng.range(min[0] + r, max[0] - r - 1);
        let z = b.rng.range(min[2] + r, max[2] - r - 1);
        let col = *b.rng.pick(&[dark, mix(c, [1., 1., 0.7], 0.4), c]);
        let s = b.rng.range(1, 3);
        b.fill(
            [x, max[1] - 1, z],
            [(x + s).min(max[0] - r), max[1], (z + s).min(max[2] - r)],
            col,
        );
    }
}

fn trunk(b: &mut B, half: i32, height: i32, bark: Rgb) {
    b.solid([-half, 0, -half], [half, height, half], bark);
    let dark = shade(bark, 0.8);
    // Bark furrows and root flares.
    for k in 0..4u8 {
        let x = b.rng.range(-half, half - 1);
        b.fill_k(k, [x, 1, half - 1], [x + 1, height - 2, half], dark);
        b.fill_k(k, [-1, 0, half], [1, 2, half + 2], bark);
    }
}

/// Stair-stepped limb from the trunk towards `(dx, dz)` ending at `top`.
fn limb(b: &mut B, from: i32, dx: i32, dz: i32, top: i32, bark: Rgb) {
    let n = (top - from) / 3;
    for i in 0..n {
        let x = dx * i / n.max(1);
        let z = dz * i / n.max(1);
        b.fill(
            [x - 1, from + 3 * i, z - 1],
            [x + 1, from + 3 * i + 4, z + 1],
            bark,
        );
    }
}

/// A broadleaf tree: a trunk, stepped limbs and a cluster of leafy blobs.
struct Canopy<'a> {
    trunk: i32,
    girth: i32,
    spread: i32,
    blob: (i32, i32),
    blobs: usize,
    colors: &'a [Rgb],
}
fn cluster_tree(name: &str, c: Canopy, bark: Rgb) -> Tile {
    let mut b = B::new(name, "tree");
    trunk(&mut b, c.girth, c.trunk + 6, bark);
    let top = c.trunk;
    let s = c.blob.1 / 2 + 1;
    leaves(
        &mut b,
        [-s, top + 2, -s],
        [s, top + 2 + s * 2 - 2, s],
        c.colors[0],
    );
    let turn = b.rng.unit() * std::f32::consts::TAU;
    for i in 0..c.blobs {
        let a =
            turn + i as f32 / c.blobs as f32 * std::f32::consts::TAU + (b.rng.unit() - 0.5) * 0.6;
        let dist = c.spread as f32 * (0.65 + 0.35 * b.rng.unit());
        let (cx, cz) = ((a.cos() * dist) as i32, (a.sin() * dist) as i32);
        let size = b.rng.range(c.blob.0, c.blob.1 + 1);
        let half = size / 2;
        let y = top + b.rng.range(-2, half + 2);
        let h = size - b.rng.range(0, 3);
        limb(&mut b, top - 4, cx / 2, cz / 2, y + 2, bark);
        let col = c.colors[(i + 1) % c.colors.len()];
        leaves(
            &mut b,
            [cx - half, y, cz - half],
            [cx + half, y + h, cz + half],
            col,
        );
    }
    // A crown blob tops the cluster.
    let crown = c.blob.0 / 2 + 1;
    let y = top + s * 2 - 3;
    leaves(
        &mut b,
        [-crown, y, -crown],
        [crown, y + crown * 2, crown],
        c.colors[c.colors.len() - 1],
    );
    b.base("plant-base", "spot");
    b.finish()
}

fn fruit_tree() -> Tile {
    let mut t = B::new("Apple tree", "tree");
    trunk(&mut t, 2, 14, BARK);
    limb(&mut t, 10, 6, 2, 18, BARK);
    leaves(&mut t, [-11, 12, -11], [11, 28, 11], [0.38, 0.64, 0.28]);
    leaves(&mut t, [-8, 26, -8], [8, 32, 8], [0.44, 0.70, 0.32]);
    for _ in 0..16 {
        let k = (t.rng.next_u64() % 4) as u8;
        let x = t.rng.range(-10, 9);
        let y = t.rng.range(14, 27);
        t.fill_k(k, [x, y, 10], [x + 2, y + 2, 12], [0.86, 0.18, 0.16]);
    }
    for _ in 0..6 {
        let x = t.rng.range(-9, 8);
        let z = t.rng.range(-9, 8);
        t.fill([x, 32, z], [x + 2, 33, z + 2], [0.86, 0.18, 0.16]);
    }
    t.base("plant-base", "spot");
    t.finish()
}

fn pine() -> Tile {
    let mut b = B::new("Pine tree", "tree");
    trunk(&mut b, 3, 14, [0.42, 0.28, 0.20]);
    let greens = [[0.18, 0.42, 0.30], [0.22, 0.48, 0.33]];
    let mut y = 8;
    for (n, w) in [17, 15, 13, 11, 9, 7, 5, 3].into_iter().enumerate() {
        let c = greens[n % 2];
        b.fill([-w, y, -w], [w, y + 7, w], c);
        b.fill([-w, y, -w], [w, y + 1, w], shade(c, 0.75));
        b.fill(
            [-w + 1, y + 6, -w + 1],
            [w - 1, y + 7, w - 1],
            mix(c, [0.9, 1.0, 0.8], 0.2),
        );
        // Notched corners so tiers read as boughs.
        for k in 0..4u8 {
            b.fill_k(k, [w - 2, y + 4, w - 2], [w, y + 7, w], shade(c, 0.85));
        }
        y += 6;
    }
    b.fill([-1, y, -1], [1, y + 4, 1], greens[1]);
    b.base("plant-base", "spot");
    b.finish()
}

fn birch() -> Tile {
    let mut b = B::new("Birch tree", "tree");
    let h = 46;
    b.solid([-2, 0, -2], [2, h, 2], [0.93, 0.92, 0.88]);
    // Dark bark marks.
    for y in (2..h - 2).step_by(5) {
        let k = (b.rng.next_u64() % 4) as u8;
        let x = b.rng.range(-2, 1);
        b.fill_k(k, [x, y, 1], [x + 2, y + 1, 2], [0.25, 0.24, 0.22]);
    }
    limb(&mut b, 26, 6, 2, 34, [0.88, 0.87, 0.82]);
    let c = [0.70, 0.80, 0.32];
    leaves(&mut b, [-9, 30, -8], [8, 46, 9], c);
    leaves(&mut b, [2, 34, -4], [14, 48, 8], [0.80, 0.84, 0.30]);
    leaves(&mut b, [-6, 44, -5], [6, 54, 6], c);
    b.base("plant-base", "spot");
    b.finish()
}

fn poplar() -> Tile {
    let mut b = B::new("Poplar tree", "tree");
    trunk(&mut b, 2, 12, BARK);
    leaves(&mut b, [-7, 10, -7], [7, 50, 7], [0.32, 0.58, 0.28]);
    leaves(&mut b, [-5, 50, -5], [5, 62, 5], [0.38, 0.64, 0.30]);
    leaves(&mut b, [-9, 18, -6], [-7, 40, 6], [0.30, 0.55, 0.26]);
    b.base("plant-base", "spot");
    b.finish()
}

fn street_tree() -> Tile {
    let mut b = B::new("Street tree", "tree");
    trunk(&mut b, 2, 26, BARK);
    b.fill([-4, 0, -4], [4, 1, 4], [0.35, 0.36, 0.38]);
    b.fill([-3, 0, -3], [3, 1, 3], SOIL);
    limb(&mut b, 18, 6, -2, 28, BARK);
    let g = [[0.36, 0.62, 0.30], [0.42, 0.68, 0.32], [0.32, 0.56, 0.27]];
    leaves(&mut b, [-10, 24, -10], [10, 36, 10], g[0]);
    leaves(&mut b, [0, 26, -9], [12, 36, 3], g[2]);
    leaves(&mut b, [-11, 28, -2], [-1, 38, 10], g[1]);
    leaves(&mut b, [-6, 34, -6], [6, 42, 6], g[1]);
    b.base("plant-base", "spot");
    b.finish()
}

fn bush(name: &str, c: Rgb, flowers: Option<Rgb>, size: i32) -> Tile {
    let mut b = B::new(name, "bush");
    let h = size + size / 2;
    leaves(&mut b, [-size, 0, -size], [size, h, size], c);
    let side = size * 2 / 3;
    let ox = b.rng.range(-2, 3);
    leaves(
        &mut b,
        [ox - side, h - 3, -side + 1],
        [ox + side, h + side - 1, side - 1],
        mix(c, [1., 1., 0.8], 0.12),
    );
    leaves(
        &mut b,
        [size - side - 1, 0, -side - 2],
        [size + 2, side + 2, side - 2],
        shade(c, 0.92),
    );
    if let Some(f) = flowers {
        let dark = shade(f, 0.82);
        for _ in 0..size * 4 {
            let k = (b.rng.next_u64() % 4) as u8;
            let x = b.rng.range(-size + 2, size - 3);
            let y = b.rng.range(2, h - 2);
            let col = if b.rng.chance(30) { dark } else { f };
            b.fill_k(k, [x, y, size - 1], [x + 1, y + 1, size], col);
        }
        for _ in 0..size * 3 {
            let x = b.rng.range(ox - side + 2, ox + side - 3);
            let z = b.rng.range(-side + 3, side - 4);
            let col = if b.rng.chance(30) { dark } else { f };
            b.fill([x, h + side - 2, z], [x + 1, h + side - 1, z + 1], col);
        }
    }
    b.collide([-size, 0, -size], [size, h, size]);
    b.base("plant-base", "spot");
    b.finish()
}

fn flowers(name: &str, colors: &[Rgb], tall: bool) -> Tile {
    let mut b = B::new(name, "flowers");
    b.fill([-7, 0, -7], [7, 1, 7], SOIL);
    for z in (-6..6).step_by(2) {
        for x in (-6..6).step_by(2) {
            let jx = x + b.rng.range(0, 2);
            let jz = z + b.rng.range(0, 2);
            let h = if tall {
                b.rng.range(5, 9)
            } else {
                b.rng.range(2, 4)
            };
            b.fill([jx, 1, jz], [jx + 1, h, jz + 1], GRASS_DARK);
            let c = *b.rng.pick(colors);
            if tall {
                b.fill([jx - 1, h, jz - 1], [jx + 2, h + 1, jz + 2], c);
                b.fill([jx, h, jz], [jx + 1, h + 1, jz + 1], [0.45, 0.30, 0.15]);
            } else {
                b.fill([jx, h, jz], [jx + 1, h + 2, jz + 1], c);
            }
            if b.rng.chance(40) {
                b.fill([jx + 1, 1, jz], [jx + 2, 3, jz + 1], GRASS);
            }
        }
    }
    b.base("plant-base", "spot");
    b.finish()
}

fn flower_bed() -> Tile {
    let mut b = B::new("Flower bed", "flowers");
    b.solid([-12, 0, -8], [12, 3, 8], STONE_LIGHT);
    b.fill([-12, 2, -8], [12, 3, 8], [0.78, 0.76, 0.72]);
    b.fill([-11, 2, -7], [11, 3, 7], SOIL);
    let rows = [
        [0.95, 0.30, 0.35],
        [0.98, 0.85, 0.30],
        [0.72, 0.48, 0.90],
        WHITE,
    ];
    for (n, z) in (-6..6).step_by(3).enumerate() {
        for x in (-10..10).step_by(2) {
            let h = b.rng.range(1, 4);
            let jz = z + b.rng.range(0, 2);
            b.fill([x, 3, jz], [x + 1, 3 + h, jz + 1], GRASS_DARK);
            b.fill([x + 1, 3, jz], [x + 2, 4, jz + 1], GRASS);
            let c = if b.rng.chance(15) {
                rows[(n + 1) % 4]
            } else {
                rows[n % 4]
            };
            b.fill([x, 3 + h, jz], [x + 1, 4 + h, jz + 1], c);
        }
    }
    b.base("plant-base", "spot");
    b.finish()
}

fn sunflowers() -> Tile {
    let mut b = B::new("Sunflowers", "flowers");
    b.fill([-7, 0, -7], [7, 1, 7], SOIL);
    for (x, z) in [
        (-5, -4),
        (-1, -6),
        (3, -3),
        (-4, 2),
        (1, 1),
        (5, 4),
        (-1, 5),
    ] {
        let h = b.rng.range(10, 16);
        b.fill([x, 1, z], [x + 1, h, z + 1], [0.33, 0.55, 0.24]);
        b.fill(
            [x + 1, h / 2, z],
            [x + 3, h / 2 + 1, z + 1],
            [0.38, 0.62, 0.27],
        );
        b.fill(
            [x - 2, h / 2 + 2, z],
            [x, h / 2 + 3, z + 1],
            [0.38, 0.62, 0.27],
        );
        b.fill([x - 1, h, z - 1], [x + 2, h + 1, z + 2], [0.98, 0.80, 0.20]);
        b.fill([x, h + 1, z], [x + 1, h + 2, z + 1], [0.45, 0.28, 0.14]);
        b.fill([x - 1, h + 1, z], [x, h + 2, z + 1], [0.98, 0.84, 0.25]);
        b.fill([x + 1, h + 1, z], [x + 2, h + 2, z + 1], [0.98, 0.84, 0.25]);
    }
    b.base("plant-base", "spot");
    b.finish()
}

fn tall_grass() -> Tile {
    let mut b = B::new("Tall grass", "flowers");
    for _ in 0..40 {
        let x = b.rng.range(-7, 7);
        let z = b.rng.range(-7, 7);
        let h = b.rng.range(3, 9);
        let c = *b.rng.pick(&[GRASS_DARK, GRASS_DRY, MOSS, GRASS_LIGHT]);
        b.fill([x, 0, z], [x + 1, h, z + 1], c);
        if h > 6 && b.rng.chance(40) {
            b.fill([x, h, z], [x + 1, h + 2, z + 1], [0.85, 0.78, 0.50]);
        }
    }
    b.base("plant-base", "spot");
    b.finish()
}

fn mushrooms() -> Tile {
    let mut b = B::new("Mushrooms", "flowers");
    for (x, z, h, w) in [(-4, -2, 5, 3), (2, 2, 3, 2), (4, -4, 4, 2), (-1, 4, 2, 2)] {
        b.fill([x, 0, z], [x + 1, h, z + 1], [0.95, 0.92, 0.85]);
        b.fill(
            [x - w + 1, h, z - w + 1],
            [x + w, h + 2, z + w],
            [0.86, 0.18, 0.16],
        );
        b.fill([x, h + 2, z], [x + 1, h + 3, z + 1], [0.86, 0.18, 0.16]);
        b.fill(
            [x - w + 2, h + 1, z - w + 1],
            [x - w + 3, h + 2, z - w + 2],
            WHITE,
        );
        b.fill([x + w - 1, h + 1, z], [x + w, h + 2, z + 1], WHITE);
    }
    b.fill([-6, 0, -6], [6, 1, 6], MOSS);
    b.base("plant-base", "spot");
    b.finish()
}

/// A weathered rock: bevelled, overlapping chunks with lichen and moss.
fn rock(b: &mut B, x: i32, z: i32, size: i32, moss: bool) -> i32 {
    let base = shade([0.56, 0.55, 0.54], 0.92 + b.rng.unit() * 0.16);
    let h = size + size / 3;
    rounded(
        b,
        [x - size, 0, z - size + 1],
        [x + size, h, z + size - 1],
        2,
        base,
    );
    let s2 = size * 2 / 3;
    let ox = x + b.rng.range(-size / 3, size / 3 + 1);
    let oz = z + b.rng.range(-size / 3, size / 3 + 1);
    rounded(
        b,
        [ox - s2, h - 2, oz - s2],
        [ox + s2, h + s2, oz + s2],
        2,
        shade(base, 1.06),
    );
    let top = h + s2;
    for _ in 0..size * 2 {
        let k = (b.rng.next_u64() % 4) as u8;
        let u = b.rng.range(-size + 2, size - 3);
        let v = b.rng.range(1, h - 2);
        let c = *b
            .rng
            .pick(&[shade(base, 0.82), shade(base, 1.12), [0.70, 0.72, 0.52]]);
        let (lo, hi) = turn_box(k, [u, v, size - 2], [u + 2, v + 1, size - 1]);
        b.fill(
            [lo[0] + x, lo[1], lo[2] + z],
            [hi[0] + x, hi[1], hi[2] + z],
            c,
        );
    }
    if moss {
        b.fill(
            [ox - s2 + 2, top - 1, oz - s2 + 2],
            [ox + s2 - 3, top, oz + s2 - 2],
            MOSS,
        );
        b.fill([x - size + 2, h - 1, z + 1], [x - 1, h, z + size - 2], MOSS);
    }
    b.collide(
        [x - size + 1, 0, z - size + 1],
        [x + size - 1, h, z + size - 1],
    );
    top
}

fn boulder(name: &str, size: i32, moss: bool) -> Tile {
    let mut b = B::new(name, "rock");
    rock(&mut b, 0, 0, size, moss);
    for _ in 0..4 {
        let x = b.rng.range(-size - 3, size + 2);
        let z = b.rng.range(-size - 3, size + 2);
        if x.abs() > size || z.abs() > size {
            b.fill([x, 0, z], [x + 2, 1, z + 2], STONE_DARK);
        }
    }
    b.base("plant-base", "spot");
    b.finish()
}

fn rock_cluster() -> Tile {
    let mut b = B::new("Rock cluster", "rock");
    rock(&mut b, -4, -3, 6, true);
    rock(&mut b, 6, 4, 4, false);
    rock(&mut b, -3, 8, 3, false);
    b.base("plant-base", "spot");
    b.finish()
}

fn stump() -> Tile {
    let mut b = B::new("Tree stump", "rock");
    b.solid([-4, 0, -4], [4, 5, 4], BARK);
    b.fill([-3, 5, -3], [3, 6, 3], WOOD_LIGHT);
    b.fill([-2, 5, -2], [2, 6, 2], [0.80, 0.62, 0.40]);
    for k in 0..4u8 {
        b.fill_k(k, [-1, 0, 4], [1, 2, 7], BARK);
    }
    b.fill([4, 1, 0], [6, 2, 2], [0.96, 0.85, 0.60]);
    b.fill([4, 2, -1], [6, 3, 1], [0.92, 0.75, 0.45]);
    b.fill([-6, 0, -6], [-2, 1, -3], MOSS);
    b.base("plant-base", "spot");
    b.finish()
}

fn vegetable_patch() -> Tile {
    let mut b = B::new("Vegetable patch", "garden");
    b.solid([-13, 0, -9], [13, 3, 9], WOOD);
    b.fill([-12, 2, -8], [12, 3, 8], SOIL);
    let crops = [
        ([0.42, 0.72, 0.30], None),
        ([0.30, 0.56, 0.25], Some([0.95, 0.55, 0.15])),
        ([0.35, 0.60, 0.28], Some([0.88, 0.20, 0.18])),
    ];
    for (n, z) in [-6, -1, 4].into_iter().enumerate() {
        let (leaf, fruit) = crops[n];
        for x in (-11..11).step_by(3) {
            let h = b.rng.range(2, 4);
            b.fill([x, 3, z], [x + 2, 3 + h, z + 2], leaf);
            if let Some(f) = fruit
                && b.rng.chance(60)
            {
                b.fill([x, 3 + h, z], [x + 1, 4 + h, z + 1], f);
            }
        }
    }
    // Bean canes.
    for x in [-10, -4, 2, 8] {
        b.fill([x, 3, 8], [x + 1, 14, 9], WOOD_LIGHT);
        b.fill([x - 1, 6, 7], [x + 2, 12, 9], [0.32, 0.58, 0.26]);
    }
    b.base("plant-base", "spot");
    b.finish()
}

fn gazebo() -> Tile {
    let mut b = B::new("Gazebo", "garden");
    b.solid([-17, 0, -17], [17, 2, 17], STONE_LIGHT);
    b.fill([-15, 1, -15], [15, 2, 15], WOOD_LIGHT);
    for (x, z) in [(-15, -15), (13, -15), (-15, 13), (13, 13)] {
        b.solid([x, 2, z], [x + 2, 24, z + 2], WHITE);
    }
    for k in 0..4u8 {
        b.fill_k(k, [-15, 9, 13], [15, 10, 15], WHITE);
        for x in (-13..13).step_by(3) {
            b.fill_k(k, [x, 2, 14], [x + 1, 9, 15], WHITE);
        }
    }
    // Open the entrance on the front.
    let roof = [0.30, 0.52, 0.48];
    for k in 0..7 {
        let w = 19 - 3 * k;
        let c = if k % 2 == 0 { roof } else { shade(roof, 0.86) };
        b.fill([-w, 24 + 2 * k, -w], [w, 26 + 2 * k, w], c);
    }
    b.fill([-1, 38, -1], [1, 42, 1], LINE_YELLOW);
    b.solid([-6, 2, -6], [6, 7, 6], WOOD);
    b.base("plant-base", "spot");
    b.finish()
}

fn shed() -> Tile {
    let mut b = B::new("Garden shed", "garden");
    let wall = [0.55, 0.40, 0.30];
    b.solid([-10, 0, -8], [10, 20, 8], wall);
    for x in (-10..10).step_by(3) {
        b.fill([x, 0, 7], [x + 1, 20, 8], shade(wall, 0.85));
    }
    b.fill([-4, 0, 8], [4, 15, 9], WOOD_DARK);
    b.fill([5, 9, 8], [9, 14, 9], GLASS);
    for k in 0..4 {
        b.fill(
            [-12, 20 + 2 * k, -10 + 3 * k],
            [12, 22 + 2 * k, 10 - 3 * k],
            if k % 2 == 0 {
                ROOF_SLATE
            } else {
                shade(ROOF_SLATE, 0.86)
            },
        );
    }
    b.base("plant-base", "spot");
    b.finish()
}

pub fn tiles() -> Vec<Tile> {
    vec![
        cluster_tree(
            "Oak tree",
            Canopy {
                trunk: 22,
                girth: 3,
                spread: 15,
                blob: (16, 22),
                blobs: 6,
                colors: &[
                    [0.34, 0.58, 0.26],
                    [0.40, 0.65, 0.29],
                    [0.30, 0.53, 0.25],
                    [0.45, 0.70, 0.32],
                ],
            },
            BARK,
        ),
        cluster_tree(
            "Round tree",
            Canopy {
                trunk: 17,
                girth: 2,
                spread: 10,
                blob: (13, 17),
                blobs: 4,
                colors: &[[0.45, 0.70, 0.30], [0.52, 0.76, 0.34], [0.40, 0.64, 0.28]],
            },
            BARK,
        ),
        cluster_tree(
            "Maple tree",
            Canopy {
                trunk: 19,
                girth: 3,
                spread: 12,
                blob: (13, 18),
                blobs: 5,
                colors: &[[0.92, 0.52, 0.20], [0.86, 0.34, 0.20], [0.96, 0.66, 0.26]],
            },
            [0.40, 0.27, 0.20],
        ),
        cluster_tree(
            "Golden tree",
            Canopy {
                trunk: 18,
                girth: 2,
                spread: 11,
                blob: (12, 16),
                blobs: 4,
                colors: &[[0.95, 0.76, 0.24], [0.90, 0.64, 0.20], [0.98, 0.86, 0.36]],
            },
            [0.48, 0.42, 0.38],
        ),
        cluster_tree(
            "Blossom tree",
            Canopy {
                trunk: 15,
                girth: 2,
                spread: 10,
                blob: (11, 15),
                blobs: 5,
                colors: &[[0.96, 0.72, 0.82], [0.93, 0.60, 0.74], [0.99, 0.84, 0.90]],
            },
            [0.38, 0.26, 0.22],
        ),
        fruit_tree(),
        pine(),
        birch(),
        poplar(),
        street_tree(),
        bush("Round bush", [0.33, 0.58, 0.28], None, 6),
        bush("Big bush", [0.30, 0.54, 0.27], None, 9),
        bush(
            "Flowering bush",
            [0.36, 0.60, 0.30],
            Some([0.96, 0.62, 0.80]),
            6,
        ),
        bush("Hydrangea", [0.34, 0.56, 0.30], Some([0.55, 0.62, 0.95]), 7),
        flowers(
            "Tulips",
            &[[0.92, 0.22, 0.25], [0.98, 0.84, 0.25], [0.96, 0.55, 0.70]],
            false,
        ),
        flowers("Lavender", &[[0.62, 0.48, 0.86], [0.70, 0.56, 0.90]], false),
        sunflowers(),
        flower_bed(),
        tall_grass(),
        mushrooms(),
        boulder("Boulder", 9, true),
        boulder("Stone", 5, false),
        rock_cluster(),
        stump(),
        vegetable_patch(),
        gazebo(),
        shed(),
    ]
}
