//! Terrain cells: parks, paths, roads, streams, lots and the hill.
use super::*;
use pal::*;

/// Surface materials of heightfield columns.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Surf {
    Grass,
    Sand,
    Pebble,
    Gravel,
    Water,
    Dirt,
    Paving,
}
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Col {
    pub top: i32,
    pub surf: Surf,
}
const fn col(top: i32, surf: Surf) -> Col {
    Col { top, surf }
}
impl Col {
    /// Walkable height: water hides a shallow bed two voxels down.
    fn floor(self) -> i32 {
        if self.surf == Surf::Water {
            self.top - 2
        } else {
            self.top
        }
    }
}
fn surf_color(s: Surf) -> Rgb {
    match s {
        Surf::Grass => GRASS,
        Surf::Sand => SAND,
        Surf::Pebble => PEBBLE,
        Surf::Gravel => GRAVEL,
        Surf::Water => WATER,
        Surf::Dirt => DIRT,
        Surf::Paving => PAVING,
    }
}

/// Merge equal values of `f` over `[x0, x1) × [z0, z1)` into rectangles.
fn rects<T: Copy + PartialEq>(
    x0: i32,
    z0: i32,
    x1: i32,
    z1: i32,
    f: impl Fn(i32, i32) -> T,
) -> Vec<([i32; 4], T)> {
    let mut done = vec![];
    // Open rectangles: (xa, xb, za, value).
    let mut open: Vec<(i32, i32, i32, T)> = vec![];
    for z in z0..=z1 {
        let mut runs = vec![];
        if z < z1 {
            let mut x = x0;
            while x < x1 {
                let v = f(x, z);
                let mut e = x + 1;
                while e < x1 && f(e, z) == v {
                    e += 1;
                }
                runs.push((x, e, v));
                x = e;
            }
        }
        let mut next = vec![];
        for (xa, xb, za, v) in open {
            if let Some(i) = runs.iter().position(|r| r.0 == xa && r.1 == xb && r.2 == v) {
                runs.remove(i);
                next.push((xa, xb, za, v));
            } else {
                done.push(([xa, za, xb, z], v));
            }
        }
        for (xa, xb, v) in runs {
            next.push((xa, xb, z, v));
        }
        open = next;
    }
    done
}

/// Hill rock strata by height.
fn stratum(y: i32) -> Rgb {
    match y {
        ..3 => STONE_DARK,
        3..9 => STONE,
        9..14 => STONE_WARM,
        14..20 => STONE,
        20..26 => STONE_LIGHT,
        _ => STONE_WARM,
    }
}
const STRATA: [i32; 7] = [-8, 3, 9, 14, 20, 26, 99];
/// Fill a column range with dirt (lower ground) or rock strata (hill).
fn underfill(b: &mut B, r: [i32; 4], from: i32, to: i32, hill: bool) {
    if from >= to {
        return;
    }
    if !hill {
        b.fill([r[0], from, r[1]], [r[2], to, r[3]], DIRT);
        return;
    }
    for w in STRATA.windows(2) {
        let (a, c) = (w[0].max(from), w[1].min(to));
        if a < c {
            b.fill([r[0], a, r[1]], [r[2], c, r[3]], stratum(a));
        }
    }
}

/// A terrain heightfield region: bedrock, fill, surface layer and merged colliders.
/// `floor` is the height already filled below (bedrock top for lower ground).
fn heightfield(b: &mut B, area: [i32; 4], floor: i32, hill: bool, f: impl Fn(i32, i32) -> Col) {
    for (r, c) in rects(area[0], area[1], area[2], area[3], &f) {
        let skin = c.top - 2;
        underfill(b, r, floor, skin, hill && c.top > HILL - 8);
        let color = if hill && c.surf == Surf::Grass && c.top < HILL - 8 {
            GRASS
        } else {
            surf_color(c.surf)
        };
        b.fill([r[0], skin, r[1]], [r[2], c.top, r[3]], color);
        if hill && c.top >= HILL {
            // Topsoil shows as a dirt band on cut faces.
            b.fill([r[0], c.top - 4, r[1]], [r[2], c.top - 2, r[3]], DIRT);
        }
    }
    for (r, top) in rects(area[0], area[1], area[2], area[3], |x, z| f(x, z).floor()) {
        b.collide([r[0], BASE, r[1]], [r[2], top, r[3]]);
    }
}
fn bedrock(b: &mut B, area: [i32; 4]) {
    b.fill([area[0], BASE, area[1]], [area[2], -8, area[3]], BEDROCK);
}

/// Diorama cross-section detail: pebbles and roots on the cell's outer faces
/// (recolouring existing solid cells only).
fn face_specks(b: &mut B) {
    for side in 0..4u8 {
        for _ in 0..7 {
            let x = b.rng.range(-60, 56);
            let y = b.rng.range(-15, -9);
            let w = b.rng.range(2, 5);
            let h = b.rng.range(1, 3);
            let c = *b.rng.pick(&[PEBBLE, DIRT_DARK, STONE_DARK, STONE]);
            let c = if y < -9 { shade(c, 0.9) } else { c };
            b.fill_k(side, [x, y, -HALF], [x + w, y + h, -HALF + 1], c);
        }
    }
}

/// Lawn texture on a flat top: colour patches inside the surface layer plus a
/// few raised blades and flowers kept away from decoration sockets. `ok`
/// limits detail to columns whose surface really is at `y`.
fn lawn(
    b: &mut B,
    area: [i32; 4],
    y: i32,
    keep: &[[i32; 4]],
    density: u32,
    ok: &dyn Fn(i32, i32) -> bool,
) {
    let w = (area[2] - area[0]).max(0) as u32;
    let d = (area[3] - area[1]).max(0) as u32;
    let fits = |x0: i32, z0: i32, x1: i32, z1: i32| {
        ok(x0, z0) && ok(x1 - 1, z0) && ok(x0, z1 - 1) && ok(x1 - 1, z1 - 1)
    };
    let n = (w * d * density / 1100) as usize;
    for _ in 0..n {
        let pw = b.rng.range(3, 13);
        let pd = b.rng.range(3, 13);
        let x = b.rng.range(area[0], (area[2] - pw).max(area[0]) + 1);
        let z = b.rng.range(area[1], (area[3] - pd).max(area[1]) + 1);
        let (x1, z1) = ((x + pw).min(area[2]), (z + pd).min(area[3]));
        if !fits(x, z, x1, z1) {
            continue;
        }
        let c = *b
            .rng
            .pick(&[GRASS_DARK, GRASS_LIGHT, GRASS_DARK, GRASS_DRY]);
        b.fill([x, y - 1, z], [x1, y, z1], c);
    }
    let blades = (w * d * density / 700) as usize;
    for i in 0..blades {
        let x = b.rng.range(area[0], area[2] - 1);
        let z = b.rng.range(area[1], area[3] - 1);
        if !fits(x, z, x + 2, z + 1)
            || keep
                .iter()
                .any(|k| x + 2 > k[0] && x < k[2] && z + 2 > k[1] && z < k[3])
        {
            continue;
        }
        let flower = i % 5 == 0;
        let c = if flower {
            *b.rng.pick(&[
                [0.97, 0.95, 0.88],
                [0.98, 0.84, 0.30],
                [0.93, 0.55, 0.72],
                [0.62, 0.66, 0.95],
            ])
        } else {
            *b.rng.pick(&[GRASS_DARK, GRASS_LIGHT, MOSS])
        };
        let h = if flower { 1 } else { b.rng.range(1, 3) };
        let long = b.rng.chance(40);
        let (dx, dz) = if long { (2, 1) } else { (1, 1) };
        b.fill([x, y, z], [x + dx, y + h, z + dz], c);
    }
}
const ANY: &dyn Fn(i32, i32) -> bool = &|_, _| true;

/// Mown stripes on a garden lawn: alternate 8-voxel bands slightly lighter.
fn mown(b: &mut B, ok: &dyn Fn(i32, i32) -> bool) {
    let light = mix(GRASS, [0.9, 1.0, 0.6], 0.12);
    for x in (-HALF..HALF).step_by(16) {
        // Split each band into runs where the predicate holds.
        let mut z = -HALF;
        while z < HALF {
            if !(ok(x, z) && ok(x + 7, z)) {
                z += 1;
                continue;
            }
            let start = z;
            while z < HALF && ok(x, z) && ok(x + 7, z) {
                z += 1;
            }
            b.fill([x, GROUND - 1, start], [x + 8, GROUND, z], light);
        }
    }
}

/// Optional upward decoration sockets with varied facing; returns keep-out boxes.
fn plant_sockets(b: &mut B, kind: &str, spots: &[[i32; 3]], reach: i32) -> Vec<[i32; 4]> {
    let first = b.t.sockets.iter().filter(|s| s.kind == kind).count();
    let mut keep = vec![];
    for (n, p) in spots.iter().enumerate() {
        let turn = (b.rng.next_u64() % 4) as u8;
        b.socket(
            &format!("{kind}-{}", first + n),
            kind,
            *p,
            up_turned(turn),
            "spot",
            false,
        );
        keep.push([p[0] - reach, p[2] - reach, p[0] + reach, p[2] + reach]);
    }
    keep
}
/// Facing-controlled street sockets (furniture faces local +Z turned `turn`).
fn street_socket(b: &mut B, name: &str, p: [i32; 3], turn: u8) {
    b.socket(name, "street", p, up_turned(turn), "spot", false);
}

/// Jittered grid of spots over an area, skipping blocked rectangles.
fn spots(
    b: &mut B,
    area: [i32; 4],
    n: (i32, i32),
    jitter: i32,
    y: i32,
    blocked: &[[i32; 4]],
) -> Vec<[i32; 3]> {
    let mut out = vec![];
    for j in 0..n.1 {
        for i in 0..n.0 {
            let cx = area[0] + (area[2] - area[0]) * (2 * i + 1) / (2 * n.0);
            let cz = area[1] + (area[3] - area[1]) * (2 * j + 1) / (2 * n.1);
            let x = cx + b.rng.range(-jitter, jitter + 1);
            let z = cz + b.rng.range(-jitter, jitter + 1);
            if blocked
                .iter()
                .any(|r| x > r[0] - 12 && x < r[2] + 12 && z > r[1] - 12 && z < r[3] + 12)
            {
                continue;
            }
            out.push([x, y, z]);
        }
    }
    out
}

