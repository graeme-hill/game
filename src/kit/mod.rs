//! The authored "Neighbourhood" voxel kit: terrain cells, buildings, nature and
//! street furniture. Every tile is ordinary editable source data; this module
//! only describes how the starter assets are drawn.
//!
//! Conventions (voxel units, 1 voxel = 0.1 world units):
//! - Terrain cells are 128×128 centred on their origin. Lower ground is at
//!   y = 2 (roads at y = 0, streams at y = -4) and the hill top is at y = 34.
//!   Cell edges carry `edge-N` sockets (N: 0 = -Z, 1 = +X, 2 = +Z, 3 = -X)
//!   whose kind describes what crosses the edge.
//! - Decorations sit on upward sockets (`plant`, `street`, `fence`, lots) and
//!   start at their own y = 0.
use crate::tiles::{Bounds, Socket, SocketRules, Tile, Volume, yaw};
use bevy::prelude::*;

mod buildings;
mod nature;
pub use buildings::HOUSE_COLOURS;
mod street;
mod terrain;

pub type Rgb = [f32; 3];

/// Cell geometry shared by the generator.
pub const CELL: i32 = 128;
pub const HALF: i32 = 64;
pub const GROUND: i32 = 2;
pub const ROAD: i32 = 0;
pub const WATER_Y: i32 = -4;
pub const HILL: i32 = 34;
pub const BASE: i32 = -16;
pub const EDGE_Y: i32 = -8;

pub fn up() -> Quat {
    Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2)
}
pub fn down() -> Quat {
    up() * Quat::from_rotation_y(std::f32::consts::PI)
}
/// An upward socket turned `turns` quarter steps about the vertical axis.
pub fn up_turned(turns: u8) -> Quat {
    Quat::from_array(yaw(turns)) * up()
}
/// The outward frame of cell edge `n`.
pub fn edge_rotation(n: usize) -> Quat {
    Quat::from_array(yaw(((6 - n) % 4) as u8))
}
pub fn edge_position(n: usize) -> [i32; 3] {
    [
        [0, EDGE_Y, -HALF],
        [HALF, EDGE_Y, 0],
        [0, EDGE_Y, HALF],
        [-HALF, EDGE_Y, 0],
    ][n % 4]
}

/// Rotate an integer box by `k` quarter turns about the vertical axis
/// (the same direction as [`yaw`]).
pub fn turn_box(k: u8, min: [i32; 3], max: [i32; 3]) -> ([i32; 3], [i32; 3]) {
    let p = |x: i32, z: i32| match k % 4 {
        0 => (x, z),
        1 => (z, -x),
        2 => (-x, -z),
        _ => (-z, x),
    };
    let (ax, az) = p(min[0], min[2]);
    let (bx, bz) = p(max[0], max[2]);
    (
        [ax.min(bx), min[1], az.min(bz)],
        [ax.max(bx), max[1], az.max(bz)],
    )
}
pub fn turn_point(k: u8, p: [i32; 3]) -> [i32; 3] {
    let (a, _) = turn_box(k, p, p);
    a
}

/// Deterministic SplitMix64 so authored details never change between runs.
#[derive(Clone)]
pub struct Rng(u64);
impl Rng {
    pub fn new(seed: u64) -> Self {
        Self(seed)
    }
    pub fn named(name: &str) -> Self {
        Self(name.bytes().fold(0xcbf2_9ce4_8422_2325, |h, b| {
            (h ^ u64::from(b)).wrapping_mul(0x100_0000_01b3)
        }))
    }
    pub fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }
    /// Uniform in `lo..hi` (exclusive); returns `lo` for empty ranges.
    pub fn range(&mut self, lo: i32, hi: i32) -> i32 {
        if hi <= lo {
            lo
        } else {
            lo + (self.next_u64() % (hi - lo) as u64) as i32
        }
    }
    pub fn chance(&mut self, percent: u64) -> bool {
        self.next_u64() % 100 < percent
    }
    pub fn unit(&mut self) -> f32 {
        (self.next_u64() >> 40) as f32 / (1u64 << 24) as f32
    }
    pub fn pick<'a, T>(&mut self, items: &'a [T]) -> &'a T {
        &items[(self.next_u64() % items.len() as u64) as usize]
    }
    pub fn shuffle<T>(&mut self, items: &mut [T]) {
        for i in (1..items.len()).rev() {
            let j = (self.next_u64() % (i as u64 + 1)) as usize;
            items.swap(i, j);
        }
    }
}