fn flat_ground(b: &mut B) {
    let all = [-HALF, -HALF, HALF, HALF];
    bedrock(b, all);
    heightfield(b, all, -8, false, |_, _| col(GROUND, Surf::Grass));
}

fn meadow() -> Tile {
    let mut b = B::new("Meadow", "park");
    flat_ground(&mut b);
    let s = spots(&mut b, [-HALF, -HALF, HALF, HALF], (3, 3), 8, GROUND, &[]);
    let keep = plant_sockets(&mut b, "plant", &s, 14);
    lawn(&mut b, [-HALF, -HALF, HALF, HALF], GROUND, &keep, 3, ANY);
    b.edges(["ground"; 4]);
    face_specks(&mut b);
    b.finish()
}

fn flower_meadow() -> Tile {
    let mut b = B::new("Wildflower meadow", "park");
    flat_ground(&mut b);
    let s = spots(&mut b, [-HALF, -HALF, HALF, HALF], (2, 2), 10, GROUND, &[]);
    let keep = plant_sockets(&mut b, "plant", &s, 16);
    lawn(&mut b, [-HALF, -HALF, HALF, HALF], GROUND, &keep, 2, ANY);
    // Drifts of colour: dense clusters of one flower each.
    let colors = [
        [0.96, 0.82, 0.26],
        [0.95, 0.45, 0.62],
        [0.70, 0.55, 0.92],
        [0.98, 0.97, 0.94],
        [0.95, 0.52, 0.25],
    ];
    for d in 0..12 {
        let c = colors[d % colors.len()];
        let cx = b.rng.range(-54, 54);
        let cz = b.rng.range(-54, 54);
        for _ in 0..34 {
            let x = cx + b.rng.range(-12, 13);
            let z = cz + b.rng.range(-12, 13);
            if keep
                .iter()
                .any(|k| x + 2 > k[0] && x < k[2] && z + 2 > k[1] && z < k[3])
                || x.abs() > 62
                || z.abs() > 62
            {
                continue;
            }
            let h = b.rng.range(1, 4);
            b.fill([x, GROUND, z], [x + 1, GROUND + h, z + 1], GRASS_DARK);
            b.fill([x, GROUND + h, z], [x + 2, GROUND + h + 1, z + 1], c);
        }
    }
    b.edges(["ground"; 4]);
    face_specks(&mut b);
    b.finish()
}

/// Gravel footpaths flush with the lawn; `arms` lists connected edges.
fn path_tile(name: &str, arms: [bool; 4]) -> Tile {
    let mut b = B::new(name, "path");
    let all = [-HALF, -HALF, HALF, HALF];
    bedrock(&mut b, all);
    let w = 9;
    let on_path = |x: i32, z: i32| {
        let centre = x.abs() < w && z.abs() < w;
        let arm = [
            arms[0] && x.abs() < w && z < 0,
            arms[1] && z.abs() < w && x > 0,
            arms[2] && x.abs() < w && z > 0,
            arms[3] && z.abs() < w && x < 0,
        ];
        centre || arm.iter().any(|a| *a)
    };
    let dead_end = arms.iter().filter(|a| **a).count() == 1;
    let plaza = |x: i32, z: i32| dead_end && x * x + z * z < 26 * 26;
    heightfield(&mut b, all, -8, false, |x, z| {
        if on_path(x, z) || plaza(x, z) {
            col(GROUND, Surf::Gravel)
        } else {
            col(GROUND, Surf::Grass)
        }
    });
    // Edging stones and gravel texture.
    for z in (-HALF..HALF).step_by(4) {
        for x in (-HALF..HALF).step_by(4) {
            let inside = on_path(x, z) || plaza(x, z);
            if !inside {
                continue;
            }
            let border = [(-4, 0), (4, 0), (0, -4), (0, 4)]
                .iter()
                .any(|(dx, dz)| !(on_path(x + dx, z + dz) || plaza(x + dx, z + dz)))
                && (x + 4).abs() <= HALF
                && (z + 4).abs() <= HALF;
            if border {
                let c = shade(STONE_LIGHT, 0.95 + b.rng.unit() * 0.1);
                b.fill([x, GROUND - 1, z], [x + 4, GROUND, z + 4], c);
            } else if b.rng.chance(30) {
                let c = *b.rng.pick(&[GRAVEL_DARK, [0.84, 0.79, 0.66]]);
                b.fill([x, GROUND - 1, z], [x + 2, GROUND, z + 2], c);
            }
        }
    }
    let mut blocked = vec![[-w, -w, w, w]];
    for (n, a) in arms.iter().enumerate() {
        if *a {
            let (lo, hi) = turn_box(((4 - n) % 4) as u8, [-w, 0, -HALF], [w, 0, 0]);
            blocked.push([lo[0], lo[2], hi[0], hi[2]]);
        }
    }
    if dead_end {
        blocked.push([-28, -28, 28, 28]);
    }
    let s = spots(
        &mut b,
        [-HALF, -HALF, HALF, HALF],
        (3, 3),
        6,
        GROUND,
        &blocked,
    );
    let keep = plant_sockets(&mut b, "plant", &s, 14);
    let mut keep = keep;
    keep.extend(
        blocked
            .iter()
            .map(|r| [r[0] - 1, r[1] - 1, r[2] + 1, r[3] + 1]),
    );
    if dead_end {
        // A lookout bench circle at the end of the path.
        for (n, p) in [[-14, GROUND, 14], [14, GROUND, 14]].iter().enumerate() {
            b.socket(
                &format!("seat-{n}"),
                "street",
                *p,
                up_turned(if n == 0 { 1 } else { 3 }),
                "spot",
                false,
            );
        }
    }
    lawn(&mut b, all, GROUND, &keep, 3, &|x, z| {
        !(on_path(x, z) || plaza(x, z))
    });
    b.edges(arms.map(|a| if a { "path" } else { "ground" }));
    face_specks(&mut b);
    b.finish()
}

fn plaza() -> Tile {
    let mut b = B::new("Fountain plaza", "park");
    let all = [-HALF, -HALF, HALF, HALF];
    bedrock(&mut b, all);
    let basin = |x: i32, z: i32| x * x + z * z < 22 * 22;
    heightfield(&mut b, all, -8, false, |x, z| {
        if basin(x, z) {
            col(GROUND, Surf::Paving)
        } else if x.abs() > 58 || z.abs() > 58 {
            col(GROUND, Surf::Grass)
        } else {
            col(GROUND, Surf::Paving)
        }
    });
    // Two-tone paving in concentric squares.
    for ring in (8..58).step_by(8) {
        let c = if (ring / 8) % 2 == 0 {
            [0.80, 0.76, 0.68]
        } else {
            [0.72, 0.70, 0.66]
        };
        for k in 0..4u8 {
            b.fill_k(
                k,
                [-ring, GROUND - 1, -ring - 2],
                [ring + 2, GROUND, -ring],
                c,
            );
        }
    }
    // Fountain: stone rim, water, central column with spout.
    for z in -22i32..22 {
        let half = (((22 * 22 - z * z) as f32).sqrt()) as i32;
        let inner = (((18 * 18 - z * z).max(0) as f32).sqrt()) as i32;
        if z.abs() < 18 {
            b.fill([-half, GROUND, z], [-inner, GROUND + 5, z + 1], STONE_LIGHT);
            b.fill([inner, GROUND, z], [half, GROUND + 5, z + 1], STONE_LIGHT);
            b.fill([-inner, GROUND, z], [inner, GROUND + 3, z + 1], WATER);
        } else {
            b.fill([-half, GROUND, z], [half, GROUND + 5, z + 1], STONE_LIGHT);
        }
    }
    b.collide([-20, GROUND, -20], [20, GROUND + 5, 20]);
    // Rim coping and ripples.
    b.patches(
        [-14, -14],
        [14, 14],
        GROUND + 3,
        &[WATER_LIGHT, WATER_DEEP],
        14,
        (1, 3),
    );
    b.solid([-4, GROUND, -4], [4, GROUND + 9, 4], STONE);
    b.fill([-6, GROUND + 9, -6], [6, GROUND + 11, 6], STONE_LIGHT);
    b.solid([-2, GROUND + 11, -2], [2, GROUND + 16, 2], STONE);
    b.fill([-3, GROUND + 16, -3], [3, GROUND + 17, 3], FOAM);
    for k in 0..4u8 {
        b.fill_k(k, [-1, GROUND + 4, 4], [1, GROUND + 15, 6], WATER_LIGHT);
        b.fill_k(k, [-2, GROUND + 3, 6], [2, GROUND + 4, 9], FOAM);
    }
    // Raised planters with tree sockets in each corner.
    let mut corners = vec![];
    for k in 0..4u8 {
        let (lo, hi) = turn_box(k, [34, GROUND, 34], [52, GROUND + 4, 52]);
        b.solid(lo, hi, STONE);
        b.fill(
            [lo[0] + 1, GROUND + 4, lo[2] + 1],
            [hi[0] - 1, GROUND + 5, hi[2] - 1],
            STONE_LIGHT,
        );
        b.fill(
            [lo[0] + 2, GROUND + 4, lo[2] + 2],
            [hi[0] - 2, GROUND + 5, hi[2] - 2],
            SOIL,
        );
        corners.push([(lo[0] + hi[0]) / 2, GROUND + 5, (lo[2] + hi[2]) / 2]);
    }
    plant_sockets(&mut b, "plant", &corners, 8);
    for (n, (x, z, t)) in [(0, 30, 2u8), (30, 0, 1), (0, -30, 0), (-30, 0, 3)]
        .into_iter()
        .enumerate()
    {
        street_socket(&mut b, &format!("bench-{n}"), [x, GROUND, z], t);
    }
    lawn(&mut b, all, GROUND, &[[-58, -58, 58, 58]], 2, &|x, z| {
        x.abs() > 58 || z.abs() > 58
    });
    b.edges(["ground"; 4]);
    face_specks(&mut b);
    b.finish()
}

fn pond() -> Tile {
    let mut b = B::new("Pond", "park");
    let all = [-HALF, -HALF, HALF, HALF];
    bedrock(&mut b, all);
    let radius = |x: i32, z: i32| {
        let a = (z as f32).atan2(x as f32);
        let r = ((x * x + z * z) as f32).sqrt();
        r - (34. + 5. * (2. * a).sin() + 3. * (3. * a + 1.).cos())
    };
    let shore = move |x: i32, z: i32| {
        let d = radius(x, z);
        if d < -4. {
            col(GROUND - 4, Surf::Water)
        } else if d < -2. {
            col(GROUND - 4, Surf::Pebble)
        } else if d < 1. {
            col(GROUND - 2, Surf::Sand)
        } else {
            col(GROUND, Surf::Grass)
        }
    };
    heightfield(&mut b, all, -8, false, shore);
    b.patches(
        [-34, -34],
        [34, 34],
        GROUND - 4,
        &[WATER_LIGHT, WATER_DEEP, WATER_LIGHT],
        26,
        (1, 4),
    );
    // Lily pads with the odd flower.
    for _ in 0..9 {
        let x = b.rng.range(-26, 24);
        let z = b.rng.range(-26, 24);
        if radius(x, z) < -8. && radius(x + 4, z + 4) < -8. {
            b.fill(
                [x, GROUND - 4, z],
                [x + 4, GROUND - 3, z + 3],
                [0.33, 0.62, 0.30],
            );
            b.fill(
                [x + 1, GROUND - 4, z + 3],
                [x + 3, GROUND - 3, z + 4],
                [0.33, 0.62, 0.30],
            );
            if b.rng.chance(40) {
                b.fill(
                    [x + 1, GROUND - 3, z + 1],
                    [x + 2, GROUND - 2, z + 2],
                    [0.98, 0.72, 0.85],
                );
            }
        }
    }
    // Reeds and shore stones.
    for _ in 0..40 {
        let a = b.rng.unit() * std::f32::consts::TAU;
        let r = 34. + 5. * (2. * a).sin() + 3. * (3. * a + 1.).cos() - 2.;
        let x = (a.cos() * r) as i32;
        let z = (a.sin() * r) as i32;
        if b.rng.chance(55) {
            let h = b.rng.range(4, 9);
            let top = shore(x, z).top;
            let c = *b.rng.pick(&[MOSS, GRASS_DARK, GRASS_DRY]);
            b.fill([x, top, z], [x + 1, top + h, z + 1], c);
            if h > 6 {
                b.fill([x, top + h, z], [x + 1, top + h + 2, z + 1], WOOD_DARK);
            }
        } else {
            let top = shore(x, z).top;
            let s = b.rng.range(2, 5);
            let c = *b.rng.pick(&[STONE, STONE_LIGHT]);
            b.fill([x, top, z], [x + s, top + s / 2 + 1, z + s], c);
        }
    }
    // A small wooden jetty.
    b.solid([-4, GROUND, 26], [4, GROUND + 1, 46], WOOD);
    for z in (28..46).step_by(3) {
        b.fill([-4, GROUND + 1, z], [4, GROUND + 2, z + 1], WOOD_LIGHT);
    }
    for (x, z) in [(-4, 27), (3, 27), (-4, 36), (3, 36)] {
        b.fill([x, GROUND - 4, z], [x + 1, GROUND + 3, z + 1], WOOD_DARK);
    }
    let s = [
        [-48, GROUND, -48],
        [48, GROUND, -48],
        [-50, GROUND, 46],
        [50, GROUND, 48],
    ];
    let mut keep = plant_sockets(&mut b, "plant", &s, 14);
    keep.push([-44, -44, 44, 50]);
    lawn(&mut b, all, GROUND, &keep, 3, &|x, z| radius(x, z) > 1.);
    b.edges(["ground"; 4]);
    face_specks(&mut b);
    b.finish()
}

fn community_garden() -> Tile {
    let mut b = B::new("Community garden", "park");
    flat_ground(&mut b);
    // Gravel walks between raised beds.
    b.fill([-56, GROUND - 1, -56], [56, GROUND, 56], GRAVEL);
    for z in (-56..56).step_by(4) {
        for x in (-56..56).step_by(4) {
            if b.rng.chance(25) {
                b.fill([x, GROUND - 1, z], [x + 2, GROUND, z + 2], GRAVEL_DARK);
            }
        }
    }
    let crops: [(Rgb, Rgb); 5] = [
        ([0.42, 0.72, 0.30], [0.55, 0.82, 0.38]),
        ([0.30, 0.55, 0.25], [0.93, 0.55, 0.20]),
        ([0.35, 0.60, 0.28], [0.86, 0.22, 0.20]),
        ([0.55, 0.75, 0.35], [0.95, 0.86, 0.35]),
        ([0.40, 0.48, 0.70], [0.60, 0.40, 0.75]),
    ];
    let mut n = 0;
    for bz in [-48, -22, 4] {
        for bx in [-50, -14, 22] {
            if bz == 4 && bx == 22 {
                continue;
            }
            let (leaf, fruit) = crops[n % crops.len()];
            n += 1;
            b.solid([bx, GROUND, bz], [bx + 28, GROUND + 4, bz + 18], WOOD);
            b.fill(
                [bx + 1, GROUND + 4, bz + 1],
                [bx + 27, GROUND + 5, bz + 17],
                WOOD_LIGHT,
            );
            b.fill(
                [bx + 2, GROUND + 3, bz + 2],
                [bx + 26, GROUND + 5, bz + 16],
                SOIL,
            );
            for row in 0..3 {
                let z = bz + 4 + row * 5;
                for x in (bx + 3..bx + 25).step_by(3) {
                    let h = b.rng.range(1, 4);
                    b.fill([x, GROUND + 5, z], [x + 2, GROUND + 5 + h, z + 2], leaf);
                    if b.rng.chance(35) {
                        b.fill(
                            [x, GROUND + 5 + h, z],
                            [x + 1, GROUND + 6 + h, z + 1],
                            fruit,
                        );
                    }
                }
            }
        }
    }
    // Tool shed and water butt in the open corner.
    b.solid([26, GROUND, 8], [54, GROUND + 22, 30], [0.40, 0.55, 0.42]);
    for x in (26..54).step_by(4) {
        b.fill(
            [x, GROUND, 30],
            [x + 1, GROUND + 22, 31],
            [0.34, 0.47, 0.36],
        );
    }
    b.fill([36, GROUND, 30], [44, GROUND + 16, 31], WOOD_DARK);
    b.fill([24, GROUND + 22, 6], [56, GROUND + 24, 32], ROOF_SLATE);
    b.fill(
        [28, GROUND + 24, 8],
        [52, GROUND + 26, 30],
        shade(ROOF_SLATE, 0.9),
    );
    b.fill([30, GROUND + 12, 31], [34, GROUND + 16, 32], GLASS);
    b.solid([46, GROUND, 34], [54, GROUND + 9, 42], [0.30, 0.42, 0.62]);
    b.fill(
        [45, GROUND + 8, 33],
        [55, GROUND + 9, 43],
        [0.24, 0.34, 0.52],
    );
    b.collide([24, GROUND, 6], [56, GROUND + 24, 32]);
    let s = [[-56, GROUND, 52], [0, GROUND, 52], [54, GROUND, 54]];
    let mut keep = plant_sockets(&mut b, "plant", &s, 10);
    keep.push([-58, -58, 58, 44]);
    lawn(
        &mut b,
        [-HALF, -HALF, HALF, HALF],
        GROUND,
        &keep,
        2,
        &|x, z| x.abs() >= 56 || z.abs() >= 56,
    );
    b.edges(["ground"; 4]);
    face_specks(&mut b);
    b.finish()
}

fn playground() -> Tile {
    let mut b = B::new("Playground", "park");
    flat_ground(&mut b);
    // Rubber safety surface and a sandpit.
    b.fill([-50, GROUND - 1, -50], [50, GROUND, 50], [0.80, 0.42, 0.33]);
    b.patches(
        [-50, -50],
        [50, 50],
        GROUND,
        &[[0.74, 0.38, 0.30], [0.84, 0.47, 0.37]],
        18,
        (2, 6),
    );
    b.solid([-44, GROUND, 8], [-8, GROUND + 3, 44], WOOD);
    b.fill([-42, GROUND + 1, 10], [-10, GROUND + 3, 42], SAND);
    b.fill(
        [-30, GROUND + 3, 20],
        [-24, GROUND + 6, 26],
        [0.92, 0.36, 0.30],
    );
    b.fill(
        [-20, GROUND + 3, 30],
        [-16, GROUND + 5, 34],
        [0.30, 0.55, 0.90],
    );
    // Slide tower.
    let red = [0.88, 0.30, 0.26];
    let yellow = [0.98, 0.80, 0.25];
    for (x, z) in [(14, -40), (30, -40), (14, -24), (30, -24)] {
        b.solid([x, GROUND, z], [x + 2, GROUND + 26, z + 2], WOOD_LIGHT);
    }
    b.solid([14, GROUND + 14, -40], [32, GROUND + 15, -22], WOOD);
    b.fill([12, GROUND + 26, -42], [34, GROUND + 28, -20], red);
    b.fill([16, GROUND + 28, -38], [30, GROUND + 30, -24], red);
    for i in 0..14 {
        b.fill(
            [32 + i, GROUND + 14 - i, -34],
            [34 + i, GROUND + 15 - i, -28],
            yellow,
        );
    }
    b.fill([32, GROUND + 1, -34], [47, GROUND + 2, -33], yellow);
    for y in (0..14).step_by(3) {
        b.fill([10, GROUND + y, -36], [14, GROUND + y + 1, -28], WOOD_DARK);
    }
    // Swing frame.
    for x in [8, 46] {
        b.solid(
            [x, GROUND, 14],
            [x + 2, GROUND + 22, 16],
            [0.30, 0.55, 0.90],
        );
        b.solid(
            [x, GROUND, 32],
            [x + 2, GROUND + 22, 34],
            [0.30, 0.55, 0.90],
        );
        b.fill(
            [x, GROUND + 22, 14],
            [x + 2, GROUND + 24, 34],
            [0.30, 0.55, 0.90],
        );
    }
    b.fill(
        [8, GROUND + 24, 23],
        [48, GROUND + 26, 25],
        [0.30, 0.55, 0.90],
    );
    for x in [18, 34] {
        b.fill([x, GROUND + 6, 23], [x + 1, GROUND + 24, 24], IRON);
        b.fill([x + 5, GROUND + 6, 23], [x + 6, GROUND + 24, 24], IRON);
        b.fill([x - 1, GROUND + 5, 21], [x + 7, GROUND + 6, 26], yellow);
    }
    let s = [
        [-54, GROUND, -54],
        [54, GROUND, 54],
        [-54, GROUND, 54],
        [56, GROUND, -56],
    ];
    let mut keep = plant_sockets(&mut b, "plant", &s, 10);
    keep.push([-52, -52, 52, 52]);
    lawn(
        &mut b,
        [-HALF, -HALF, HALF, HALF],
        GROUND,
        &keep,
        2,
        &|x, z| x.abs() >= 50 || z.abs() >= 50,
    );
    b.edges(["ground"; 4]);
    face_specks(&mut b);
    b.finish()
}