pub fn shade(c: Rgb, f: f32) -> Rgb {
    c.map(|v| (v * f).clamp(0., 1.))
}
pub fn mix(a: Rgb, b: Rgb, t: f32) -> Rgb {
    std::array::from_fn(|i| (a[i] + (b[i] - a[i]) * t).clamp(0., 1.))
}

/// Shared palette. Colours are sRGB and slightly saturated for a toy-like look.
pub mod pal {
    use super::Rgb;
    pub const GRASS: Rgb = [0.47, 0.70, 0.30];
    pub const GRASS_DARK: Rgb = [0.39, 0.62, 0.26];
    pub const GRASS_LIGHT: Rgb = [0.56, 0.77, 0.34];
    pub const GRASS_DRY: Rgb = [0.63, 0.74, 0.35];
    pub const MOSS: Rgb = [0.40, 0.55, 0.27];
    pub const DIRT: Rgb = [0.55, 0.39, 0.25];
    pub const DIRT_DARK: Rgb = [0.43, 0.30, 0.20];
    pub const BEDROCK: Rgb = [0.42, 0.39, 0.38];
    pub const SOIL: Rgb = [0.36, 0.25, 0.17];
    pub const ASPHALT: Rgb = [0.27, 0.28, 0.31];
    pub const ASPHALT_DARK: Rgb = [0.23, 0.24, 0.27];
    pub const ASPHALT_LIGHT: Rgb = [0.31, 0.32, 0.35];
    pub const PAVING: Rgb = [0.76, 0.75, 0.72];
    pub const PAVING_SEAM: Rgb = [0.66, 0.65, 0.62];
    pub const CURB: Rgb = [0.85, 0.84, 0.81];
    pub const LINE_YELLOW: Rgb = [0.96, 0.80, 0.29];
    pub const LINE_WHITE: Rgb = [0.95, 0.95, 0.93];
    pub const WATER: Rgb = [0.27, 0.60, 0.82];
    pub const WATER_DEEP: Rgb = [0.21, 0.50, 0.74];
    pub const WATER_LIGHT: Rgb = [0.52, 0.80, 0.93];
    pub const FOAM: Rgb = [0.90, 0.96, 0.99];
    pub const SAND: Rgb = [0.88, 0.81, 0.62];
    pub const PEBBLE: Rgb = [0.67, 0.65, 0.61];
    pub const STONE: Rgb = [0.60, 0.58, 0.56];
    pub const STONE_DARK: Rgb = [0.48, 0.46, 0.45];
    pub const STONE_LIGHT: Rgb = [0.71, 0.69, 0.66];
    pub const STONE_WARM: Rgb = [0.66, 0.59, 0.51];
    pub const GRAVEL: Rgb = [0.80, 0.74, 0.61];
    pub const GRAVEL_DARK: Rgb = [0.70, 0.64, 0.52];
    pub const WOOD: Rgb = [0.58, 0.38, 0.23];
    pub const WOOD_DARK: Rgb = [0.43, 0.28, 0.17];
    pub const WOOD_LIGHT: Rgb = [0.71, 0.52, 0.33];
    pub const BARK: Rgb = [0.45, 0.30, 0.20];
    pub const WHITE: Rgb = [0.95, 0.94, 0.91];
    pub const IRON: Rgb = [0.17, 0.18, 0.20];
    pub const GLASS: Rgb = [0.39, 0.58, 0.74];
    pub const GLASS_DARK: Rgb = [0.25, 0.37, 0.52];
    pub const GLASS_LIGHT: Rgb = [0.67, 0.84, 0.93];
    pub const BRICK: Rgb = [0.70, 0.33, 0.24];
    pub const ROOF_RED: Rgb = [0.74, 0.30, 0.22];
    pub const ROOF_SLATE: Rgb = [0.36, 0.40, 0.48];
    pub const ROOF_GREEN: Rgb = [0.33, 0.52, 0.42];
    pub const ROOF_BROWN: Rgb = [0.52, 0.35, 0.25];
    pub const INTERIOR: Rgb = [0.24, 0.20, 0.18];
    pub const FLOOR_WOOD: Rgb = [0.66, 0.48, 0.32];
}