// ---------------------------------------------------------------------------
// Roads

#[derive(Clone, Copy, PartialEq)]
enum RoadStyle {
    Asphalt,
    Lane,
}
/// Edge kind of each side and whether it is a carriageway arm.
struct RoadSpec {
    arms: [Option<RoadStyle>; 4],
    crosswalks: bool,
    mid_crosswalk: bool,
}

const LANE_HALF: i32 = 24;

fn arm_half(s: RoadStyle) -> i32 {
    match s {
        RoadStyle::Asphalt => 32,
        RoadStyle::Lane => LANE_HALF,
    }
}

fn cobbles(b: &mut B, area: [i32; 4]) {
    // Running-bond brick setts in warm reds with darker joints.
    let tones = [
        [0.70, 0.45, 0.36],
        [0.64, 0.40, 0.33],
        [0.74, 0.50, 0.40],
        [0.67, 0.43, 0.36],
    ];
    for z in (area[1]..area[3]).step_by(4) {
        let offset = if (z / 4).rem_euclid(2) == 0 { 0 } else { 4 };
        let mut x = area[0] - offset;
        while x < area[2] {
            let a = x.max(area[0]);
            let c = (x + 8).min(area[2]);
            let tone = tones[b.rng.range(0, 4) as usize];
            b.fill([a, ROAD - 1, z], [c - 1, ROAD, (z + 3).min(area[3])], tone);
            x += 8;
        }
    }
}

/// Paving seams on a sidewalk area aligned to the global 8-voxel grid.
fn paving(b: &mut B, area: [i32; 4]) {
    b.fill([area[0], 0, area[1]], [area[2], GROUND, area[3]], PAVING);
    let mut x = (area[0] + 7).div_euclid(8) * 8;
    while x < area[2] {
        b.fill(
            [x, GROUND - 1, area[1]],
            [x + 1, GROUND, area[3]],
            PAVING_SEAM,
        );
        x += 8;
    }
    let mut z = (area[1] + 7).div_euclid(8) * 8;
    while z < area[3] {
        b.fill(
            [area[0], GROUND - 1, z],
            [area[2], GROUND, z + 1],
            PAVING_SEAM,
        );
        z += 8;
    }
    if b.rng.chance(60) {
        let cells_x = ((area[2] - area[0]) / 8).max(1);
        let cells_z = ((area[3] - area[1]) / 8).max(1);
        let gx = (area[0] + 7).div_euclid(8) * 8 + 8 * b.rng.range(0, cells_x);
        let gz = (area[1] + 7).div_euclid(8) * 8 + 8 * b.rng.range(0, cells_z);
        if gx + 8 <= area[2] && gz + 8 <= area[3] {
            b.fill(
                [gx + 1, GROUND - 1, gz + 1],
                [gx + 8, GROUND, gz + 8],
                [0.71, 0.70, 0.67],
            );
        }
    }
}

fn verge(b: &mut B, area: [i32; 4], gap: Option<[i32; 4]>) {
    b.fill([area[0], 0, area[1]], [area[2], GROUND, area[3]], GRASS);
    b.patches(
        [area[0], area[1]],
        [area[2], area[3]],
        GROUND,
        &[GRASS_DARK, GRASS_LIGHT],
        6,
        (2, 6),
    );
    if let Some(g) = gap {
        paving(b, g);
    }
}

fn road_tile(name: &str, spec: RoadSpec) -> Tile {
    let mut b = B::new(name, "road");
    let all = [-HALF, -HALF, HALF, HALF];
    bedrock(&mut b, all);
    b.fill([-HALF, -8, -HALF], [HALF, ROAD, HALF], DIRT);
    b.collide([-HALF, BASE, -HALF], [HALF, ROAD, HALF]);
    let arms = spec.arms;
    let n_arms = arms.iter().filter(|a| a.is_some()).count();
    let straight = n_arms == 2 && arms[0] == arms[2] && arms[0].is_some();
    let junction = n_arms >= 3 || (n_arms == 2 && !straight);
    // Junction box width follows the widest carriageway.
    let box_half = arms
        .iter()
        .flatten()
        .map(|s| arm_half(*s))
        .max()
        .unwrap_or(32);
    let main = arms.iter().flatten().any(|s| *s == RoadStyle::Asphalt);
    let surface = |b: &mut B, area: [i32; 4], style: RoadStyle| {
        let c = match style {
            RoadStyle::Asphalt => ASPHALT,
            RoadStyle::Lane => [0.52, 0.36, 0.31],
        };
        b.fill([area[0], -2, area[1]], [area[2], ROAD, area[3]], c);
        if style == RoadStyle::Lane {
            cobbles(b, area);
        }
    };
    let box_style = if main {
        RoadStyle::Asphalt
    } else {
        RoadStyle::Lane
    };
    surface(
        &mut b,
        [-box_half, -box_half, box_half, box_half],
        box_style,
    );
    let mut sidewalks: Vec<[i32; 4]> = vec![];
    let mut curbs: Vec<([i32; 4], u8)> = vec![];
    // Arms and side strips, authored on side 0 then turned into place.
    for (side, arm) in arms.iter().enumerate() {
        let k = ((4 - side) % 4) as u8;
        let area_k = |b: [i32; 4]| {
            let (lo, hi) = turn_box(k, [b[0], 0, b[1]], [b[2], 0, b[3]]);
            [lo[0], lo[2], hi[0], hi[2]]
        };
        match *arm {
            Some(style) => {
                let h = arm_half(style);
                surface(&mut b, area_k([-h, -HALF, h, -box_half]), style);
                // Sidewalk filling between a narrow lane and the box width.
                if h < box_half {
                    for (a, c) in [(-box_half, -h), (h, box_half)] {
                        let r = area_k([a, -HALF, c, -box_half]);
                        sidewalks.push(r);
                    }
                    curbs.push((area_k([-h - 2, -HALF, -h, -box_half]), 0));
                    curbs.push((area_k([h, -HALF, h + 2, -box_half]), 0));
                }
                if style == RoadStyle::Asphalt {
                    let mut dashes = vec![-60];
                    if !junction {
                        dashes.push(-44);
                    }
                    for z in dashes {
                        b.fill_k(k, [-1, ROAD - 1, z], [1, ROAD, z + 8], LINE_YELLOW);
                    }
                    if junction && spec.crosswalks {
                        let mut x = -30;
                        while x < 30 {
                            b.fill_k(k, [x, ROAD - 1, -48], [x + 4, ROAD, -37], LINE_WHITE);
                            x += 8;
                        }
                        b.fill_k(k, [0, ROAD - 1, -52], [30, ROAD, -50], LINE_WHITE);
                    }
                }
            }
            None => {
                // Curb, paving and a grass verge with a paved gap for paths.
                curbs.push((area_k([-box_half, -box_half - 2, box_half, -box_half]), 0));
                sidewalks.push(area_k([-box_half, -56, box_half, -box_half - 2]));
                let gap = area_k([-8, -HALF, 8, -56]);
                let left = area_k([-box_half, -HALF, -8, -56]);
                let right = area_k([8, -HALF, box_half, -56]);
                verge(&mut b, left, None);
                verge(&mut b, right, Some(gap));
                // Lamps and furniture near the curb, street trees in the verge.
                street_socket(
                    &mut b,
                    &format!("kerb-{side}a"),
                    turn_point(k, [-20, GROUND, -box_half - 6]),
                    k,
                );
                street_socket(
                    &mut b,
                    &format!("kerb-{side}b"),
                    turn_point(k, [20, GROUND, -box_half - 6]),
                    k,
                );
                let first = b.t.sockets.iter().filter(|s| s.kind == "plant").count();
                for (n, x) in [-22, 22].into_iter().enumerate() {
                    let turn = (b.rng.next_u64() % 4) as u8;
                    b.socket(
                        &format!("plant-{}", first + n),
                        "plant",
                        turn_point(k, [x, GROUND, -60]),
                        up_turned(turn),
                        "spot",
                        false,
                    );
                }
            }
        }
    }
    // Corner squares between consecutive sides.
    for c in 0..4usize {
        let k = ((4 - c) % 4) as u8;
        let area_k = |r: [i32; 4]| {
            let (lo, hi) = turn_box(k, [r[0], 0, r[1]], [r[2], 0, r[3]]);
            [lo[0], lo[2], hi[0], hi[2]]
        };
        let a0 = arms[c].is_some();
        let a1 = arms[(c + 1) % 4].is_some();
        let (mut x0, mut z1) = (box_half, -box_half);
        if a0 {
            curbs.push((area_k([box_half, -HALF, box_half + 2, -box_half]), 0));
            x0 = box_half + 2;
        }
        if a1 {
            curbs.push((area_k([box_half, -box_half - 2, HALF, -box_half]), 0));
            z1 = -box_half - 2;
        }
        let mut x1 = HALF;
        let mut z0 = -HALF;
        if !a1 {
            verge(&mut b, area_k([56, -HALF, HALF, z1]), None);
            x1 = 56;
        }
        if !a0 {
            verge(&mut b, area_k([x0, -HALF, x1, -56]), None);
            z0 = -56;
        }
        sidewalks.push(area_k([x0, z0, x1, z1]));
        // Rounded outer corner where the carriageway turns.
        if !a0 && !a1 && n_arms >= 2 {
            let r = 16;
            for z in 0..r {
                // Row inside the junction box corner at (box_half, -box_half).
                let dz = r - z;
                let reach = r - (((r * r - (dz - 1) * (dz - 1)) as f32).sqrt() as i32);
                if reach <= 0 {
                    continue;
                }
                let zz = -box_half + z;
                sidewalks.push(area_k([box_half - reach, zz, box_half, zz + 1]));
                let curb_w = 2.min(reach);
                curbs.push((
                    area_k([box_half - reach, zz, box_half - reach + curb_w, zz + 1]),
                    1,
                ));
            }
        }
    }
    for s in &sidewalks {
        paving(&mut b, *s);
        b.collide([s[0], ROAD, s[1]], [s[2], GROUND, s[3]]);
    }
    for (c, _) in &curbs {
        b.fill([c[0], 0, c[1]], [c[2], GROUND, c[3]], CURB);
        b.collide([c[0], ROAD, c[1]], [c[2], GROUND, c[3]]);
    }
    // Verge colliders: every non-road edge strip is raised.
    for (side, arm) in arms.iter().enumerate() {
        let k = ((4 - side) % 4) as u8;
        if arm.is_none() {
            b.collide_k(k, [-HALF, ROAD, -HALF], [HALF, GROUND, -56]);
        }
    }
    // Carriageway wear, drains and a manhole.
    if box_style == RoadStyle::Asphalt {
        b.patches(
            [-box_half + 3, -box_half + 3],
            [box_half - 3, box_half - 3],
            ROAD,
            &[ASPHALT_DARK, ASPHALT_LIGHT],
            6,
            (4, 12),
        );
    }
    for (side, arm) in arms.iter().enumerate() {
        let k = ((4 - side) % 4) as u8;
        if *arm == Some(RoadStyle::Asphalt) {
            for _ in 0..2 {
                let x = b.rng.range(-26, 18);
                let z = b.rng.range(-62, -box_half - 8);
                let w = b.rng.range(4, 9);
                let d = b.rng.range(4, 12);
                let c = *b.rng.pick(&[ASPHALT_DARK, ASPHALT_LIGHT]);
                b.fill_k(k, [x, ROAD - 1, z], [x + w, ROAD, z + d], c);
            }
            // Re-draw the centre dash over any wear patch.
            b.fill_k(k, [-1, ROAD - 1, -60], [1, ROAD, -52], LINE_YELLOW);
            if !junction {
                b.fill_k(k, [-1, ROAD - 1, -44], [1, ROAD, -36], LINE_YELLOW);
            }
        }
        if arm.is_none() && box_style == RoadStyle::Asphalt {
            // Storm drain in the gutter.
            b.fill_k(
                k,
                [10, ROAD - 1, -box_half],
                [18, ROAD, -box_half + 3],
                IRON,
            );
            for x in (11..18).step_by(2) {
                b.fill_k(
                    k,
                    [x, ROAD - 1, -box_half],
                    [x + 1, ROAD, -box_half + 2],
                    [0.40, 0.41, 0.43],
                );
            }
        }
    }
    if straight && arms[0] == Some(RoadStyle::Asphalt) {
        if spec.mid_crosswalk {
            let mut x = -30;
            while x < 30 {
                b.fill([x, ROAD - 1, -6], [x + 4, ROAD, 6], LINE_WHITE);
                x += 8;
            }
            b.fill([-1, ROAD - 1, -28], [1, ROAD, -20], LINE_YELLOW);
            b.fill([-1, ROAD - 1, 20], [1, ROAD, 28], LINE_YELLOW);
        } else {
            for z in [-28, -12, 4, 20] {
                b.fill([-1, ROAD - 1, z], [1, ROAD, z + 8], LINE_YELLOW);
            }
        }
    }
    if main {
        let (mx, mz) = if junction { (-3, -3) } else { (10, -20) };
        b.fill(
            [mx, ROAD - 1, mz],
            [mx + 7, ROAD, mz + 7],
            [0.33, 0.33, 0.35],
        );
        b.fill(
            [mx + 1, ROAD - 1, mz + 1],
            [mx + 6, ROAD, mz + 6],
            [0.20, 0.21, 0.23],
        );
        b.fill(
            [mx + 2, ROAD - 1, mz + 3],
            [mx + 5, ROAD, mz + 4],
            [0.33, 0.33, 0.35],
        );
    }
    b.edges(std::array::from_fn(|n| match arms[n] {
        Some(RoadStyle::Asphalt) => "road",
        Some(RoadStyle::Lane) => "lane",
        None => "curb",
    }));
    face_specks(&mut b);
    b.finish()
}

fn roads() -> Vec<Tile> {
    use RoadStyle::*;
    let spec = |arms, crosswalks, mid| RoadSpec {
        arms,
        crosswalks,
        mid_crosswalk: mid,
    };
    vec![
        road_tile(
            "Road straight",
            spec([Some(Asphalt), None, Some(Asphalt), None], false, false),
        ),
        road_tile(
            "Road crosswalk",
            spec([Some(Asphalt), None, Some(Asphalt), None], false, true),
        ),
        road_tile(
            "Road corner",
            spec([Some(Asphalt), Some(Asphalt), None, None], true, false),
        ),
        road_tile(
            "Road T",
            spec(
                [Some(Asphalt), Some(Asphalt), Some(Asphalt), None],
                true,
                false,
            ),
        ),
        road_tile("Road crossing", spec([Some(Asphalt); 4], true, false)),
        road_tile(
            "Lane straight",
            spec([Some(Lane), None, Some(Lane), None], false, false),
        ),
        road_tile(
            "Lane corner",
            spec([Some(Lane), Some(Lane), None, None], false, false),
        ),
        road_tile(
            "Lane T",
            spec([Some(Lane), Some(Lane), Some(Lane), None], false, false),
        ),
        road_tile(
            "Road T lane",
            spec(
                [Some(Asphalt), Some(Lane), Some(Asphalt), None],
                true,
                false,
            ),
        ),
        road_tile(
            "Road lane crossing",
            spec(
                [Some(Asphalt), Some(Lane), Some(Asphalt), Some(Lane)],
                true,
                false,
            ),
        ),
    ]
}

// ---------------------------------------------------------------------------
// Streams

/// Cross-section of a stream by distance from its centre line.
fn stream_col(d: f32) -> Col {
    if d < 16. {
        col(WATER_Y, Surf::Water)
    } else if d < 18. {
        col(WATER_Y, Surf::Pebble)
    } else if d < 21. {
        col(-2, Surf::Sand)
    } else if d < 24. {
        col(0, Surf::Grass)
    } else {
        col(GROUND, Surf::Grass)
    }
}
/// Banks: reeds, stones and ripples along a stream whose centre is `centre(x, z)`.
fn stream_dressing(b: &mut B, dist: &dyn Fn(i32, i32) -> f32, keep: &[[i32; 4]]) {
    // Ripples and darker deep water near the centre line.
    for _ in 0..90 {
        let x = b.rng.range(-HALF, HALF - 4);
        let z = b.rng.range(-HALF, HALF - 4);
        let d = dist(x, z);
        if d < 13. && dist(x + 3, z + 3) < 13. {
            let deep = d < 7.;
            let c = if deep && b.rng.chance(60) {
                WATER_DEEP
            } else {
                WATER_LIGHT
            };
            let long = b.rng.range(2, 6);
            if b.rng.chance(50) {
                b.fill([x, WATER_Y - 1, z], [x + long, WATER_Y, z + 1], c);
            } else {
                b.fill([x, WATER_Y - 1, z], [x + 1, WATER_Y, z + long], c);
            }
        }
    }
    for _ in 0..70 {
        let x = b.rng.range(-HALF, HALF - 3);
        let z = b.rng.range(-HALF, HALF - 3);
        let d = dist(x, z);
        if keep
            .iter()
            .any(|k| x + 4 > k[0] && x < k[2] && z + 4 > k[1] && z < k[3])
        {
            continue;
        }
        let top = stream_col(d).top;
        if (16. ..23.).contains(&d) && dist(x + 3, z + 3) > 16. {
            if b.rng.chance(50) {
                let s = b.rng.range(2, 5);
                let c = *b.rng.pick(&[STONE, STONE_LIGHT, STONE_DARK, PEBBLE]);
                let h = b.rng.range(1, 3);
                b.fill([x, top, z], [x + s, top + h, z + s], c);
            } else {
                let h = b.rng.range(3, 8);
                let c = *b.rng.pick(&[MOSS, GRASS_DARK, GRASS_DRY]);
                b.fill([x, top, z], [x + 1, top + h, z + 1], c);
                if h > 5 {
                    b.fill([x, top + h, z], [x + 1, top + h + 2, z + 1], WOOD_DARK);
                }
            }
        }
    }
}