/// A tile under construction.
pub struct B {
    pub t: Tile,
    pub rng: Rng,
}
impl B {
    pub fn new(name: &str, category: &str) -> Self {
        Self {
            t: Tile {
                id: 0,
                name: name.into(),
                category: category.into(),
                voxels: vec![],
                sockets: vec![],
                rotations: (0..4).map(yaw).collect(),
                colliders: vec![],
            },
            rng: Rng::named(name),
        }
    }
    /// Visual voxels only. Later volumes repaint earlier cells.
    pub fn fill(&mut self, min: [i32; 3], max: [i32; 3], color: Rgb) {
        if (0..3).all(|a| min[a] < max[a]) {
            self.t.voxels.push(Volume { min, max, color });
        }
    }
    /// Voxels that also block movement.
    pub fn solid(&mut self, min: [i32; 3], max: [i32; 3], color: Rgb) {
        self.fill(min, max, color);
        self.collide(min, max);
    }
    pub fn collide(&mut self, min: [i32; 3], max: [i32; 3]) {
        if (0..3).all(|a| min[a] < max[a]) {
            self.t.colliders.push(Bounds {
                min: min.map(|v| v as f32 * crate::tiles::UNIT),
                max: max.map(|v| v as f32 * crate::tiles::UNIT),
            });
        }
    }
    /// A box rotated `k` quarter turns about the tile origin.
    pub fn fill_k(&mut self, k: u8, min: [i32; 3], max: [i32; 3], color: Rgb) {
        let (a, b) = turn_box(k, min, max);
        self.fill(a, b, color);
    }
    pub fn solid_k(&mut self, k: u8, min: [i32; 3], max: [i32; 3], color: Rgb) {
        let (a, b) = turn_box(k, min, max);
        self.solid(a, b, color);
    }
    pub fn collide_k(&mut self, k: u8, min: [i32; 3], max: [i32; 3]) {
        let (a, b) = turn_box(k, min, max);
        self.collide(a, b);
    }
    pub fn socket(
        &mut self,
        name: &str,
        kind: &str,
        position: [i32; 3],
        rotation: Quat,
        profile: &str,
        required: bool,
    ) {
        self.t.sockets.push(Socket {
            name: name.into(),
            kind: kind.into(),
            position,
            rotation: rotation.to_array(),
            profile: profile.into(),
            required,
        });
    }
    /// The four cell-edge sockets, kinds listed in local edge order.
    pub fn edges(&mut self, kinds: [&str; 4]) {
        for (n, kind) in kinds.into_iter().enumerate() {
            self.socket(
                &format!("edge-{n}"),
                kind,
                edge_position(n),
                edge_rotation(n),
                "cell-128",
                kind == "road" || kind == "lane",
            );
        }
    }
    /// A base socket at the tile origin, facing down onto its support.
    pub fn base(&mut self, kind: &str, profile: &str) {
        self.socket("base", kind, [0, 0, 0], down(), profile, true);
    }
    /// Recolour random small rectangles of the top layer of `[min, max)` in xz
    /// at height `y` (one voxel thick, so the occupied volume is unchanged).
    pub fn patches(
        &mut self,
        min: [i32; 2],
        max: [i32; 2],
        y: i32,
        colors: &[Rgb],
        count: usize,
        size: (i32, i32),
    ) {
        for _ in 0..count {
            let w = self.rng.range(size.0, size.1 + 1);
            let d = self.rng.range(size.0, size.1 + 1);
            let x = self.rng.range(min[0], (max[0] - w).max(min[0]) + 1);
            let z = self.rng.range(min[1], (max[1] - d).max(min[1]) + 1);
            let c = *self.rng.pick(colors);
            self.fill(
                [x, y - 1, z],
                [(x + w).min(max[0]), y, (z + d).min(max[1])],
                c,
            );
        }
    }
    pub fn finish(self) -> Tile {
        self.t
    }
}

/// Every starter tile, in a stable order.
pub fn tiles() -> Vec<Tile> {
    let mut all = vec![];
    all.extend(terrain::tiles());
    all.extend(buildings::tiles());
    all.extend(nature::tiles());
    all.extend(street::tiles());
    all
}

pub fn rules() -> SocketRules {
    let pairs = [
        // Cell edges.
        ("road", "road"),
        ("lane", "lane"),
        ("curb", "curb"),
        ("curb", "ground"),
        ("curb", "path"),
        ("curb", "frontage"),
        ("ground", "ground"),
        ("path", "path"),
        ("stream", "stream"),
        ("high", "high"),
        ("cliff-l", "cliff-r"),
        ("brook", "brook"),
        // Buildings stack on lots.
        ("lot-house", "house-base"),
        ("house-top", "house-storey"),
        ("house-top", "house-roof"),
        ("lot-block", "block-base"),
        ("block-top", "block-storey"),
        ("block-top", "block-roof"),
        // Decorations.
        ("plant", "plant-base"),
        ("plant", "street-base"),
        ("street", "street-base"),
        ("fence", "fence-base"),
        ("crossing", "bridge-base"),
    ];
    SocketRules {
        pairs: pairs
            .into_iter()
            .map(|(a, b)| [a.into(), b.into()])
            .collect(),
    }
}