fn stream_tile(name: &str, bend: bool, crossing: bool) -> Tile {
    let mut b = B::new(name, "water");
    let all = [-HALF, -HALF, HALF, HALF];
    bedrock(&mut b, all);
    let dist = move |x: i32, z: i32| -> f32 {
        // Distances are taken at voxel centres so both sides stay symmetric.
        let (x, z) = (x as f32 + 0.5, z as f32 + 0.5);
        if bend {
            ((x - 64.).hypot(z + 64.) - 64.).abs()
        } else {
            x.abs()
        }
    };
    let path = move |x: i32, z: i32| crossing && z.abs() < 9 && dist(x, z) >= 24.;
    heightfield(&mut b, all, -8, false, |x, z| {
        if path(x, z) {
            col(GROUND, Surf::Gravel)
        } else {
            stream_col(dist(x, z))
        }
    });
    let mut keep = vec![];
    let mut spots = vec![];
    if bend {
        spots.extend([
            [-44, GROUND, 44],
            [-48, GROUND, -12],
            [14, GROUND, 48],
            [52, GROUND, -52],
        ]);
    } else {
        spots.extend([
            [-44, GROUND, -40],
            [44, GROUND, -40],
            [-44, GROUND, 40],
            [44, GROUND, 40],
        ]);
        if !crossing {
            spots.extend([[-46, GROUND, 0], [46, GROUND, 2]]);
        }
    }
    keep.extend(plant_sockets(&mut b, "plant", &spots, 13));
    if crossing {
        keep.push([-HALF, -12, HALF, 12]);
        b.socket(
            "crossing",
            "crossing",
            [0, GROUND, 0],
            up(),
            "span-48",
            true,
        );
    }
    stream_dressing(&mut b, &dist, &keep);
    lawn(
        &mut b,
        [-HALF, -HALF, HALF, HALF],
        GROUND,
        &keep,
        3,
        &|x, z| dist(x, z) > 25. && !path(x, z),
    );
    let mut edges = if bend {
        ["stream", "stream", "ground", "ground"]
    } else {
        ["stream", "ground", "stream", "ground"]
    };
    if crossing {
        edges[1] = "path";
        edges[3] = "path";
    }
    b.edges(edges);
    face_specks(&mut b);
    b.finish()
}

/// A road carried over a stream on a low stone bridge.
fn road_bridge() -> Tile {
    let mut b = B::new("Road bridge", "road");
    let all = [-HALF, -HALF, HALF, HALF];
    bedrock(&mut b, all);
    // The stream runs along X under the deck.
    let dist = |x: i32, z: i32| -> f32 {
        let _ = x;
        (z as f32 + 0.5).abs()
    };
    let deck = |x: i32, z: i32| x.abs() < 56 && z.abs() < 26;
    // Banks and water everywhere, then the bridge deck on top.
    heightfield(&mut b, [-HALF, -26, HALF, 26], -8, false, |x, z| {
        let c = stream_col(dist(x, z));
        if deck(x, z) && c.surf != Surf::Water {
            col(ROAD - 2, Surf::Dirt)
        } else {
            c
        }
    });
    // Ordinary road and sidewalks beyond the banks.
    for (z0, z1) in [(-HALF, -26), (26, HALF)] {
        b.fill([-HALF, -8, z0], [HALF, ROAD, z1], DIRT);
        b.collide([-HALF, BASE, z0], [HALF, ROAD, z1]);
        b.fill([-32, -2, z0], [32, ROAD, z1], ASPHALT);
        for (a, c) in [(-56, -34), (34, 56)] {
            paving(&mut b, [a, z0, c, z1]);
            b.collide([a, ROAD, z0], [c, GROUND, z1]);
        }
        for (a, c) in [(-34, -32), (32, 34)] {
            b.fill([a, 0, z0], [c, GROUND, z1], CURB);
            b.collide([a, ROAD, z0], [c, GROUND, z1]);
        }
        for (a, c) in [(-HALF, -56), (56, HALF)] {
            verge(&mut b, [a, z0, c, z1], None);
            b.collide([a, ROAD, z0], [c, GROUND, z1]);
        }
    }
    // Deck: carriageway, raised walks and stone parapets.
    b.solid([-56, -2, -26], [56, ROAD, 26], STONE);
    b.fill([-32, -2, -26], [32, ROAD, 26], ASPHALT);
    for (a, c) in [(-52, -34), (34, 52)] {
        b.fill([a, ROAD, -26], [c, GROUND, 26], [0.70, 0.69, 0.66]);
        b.collide([a, ROAD, -26], [c, GROUND, 26]);
        for z in (-24..26).step_by(8) {
            b.fill([a, GROUND - 1, z], [c, GROUND, z + 1], PAVING_SEAM);
        }
    }
    for (a, c) in [(-34, -32), (32, 34)] {
        b.solid([a, ROAD, -26], [c, GROUND, 26], CURB);
    }
    for (a, c) in [(-56, -52), (52, 56)] {
        b.solid([a, ROAD, -28], [c, GROUND + 6, 28], STONE);
        b.fill([a, GROUND + 6, -29], [c, GROUND + 7, 29], STONE_LIGHT);
        // Arch face over the water and quoins.
        let face = if a < 0 { a } else { c - 1 };
        b.fill([face, -2, -14], [face + 1, ROAD + 1, 14], STONE_DARK);
        for z in (-26..28).step_by(6) {
            b.fill(
                [face, ROAD + 1, z],
                [face + 1, GROUND + 6, z + 3],
                STONE_LIGHT,
            );
        }
        b.fill([a, -6, -28], [c, ROAD, -16], STONE);
        b.fill([a, -6, 16], [c, ROAD, 28], STONE);
    }
    for z in [-28, -20, -12, -4, 4, 12, 20] {
        b.fill([-1, ROAD - 1, z], [1, ROAD, z + 4], LINE_YELLOW);
    }
    b.fill([-1, ROAD - 1, -60], [1, ROAD, -52], LINE_YELLOW);
    b.fill([-1, ROAD - 1, -44], [1, ROAD, -36], LINE_YELLOW);
    b.fill([-1, ROAD - 1, 36], [1, ROAD, 44], LINE_YELLOW);
    b.fill([-1, ROAD - 1, 52], [1, ROAD, 60], LINE_YELLOW);
    // Lamps on the parapets' ends.
    for (n, (x, z, t)) in [(-44, -40, 1u8), (44, 40, 3)].into_iter().enumerate() {
        street_socket(&mut b, &format!("kerb-{n}"), [x, GROUND, z], t);
    }
    stream_dressing(
        &mut b,
        &|x, z| {
            if x.abs() < 58 { 0. } else { dist(x, z) }
        },
        &[[-58, -HALF, 58, HALF]],
    );
    b.edges(["road", "stream", "road", "stream"]);
    face_specks(&mut b);
    b.finish()
}

// ---------------------------------------------------------------------------
// Lots

/// Garden lots face +Z: the building socket sits back from the frontage.
fn house_lot() -> Tile {
    let mut b = B::new("House lot", "lot");
    let all = [-HALF, -HALF, HALF, HALF];
    bedrock(&mut b, all);
    let path = |x: i32, z: i32| x.abs() < 8 && z >= 40;
    heightfield(&mut b, all, -8, false, |x, z| {
        if path(x, z) {
            col(GROUND, Surf::Paving)
        } else {
            col(GROUND, Surf::Grass)
        }
    });
    mown(&mut b, &|x, z| {
        !path(x, z) && !(x.abs() < 40 && (-28..44).contains(&z))
    });
    // Stepping-stone pavers in a warm tone.
    for z in (41..64).step_by(6).filter(|z| z + 5 <= 64) {
        b.fill([-7, GROUND - 1, z], [7, GROUND, z + 5], [0.80, 0.74, 0.64]);
    }
    b.socket(
        "building",
        "lot-house",
        [0, GROUND, 8],
        up(),
        "house-72",
        true,
    );
    // Front fence line with a gate gap for the path.
    for (n, x) in [-48, -32, -16, 16, 32, 48].into_iter().enumerate() {
        b.socket(
            &format!("fence-{n}"),
            "fence",
            [x, GROUND, 61],
            up(),
            "fence-16",
            false,
        );
    }
    let s = [
        [-26, GROUND, 52],
        [26, GROUND, 52],
        [-52, GROUND, 24],
        [52, GROUND, 24],
        [-52, GROUND, -10],
        [52, GROUND, -10],
        [-42, GROUND, -46],
        [0, GROUND, -48],
        [42, GROUND, -46],
    ];
    let mut keep = plant_sockets(&mut b, "plant", &s, 12);
    keep.push([-40, -28, 40, 64]);
    keep.push([-HALF, 58, HALF, HALF]);
    let lawn_ok = |x: i32, z: i32| !path(x, z);
    // Mailbox by the gate.
    b.solid([10, GROUND, 56], [12, GROUND + 9, 58], WOOD_DARK);
    b.fill(
        [9, GROUND + 9, 54],
        [13, GROUND + 13, 60],
        [0.30, 0.45, 0.75],
    );
    b.fill(
        [13, GROUND + 11, 57],
        [14, GROUND + 15, 58],
        [0.88, 0.30, 0.26],
    );
    lawn(&mut b, all, GROUND, &keep, 3, &lawn_ok);
    b.edges(["ground", "ground", "frontage", "ground"]);
    face_specks(&mut b);
    b.finish()
}

fn shop_lot() -> Tile {
    let mut b = B::new("Shop lot", "lot");
    let all = [-HALF, -HALF, HALF, HALF];
    bedrock(&mut b, all);
    heightfield(&mut b, all, -8, false, |x, z| {
        if z >= 52 || (z < -28 && x.abs() < 52) {
            col(GROUND, Surf::Paving)
        } else {
            col(GROUND, Surf::Grass)
        }
    });
    paving(&mut b, [-HALF, 52, HALF, HALF]);
    mown(&mut b, &|x, z| {
        z < 52 && !(z < -28 && x.abs() < 52) && x.abs() > 48
    });
    // Service yard at the back.
    b.fill(
        [-52, GROUND - 1, -HALF],
        [52, GROUND, -28],
        [0.66, 0.65, 0.62],
    );
    for x in (-48..52).step_by(12) {
        b.fill([x, GROUND - 1, -60], [x + 1, GROUND, -32], LINE_WHITE);
    }
    b.socket(
        "building",
        "lot-block",
        [0, GROUND, 12],
        up(),
        "block-96",
        true,
    );
    for (n, (x, z, t)) in [(-56, 58, 2u8), (56, 58, 2), (-40, -58, 0), (40, -58, 0)]
        .into_iter()
        .enumerate()
    {
        street_socket(&mut b, &format!("forecourt-{n}"), [x, GROUND, z], t);
    }
    let s = [
        [-57, GROUND, 40],
        [57, GROUND, 40],
        [-57, GROUND, 6],
        [57, GROUND, 6],
        [-57, GROUND, -22],
        [57, GROUND, -22],
    ];
    let mut keep = plant_sockets(&mut b, "plant", &s, 7);
    keep.push([-50, -HALF, 50, HALF]);
    keep.push([-HALF, 50, HALF, HALF]);
    keep.push([-HALF, -HALF, HALF, -26]);
    lawn(&mut b, all, GROUND, &keep, 2, &|x, z| {
        z < 52 && !(z < -28 && x.abs() < 52)
    });
    b.edges(["ground", "ground", "frontage", "ground"]);
    face_specks(&mut b);
    b.finish()
}

// ---------------------------------------------------------------------------
// Hill

const FACE: i32 = 52;

/// Plateau body over `area` (lower ground strip handled separately).
fn hill_body(b: &mut B, area: [i32; 4], f: impl Fn(i32, i32) -> Col) {
    bedrock(b, area);
    heightfield(b, area, -8, true, f);
}

/// Rock detail for a cliff face of canonical side 2 (face plane z = FACE)
/// spanning `x0..x1`, turned `k` quarter steps.
fn cliff_face(b: &mut B, k: u8, x0: i32, x1: i32, skip: &[[i32; 2]]) {
    let blocked = |x: i32, w: i32| skip.iter().any(|s| x + w > s[0] && x < s[1]);
    // Protruding ledges and boulders, coloured by stratum.
    let mut x = x0;
    while x < x1 - 2 {
        let w = b.rng.range(4, 13).min(x1 - x);
        if !blocked(x, w) {
            let mut y = 2;
            while y < HILL - 3 {
                let h = b.rng.range(3, 9).min(HILL - 2 - y);
                let depth = b.rng.range(0, 4);
                if depth > 0 {
                    let c = shade(stratum(y + h / 2), 0.92 + b.rng.unit() * 0.16);
                    b.fill_k(k, [x, y, FACE], [x + w, y + h, FACE + depth], c);
                    if y == 2 {
                        b.collide_k(k, [x, y, FACE], [x + w, y + h, FACE + depth]);
                    }
                }
                y += h;
            }
        }
        x += w;
    }
    // Cracks and moss on the face.
    for _ in 0..(x1 - x0) / 3 {
        let x = b.rng.range(x0, x1 - 1);
        let y = b.rng.range(3, HILL - 4);
        let w = b.rng.range(1, 4);
        let h = b.rng.range(1, 5);
        let c = if b.rng.chance(25) {
            MOSS
        } else {
            shade(stratum(y), 0.8)
        };
        if !blocked(x, w) {
            b.fill_k(k, [x, y, FACE - 1], [(x + w).min(x1), y + h, FACE], c);
        }
    }
    // Grass lip and hanging vines.
    let mut x = x0;
    while x < x1 {
        let w = b.rng.range(3, 10).min(x1 - x);
        if !blocked(x, w) {
            let over = b.rng.range(0, 3);
            if over > 0 {
                b.fill_k(
                    k,
                    [x, HILL - 2, FACE],
                    [x + w, HILL, FACE + over],
                    GRASS_DARK,
                );
                if b.rng.chance(45) {
                    let vx = x + b.rng.range(0, w);
                    let len = b.rng.range(3, 10);
                    b.fill_k(
                        k,
                        [vx, HILL - 2 - len, FACE],
                        [vx + 1, HILL - 2, FACE + 1],
                        MOSS,
                    );
                }
            }
        }
        x += w;
    }
    // Rubble at the foot.
    for _ in 0..(x1 - x0) / 10 {
        let x = b.rng.range(x0, x1 - 6);
        if blocked(x, 6) {
            continue;
        }
        let s = b.rng.range(2, 6);
        let z = FACE + b.rng.range(3, 8);
        if z + s > HALF - 1 {
            continue;
        }
        let c = *b.rng.pick(&[STONE, STONE_LIGHT, STONE_DARK]);
        b.fill_k(k, [x, GROUND, z], [x + s, GROUND + s / 2 + 1, z + s], c);
    }
}

/// Lower ground strip in front of a canonical side-2 face.
fn foot_strip(b: &mut B, k: u8, x0: i32, x1: i32, surf: impl Fn(i32, i32) -> Col) {
    let (lo, hi) = turn_box(k, [x0, 0, FACE], [x1, 0, HALF]);
    let area = [lo[0], lo[2], hi[0], hi[2]];
    bedrock(b, area);
    heightfield(b, area, -8, false, |x, z| {
        // Back to canonical coordinates.
        let p = turn_point((4 - k) % 4, [x, 0, z]);
        surf(p[0], p[2])
    });
    for _ in 0..(x1 - x0) / 8 {
        let x = b.rng.range(x0, x1 - 6);
        let z = b.rng.range(FACE + 4, HALF - 3);
        let c = *b.rng.pick(&[GRASS_DARK, GRASS_LIGHT, MOSS]);
        b.fill_k(k, [x, GROUND - 1, z], [x + 5, GROUND, z + 3], c);
    }
}

fn hill_top() -> Tile {
    let mut b = B::new("Hill top", "hill");
    hill_body(&mut b, [-HALF, -HALF, HALF, HALF], |_, _| {
        col(HILL, Surf::Grass)
    });
    let s = spots(&mut b, [-HALF, -HALF, HALF, HALF], (3, 3), 9, HILL, &[]);
    let keep = plant_sockets(&mut b, "plant", &s, 14);
    lawn(&mut b, [-HALF, -HALF, HALF, HALF], HILL, &keep, 3, ANY);
    b.edges(["high"; 4]);
    face_specks(&mut b);
    b.finish()
}

/// A plateau with a cliff on side 2; `feature` cuts stairs, a ramp or a waterfall.
#[derive(Clone, Copy, PartialEq)]
enum CliffFeature {
    None,
    Stairs,
    Ramp,
    Waterfall,
}

fn cliff(name: &str, feature: CliffFeature) -> Tile {
    let mut b = B::new(name, "hill");
    let stair_x = 12;
    let ramp_x = 22;
    let brook = |x: i32| (x as f32 + 0.5).abs();
    // Plateau height field with cut-outs.
    hill_body(&mut b, [-HALF, -HALF, HALF, FACE], |x, z| match feature {
        CliffFeature::Stairs if x.abs() < stair_x && z >= 4 => col(GROUND, Surf::Dirt),
        CliffFeature::Ramp if x.abs() < ramp_x && z >= 4 => {
            let step = ((z - 4) / 3).min(15);
            col(HILL - 2 * (step + 1), Surf::Grass)
        }
        CliffFeature::Waterfall => {
            if z >= 44 && brook(x) < 15. {
                col(WATER_Y, Surf::Water)
            } else {
                brook_col(brook(x))
            }
        }
        _ => col(HILL, Surf::Grass),
    });
    let foot = move |x: i32, z: i32| -> Col {
        let _ = z;
        match feature {
            CliffFeature::Stairs if x.abs() < 9 => col(GROUND, Surf::Gravel),
            CliffFeature::Waterfall => stream_col((x as f32 + 0.5).abs()),
            _ => col(GROUND, Surf::Grass),
        }
    };
    foot_strip(&mut b, 0, -HALF, HALF, foot);
    let mut skip = vec![];
    match feature {
        CliffFeature::Stairs => {
            skip.push([-stair_x - 1, stair_x + 1]);
            // Sixteen stone steps of two voxels with lighter nosings.
            for s in 0..16 {
                let z0 = 4 + 3 * s;
                let top = HILL - 2 * (s + 1);
                b.solid([-stair_x, GROUND, z0], [stair_x, top, z0 + 3], STONE);
                b.fill([-stair_x, top - 1, z0], [stair_x, top, z0 + 1], STONE_LIGHT);
            }
            // Landing at the top and wooden railings.
            b.fill([-stair_x, HILL - 1, -8], [stair_x, HILL, 4], GRAVEL);
            for x in [-stair_x - 2, stair_x] {
                for z in (4..52).step_by(8) {
                    b.solid([x, HILL, z], [x + 2, HILL + 9, z + 2], WOOD_DARK);
                }
                b.fill([x, HILL + 7, 4], [x + 2, HILL + 9, FACE], WOOD);
                b.collide([x, HILL, 4], [x + 2, HILL + 9, FACE]);
            }
        }
        CliffFeature::Ramp => {
            skip.push([-ramp_x - 1, ramp_x + 1]);
            for s in 0..16 {
                let z0 = 4 + 3 * s;
                let top = HILL - 2 * (s + 1);
                // Dirt risers under grass treads.
                b.fill([-ramp_x, top, z0 - 1], [ramp_x, top + 2, z0], DIRT);
                if s % 3 == 1 {
                    b.fill([-ramp_x, top - 1, z0], [ramp_x, top, z0 + 3], GRASS_LIGHT);
                }
            }
            // Boulders along both edges of the ramp.
            for x in [-ramp_x, ramp_x - 4] {
                for s in (0..16).step_by(3) {
                    let z0 = 4 + 3 * s;
                    let top = HILL - 2 * (s + 1);
                    let c = *b.rng.pick(&[STONE, STONE_LIGHT, STONE_DARK]);
                    b.fill([x, top, z0], [x + 4, top + 3, z0 + 3], c);
                }
            }
        }
        CliffFeature::Waterfall => {
            skip.push([-17, 17]);
            // The falling sheet with foam streaks, and spray at the pool.
            b.fill([-12, WATER_Y, 44], [12, HILL - 4, 48], WATER_LIGHT);
            for x in (-12..12).step_by(3) {
                let len = b.rng.range(6, 20);
                let top = HILL - 4 - b.rng.range(0, 6);
                b.fill([x, top - len, 47], [x + 1, top, 48], FOAM);
                let splash = b.rng.range(2, 8);
                b.fill([x + 1, WATER_Y, 47], [x + 2, WATER_Y + splash, 48], FOAM);
            }
            b.fill([-14, WATER_Y - 1, 48], [14, WATER_Y, 54], FOAM);
            b.patches(
                [-14, 52],
                [14, 63],
                WATER_Y,
                &[FOAM, WATER_LIGHT],
                14,
                (1, 4),
            );
            // Wet dark rock beside the falls.
            for x in [-17, 12] {
                b.fill(
                    [x, WATER_Y, 43],
                    [x + 5, HILL - 2, 52],
                    shade(STONE_DARK, 0.85),
                );
                b.fill([x + 1, HILL - 8, 48], [x + 4, HILL - 2, 52], MOSS);
            }
            brook_dressing(&mut b, -HALF, 40);
        }
        CliffFeature::None => {}
    }
    cliff_face(&mut b, 0, -HALF, HALF, &skip);
    let blocked: Vec<[i32; 4]> = match feature {
        CliffFeature::Stairs => vec![[-16, -12, 16, FACE]],
        CliffFeature::Ramp => vec![[-26, 0, 26, FACE]],
        CliffFeature::Waterfall => vec![[-18, -HALF, 18, FACE]],
        CliffFeature::None => vec![],
    };
    let s = spots(
        &mut b,
        [-HALF, -HALF, HALF, FACE - 8],
        (3, 3),
        8,
        HILL,
        &blocked,
    );
    let mut keep = plant_sockets(&mut b, "plant", &s, 14);
    keep.extend(blocked.iter().map(|r| [r[0] - 2, r[1], r[2] + 2, r[3]]));
    keep.push([-HALF, FACE - 4, HALF, HALF]);
    let cut = blocked.clone();
    lawn(
        &mut b,
        [-HALF, -HALF, HALF, FACE],
        HILL,
        &keep,
        3,
        &|x, z| {
            !cut.iter()
                .any(|r| x >= r[0] - 2 && x < r[2] + 2 && z >= r[1] - 2 && z < r[3] + 2)
        },
    );
    b.edges([
        if feature == CliffFeature::Waterfall {
            "brook"
        } else {
            "high"
        },
        "cliff-r",
        match feature {
            CliffFeature::Stairs | CliffFeature::Ramp => "path",
            CliffFeature::Waterfall => "stream",
            _ => "ground",
        },
        "cliff-l",
    ]);
    face_specks(&mut b);
    b.finish()
}

fn brook_col(d: f32) -> Col {
    if d < 10. {
        col(HILL - 4, Surf::Water)
    } else if d < 12. {
        col(HILL - 4, Surf::Pebble)
    } else if d < 15. {
        col(HILL - 2, Surf::Pebble)
    } else {
        col(HILL, Surf::Grass)
    }
}
fn brook_dressing(b: &mut B, z0: i32, z1: i32) {
    for _ in 0..(z1 - z0) / 2 {
        let x = b.rng.range(-9, 7);
        let z = b.rng.range(z0, z1 - 4);
        let c = *b.rng.pick(&[WATER_LIGHT, WATER_LIGHT, WATER_DEEP]);
        let len = b.rng.range(2, 5);
        b.fill([x, HILL - 5, z], [x + 1, HILL - 4, z + len], c);
    }
    for _ in 0..(z1 - z0) / 4 {
        let side = if b.rng.chance(50) { -1 } else { 1 };
        let x = side * b.rng.range(11, 15) - if side < 0 { 3 } else { 0 };
        let z = b.rng.range(z0, z1 - 4);
        let top = brook_col((x as f32 + 1.5).abs()).top;
        if b.rng.chance(50) {
            let c = *b.rng.pick(&[STONE, STONE_LIGHT]);
            b.fill([x, top, z], [x + 3, top + 2, z + 3], c);
        } else {
            let h = b.rng.range(3, 7);
            b.fill([x + 1, top, z], [x + 2, top + h, z + 1], MOSS);
        }
    }
}

fn hill_brook() -> Tile {
    let mut b = B::new("Hill brook", "hill");
    hill_body(&mut b, [-HALF, -HALF, HALF, HALF], |x, _| {
        brook_col((x as f32 + 0.5).abs())
    });
    brook_dressing(&mut b, -HALF, HALF);
    let s = [
        [-42, HILL, -40],
        [42, HILL, -40],
        [-42, HILL, 40],
        [42, HILL, 40],
        [-44, HILL, 0],
        [44, HILL, 0],
    ];
    let mut keep = plant_sockets(&mut b, "plant", &s, 14);
    keep.push([-18, -HALF, 18, HALF]);
    lawn(
        &mut b,
        [-HALF, -HALF, HALF, HALF],
        HILL,
        &keep,
        3,
        &|x, _| x.abs() > 16,
    );
    b.edges(["brook", "high", "brook", "high"]);
    face_specks(&mut b);
    b.finish()
}

fn hill_spring() -> Tile {
    let mut b = B::new("Hill spring", "hill");
    let pool = |x: i32, z: i32| {
        let (x, z) = (x as f32 + 0.5, z as f32 + 0.5);
        let r = x.hypot(z + 6.);
        let channel = if z > 0. { x.abs() } else { 99. };
        (r - 18.).min(channel - 10.)
    };
    hill_body(&mut b, [-HALF, -HALF, HALF, HALF], |x, z| {
        let d = pool(x, z);
        brook_col(d + 10.)
    });
    brook_dressing(&mut b, 8, HALF);
    // Mossy boulders ring the spring head.
    for i in 0..10 {
        let a = std::f32::consts::PI * (1.15 + i as f32 * 0.075);
        let x = (a.cos() * 30.) as i32;
        let z = (a.sin() * 30.) as i32 - 6;
        let s = b.rng.range(4, 9);
        let c = *b.rng.pick(&[STONE, STONE_LIGHT, STONE_DARK]);
        b.fill([x, HILL, z], [x + s, HILL + s / 2 + 2, z + s], c);
        b.fill(
            [x, HILL + s / 2 + 2, z],
            [x + s - 1, HILL + s / 2 + 3, z + s - 1],
            MOSS,
        );
    }
    b.patches(
        [-14, -22],
        [14, 8],
        HILL - 4,
        &[WATER_LIGHT, WATER_DEEP],
        14,
        (1, 3),
    );
    let s = [
        [-46, HILL, -46],
        [46, HILL, -46],
        [-46, HILL, 30],
        [46, HILL, 30],
        [0, HILL, -52],
    ];
    let mut keep = plant_sockets(&mut b, "plant", &s, 14);
    keep.push([-34, -40, 34, HALF]);
    lawn(
        &mut b,
        [-HALF, -HALF, HALF, HALF],
        HILL,
        &keep,
        3,
        &|x, z| pool(x, z) > 6.,
    );
    b.edges(["high", "high", "brook", "high"]);
    face_specks(&mut b);
    b.finish()
}

fn cliff_corner() -> Tile {
    let mut b = B::new("Cliff corner", "hill");
    // Plateau x < FACE and z < FACE with a chamfered corner.
    let chamfer = |x: i32, z: i32| x + z > 2 * FACE - 22;
    hill_body(&mut b, [-HALF, -HALF, FACE, FACE], |x, z| {
        if chamfer(x, z) {
            col(GROUND, Surf::Grass)
        } else {
            col(HILL, Surf::Grass)
        }
    });
    foot_strip(&mut b, 0, -HALF, HALF, |_, _| col(GROUND, Surf::Grass));
    // The +X strip (canonical side 1) excluding the shared corner square.
    let area = [FACE, -HALF, HALF, FACE];
    bedrock(&mut b, area);
    heightfield(&mut b, area, -8, false, |_, _| col(GROUND, Surf::Grass));
    cliff_face(&mut b, 0, -HALF, FACE - 22, &[]);
    cliff_face(&mut b, 1, -FACE + 22, HALF, &[]);
    // Chamfer face: a staircase of chunky rock blocks.
    for i in (0..22).step_by(3) {
        let x = FACE - 22 + i;
        let z = FACE - i - 3;
        let c = shade(STONE, 0.9 + b.rng.unit() * 0.2);
        let h = HILL - b.rng.range(0, 4);
        b.fill([x, GROUND, z], [x + 3, h, z + 3], c);
        b.fill([x, GROUND, z], [x + 3, 9, z + 3], STONE_WARM);
        b.fill([x, GROUND, z], [x + 3, 3, z + 3], STONE_DARK);
        b.collide([x, GROUND, z], [x + 3, HILL, z + 3]);
        if i % 6 == 0 {
            let h = b.rng.range(2, 5);
            b.fill(
                [x + 2, GROUND, z + 3],
                [x + 5, GROUND + h, z + 5],
                STONE_LIGHT,
            );
        }
    }
    let s = spots(
        &mut b,
        [-HALF, -HALF, FACE - 10, FACE - 10],
        (3, 3),
        6,
        HILL,
        &[],
    );
    let mut keep = plant_sockets(&mut b, "plant", &s, 14);
    keep.push([FACE - 30, -HALF, HALF, HALF]);
    keep.push([-HALF, FACE - 30, HALF, HALF]);
    lawn(
        &mut b,
        [-HALF, -HALF, FACE, FACE],
        HILL,
        &keep,
        3,
        &|x, z| !chamfer(x, z) && x + z < 2 * FACE - 26,
    );
    b.edges(["cliff-r", "ground", "ground", "cliff-l"]);
    face_specks(&mut b);
    b.finish()
}

pub fn tiles() -> Vec<Tile> {
    let mut t = vec![
        meadow(),
        flower_meadow(),
        pond(),
        plaza(),
        community_garden(),
        playground(),
        path_tile("Path straight", [true, false, true, false]),
        path_tile("Path corner", [true, true, false, false]),
        path_tile("Path T", [true, true, true, false]),
        path_tile("Path end", [false, false, true, false]),
    ];
    t.extend(roads());
    t.push(road_bridge());
    t.push(stream_tile("Stream straight", false, false));
    t.push(stream_tile("Stream bend", true, false));
    t.push(stream_tile("Stream crossing", false, true));
    t.push(house_lot());
    t.push(shop_lot());
    t.push(hill_top());
    t.push(cliff("Cliff", CliffFeature::None));
    t.push(cliff_corner());
    t.push(cliff("Cliff stairs", CliffFeature::Stairs));
    t.push(cliff("Cliff ramp", CliffFeature::Ramp));
    t.push(cliff("Waterfall", CliffFeature::Waterfall));
    t.push(hill_brook());
    t.push(hill_spring());
    t
}
