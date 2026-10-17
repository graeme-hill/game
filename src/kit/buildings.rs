//! Stackable buildings. Houses (72×64 footprint) and blocks (96×80) are built
//! from a ground floor, any number of storeys and a roof, joined by sockets.
//! Local +Z is the street front; y = 0 is the lot surface.
use super::*;
use pal::*;

/// A building footprint: walls' outer faces at ±hx and ±hz.
#[derive(Clone, Copy)]
struct Fp {
    hx: i32,
    hz: i32,
}
impl Fp {
    /// Half-length of the wall on side `k` (0 front +Z, 1 +X, 2 back, 3 -X).
    fn half(self, k: u8) -> i32 {
        if k.is_multiple_of(2) {
            self.hx
        } else {
            self.hz
        }
    }
    fn face(self, k: u8) -> i32 {
        if k.is_multiple_of(2) {
            self.hz
        } else {
            self.hx
        }
    }
}

/// Facade box on side `k`: `u` runs left→right seen from outside, `v` is
/// height, `d` is depth measured outward from the wall face.
fn face_box(b: &mut B, fp: Fp, k: u8, u: [i32; 2], v: [i32; 2], d: [i32; 2], c: Rgb) {
    let f = fp.face(k);
    b.fill_k(k, [u[0], v[0], f + d[0]], [u[1], v[1], f + d[1]], c);
}
fn face_solid(b: &mut B, fp: Fp, k: u8, u: [i32; 2], v: [i32; 2], d: [i32; 2], c: Rgb) {
    let f = fp.face(k);
    b.solid_k(k, [u[0], v[0], f + d[0]], [u[1], v[1], f + d[1]], c);
}

#[derive(Clone, Copy)]
struct Style {
    wall: Rgb,
    trim: Rgb,
    accent: Rgb,
    sill: Rgb,
}

/// A framed window with mullions, sill and optional shutters / flower box.
fn window(
    b: &mut B,
    fp: Fp,
    k: u8,
    u: [i32; 2],
    v: [i32; 2],
    s: Style,
    (shutters, flowers): (bool, bool),
) {
    // Frame proud of the wall, glass flush, cross mullion.
    face_box(
        b,
        fp,
        k,
        [u[0] - 1, u[1] + 1],
        [v[0] - 1, v[1] + 1],
        [0, 1],
        s.trim,
    );
    face_box(b, fp, k, u, v, [-1, 1], GLASS);
    let mid_u = (u[0] + u[1]) / 2;
    let mid_v = (v[0] + v[1]) / 2 + 1;
    face_box(b, fp, k, [mid_u, mid_u + 1], v, [0, 1], s.trim);
    face_box(b, fp, k, u, [mid_v, mid_v + 1], [0, 1], s.trim);
    // A reflected glint in the top-left pane and darker lower panes.
    face_box(
        b,
        fp,
        k,
        [u[0], u[0] + 2],
        [v[1] - 3, v[1]],
        [0, 1],
        GLASS_LIGHT,
    );
    face_box(
        b,
        fp,
        k,
        [u[0] + 2, u[0] + 3],
        [v[1] - 2, v[1]],
        [0, 1],
        GLASS_LIGHT,
    );
    face_box(
        b,
        fp,
        k,
        [mid_u + 1, u[1]],
        [v[0], mid_v],
        [0, 1],
        GLASS_DARK,
    );
    face_box(
        b,
        fp,
        k,
        [u[0] - 2, u[1] + 2],
        [v[0] - 2, v[0] - 1],
        [0, 2],
        s.sill,
    );
    if shutters {
        face_box(
            b,
            fp,
            k,
            [u[0] - 5, u[0] - 1],
            [v[0], v[1]],
            [0, 1],
            s.accent,
        );
        face_box(
            b,
            fp,
            k,
            [u[1] + 1, u[1] + 5],
            [v[0], v[1]],
            [0, 1],
            s.accent,
        );
        for dv in (v[0] + 1..v[1]).step_by(3) {
            face_box(
                b,
                fp,
                k,
                [u[0] - 5, u[0] - 1],
                [dv, dv + 1],
                [1, 2],
                shade(s.accent, 0.85),
            );
            face_box(
                b,
                fp,
                k,
                [u[1] + 1, u[1] + 5],
                [dv, dv + 1],
                [1, 2],
                shade(s.accent, 0.85),
            );
        }
    }
    if flowers {
        face_box(b, fp, k, [u[0], u[1]], [v[0] - 6, v[0] - 2], [0, 3], WOOD);
        face_box(
            b,
            fp,
            k,
            [u[0] + 1, u[1] - 1],
            [v[0] - 2, v[0] - 1],
            [1, 3],
            GRASS_DARK,
        );
        let mut x = u[0] + 1;
        let colors = [
            [0.95, 0.35, 0.40],
            [0.98, 0.85, 0.30],
            [0.96, 0.62, 0.80],
            WHITE,
        ];
        let mut n = b.rng.range(0, 4) as usize;
        while x < u[1] - 1 {
            face_box(
                b,
                fp,
                k,
                [x, x + 1],
                [v[0] - 1, v[0]],
                [1, 3],
                colors[n % 4],
            );
            n += 1;
            x += 2;
        }
    }
}

/// Hollow walls from `y0` to `y1` with an optional door opening on the front.
fn walls(b: &mut B, fp: Fp, y0: i32, y1: i32, s: Style, door: Option<[i32; 3]>) {
    let (hx, hz) = (fp.hx, fp.hz);
    let mut pieces = vec![
        ([-hx, y0, -hz], [hx, y1, -hz + 2]),
        ([-hx, y0, -hz + 2], [-hx + 2, y1, hz - 2]),
        ([hx - 2, y0, -hz + 2], [hx, y1, hz - 2]),
    ];
    match door {
        Some([u0, u1, top]) => {
            pieces.push(([-hx, y0, hz - 2], [u0, y1, hz]));
            pieces.push(([u1, y0, hz - 2], [hx, y1, hz]));
            pieces.push(([u0, top, hz - 2], [u1, y1, hz]));
        }
        None => pieces.push(([-hx, y0, hz - 2], [hx, y1, hz])),
    }
    for (lo, hi) in pieces {
        b.solid(lo, hi, s.wall);
    }
    // Lighter interior skin, seen through doorways.
    let inner = mix(s.wall, WHITE, 0.45);
    b.fill([-hx + 2, y0, -hz + 1], [hx - 2, y1, -hz + 2], inner);
    b.fill([-hx + 1, y0, -hz + 2], [-hx + 2, y1, hz - 2], inner);
    b.fill([hx - 2, y0, -hz + 2], [hx - 1, y1, hz - 2], inner);
    if let Some([u0, u1, _]) = door {
        b.fill([-hx + 2, y0, hz - 2], [u0, y1, hz - 1], inner);
        b.fill([u1, y0, hz - 2], [hx - 2, y1, hz - 1], inner);
    }
    // Corner boards.
    for (x, z) in [(-hx, -hz), (hx - 2, -hz), (-hx, hz - 2), (hx - 2, hz - 2)] {
        b.fill([x, y0, z], [x + 2, y1, z + 2], s.trim);
    }
}

/// A band around the top of a storey, one voxel proud.
fn band(b: &mut B, fp: Fp, y0: i32, y1: i32, c: Rgb) {
    for k in 0..4u8 {
        let h = fp.half(k);
        face_box(b, fp, k, [-h - 1, h + 1], [y0, y1], [-2, 1], c);
    }
}

fn front_door(b: &mut B, fp: Fp, s: Style, half: i32, top: i32, leaf: Rgb) {
    let k = 0;
    face_box(b, fp, k, [-half - 2, -half], [2, top + 2], [-2, 1], s.trim);
    face_box(b, fp, k, [half, half + 2], [2, top + 2], [-2, 1], s.trim);
    face_box(
        b,
        fp,
        k,
        [-half - 2, half + 2],
        [top, top + 2],
        [-2, 1],
        s.trim,
    );
    // The door stands open against the inside wall.
    b.fill(
        [half, 2, fp.hz - 2 - 2 * half + 2],
        [half + 1, top, fp.hz - 2],
        leaf,
    );
    b.fill(
        [half - 1, 12, fp.hz - 6],
        [half, 13, fp.hz - 5],
        LINE_YELLOW,
    );
    // Doorstep.
    face_solid(b, fp, k, [-half - 2, half + 2], [0, 1], [0, 4], STONE_LIGHT);
}

fn house_style(name: &str) -> (Style, &'static str) {
    match name {
        "cream" => (
            Style {
                wall: [0.95, 0.89, 0.72],
                trim: WHITE,
                accent: [0.36, 0.58, 0.45],
                sill: STONE_LIGHT,
            },
            "Cream",
        ),
        "blue" => (
            Style {
                wall: [0.62, 0.78, 0.90],
                trim: WHITE,
                accent: [0.20, 0.30, 0.52],
                sill: WHITE,
            },
            "Blue",
        ),
        "yellow" => (
            Style {
                wall: [0.98, 0.84, 0.45],
                trim: WHITE,
                accent: [0.62, 0.30, 0.22],
                sill: STONE_LIGHT,
            },
            "Yellow",
        ),
        _ => (
            Style {
                wall: [0.95, 0.68, 0.64],
                trim: WHITE,
                accent: [0.38, 0.44, 0.60],
                sill: WHITE,
            },
            "Rose",
        ),
    }
}
pub const HOUSE_COLOURS: [&str; 4] = ["cream", "blue", "yellow", "rose"];
const HOUSE: Fp = Fp { hx: 36, hz: 32 };
const HOUSE_FLOOR: i32 = 32;

fn house_ground(colour: &str) -> Tile {
    let (s, label) = house_style(colour);
    let mut b = B::new(
        &format!("House ground floor ({})", label.to_lowercase()),
        "building",
    );
    let fp = HOUSE;
    b.solid(
        [-fp.hx - 1, 0, -fp.hz - 1],
        [fp.hx + 1, 2, fp.hz + 1],
        STONE,
    );
    b.fill(
        [-fp.hx + 2, 1, -fp.hz + 2],
        [fp.hx - 2, 2, fp.hz - 2],
        FLOOR_WOOD,
    );
    for z in (-fp.hz + 4..fp.hz - 2).step_by(4) {
        b.fill(
            [-fp.hx + 2, 1, z],
            [fp.hx - 2, 2, z + 1],
            shade(FLOOR_WOOD, 0.88),
        );
    }
    walls(&mut b, fp, 2, 30, s, Some([-7, 7, 26]));
    band(&mut b, fp, 30, 32, s.trim);
    front_door(&mut b, fp, s, 7, 26, s.accent);
    // Rug and a sideboard inside the door.
    b.fill([-8, 2, 6], [8, 3, 22], [0.70, 0.30, 0.28]);
    b.solid([-30, 2, -28], [-14, 10, -22], WOOD);
    for (u, flowers) in [(-28, true), (14, true)] {
        window(&mut b, fp, 0, [u, u + 14], [10, 24], s, (true, flowers));
    }
    for k in [1u8, 3] {
        window(&mut b, fp, k, [-20, -8], [10, 24], s, (false, false));
        window(&mut b, fp, k, [8, 20], [10, 24], s, (false, false));
    }
    window(&mut b, fp, 2, [-26, -12], [10, 24], s, (false, false));
    window(&mut b, fp, 2, [12, 26], [12, 24], s, (false, false));
    // Porch lamp and house number.
    face_box(&mut b, fp, 0, [10, 12], [18, 22], [0, 2], IRON);
    face_box(&mut b, fp, 0, [10, 12], [19, 21], [2, 3], [1.0, 0.92, 0.62]);
    face_box(&mut b, fp, 0, [-12, -10], [20, 22], [0, 1], LINE_YELLOW);
    b.socket("base", "house-base", [0, 0, 0], down(), "house-72", true);
    b.socket(
        "top",
        "house-top",
        [0, HOUSE_FLOOR, 0],
        up(),
        "house-72",
        true,
    );
    b.finish()
}

fn house_upper(colour: &str) -> Tile {
    let (s, label) = house_style(colour);
    let mut b = B::new(
        &format!("House upper floor ({})", label.to_lowercase()),
        "building",
    );
    let fp = HOUSE;
    walls(&mut b, fp, 0, 30, s, None);
    band(&mut b, fp, 30, 32, s.trim);
    window(&mut b, fp, 0, [-28, -16], [8, 22], s, (true, false));
    window(&mut b, fp, 0, [-6, 6], [8, 22], s, (false, true));
    window(&mut b, fp, 0, [16, 28], [8, 22], s, (true, false));
    for k in [1u8, 3] {
        window(&mut b, fp, k, [-6, 6], [8, 22], s, (false, false));
    }
    window(&mut b, fp, 2, [-24, -12], [8, 22], s, (false, false));
    window(&mut b, fp, 2, [12, 24], [8, 22], s, (false, false));
    b.socket("base", "house-storey", [0, 0, 0], down(), "house-72", true);
    b.socket(
        "top",
        "house-top",
        [0, HOUSE_FLOOR, 0],
        up(),
        "house-72",
        true,
    );
    b.finish()
}

/// Gable roof: stepped slopes front and back, white gable ends under the eaves.
fn gable_roof(name: &str, roof: Rgb) -> Tile {
    let mut b = B::new(name, "roof");
    let (ox, oz) = (HOUSE.hx + 3, HOUSE.hz + 4);
    let steps = oz / 3;
    for k in 0..steps {
        let z = oz - 3 * k;
        let c = if k % 2 == 0 { roof } else { shade(roof, 0.88) };
        let y = 2 * k;
        if z <= 6 {
            b.fill([-ox, y, -z], [ox, y + 2, z], c);
        } else {
            b.fill([-ox, y, -z], [ox, y + 2, -z + 6], c);
            b.fill([-ox, y, z - 6], [ox, y + 2, z], c);
        }
        // Gable triangles in the wall plane.
        if z - 6 > -(z - 6) {
            for x in [-HOUSE.hx, HOUSE.hx - 2] {
                b.fill([x, y, -(z - 3)], [x + 2, y + 2, z - 3], WHITE);
            }
        }
    }
    let peak = 2 * steps;
    b.fill([-ox, peak - 2, -2], [ox, peak, 2], shade(roof, 0.7));
    // Attic windows and gutters.
    for x in [-HOUSE.hx - 1, HOUSE.hx] {
        b.fill([x, 8, -3], [x + 1, 14, 3], GLASS);
        b.fill([x, 7, -4], [x + 1, 8, 4], WHITE);
    }
    for z in [-oz - 1, oz] {
        b.fill([-ox, 0, z], [ox, 1, z + 1], STONE_DARK);
    }
    // Chimney.
    b.fill([18, 6, -12], [26, peak + 6, -4], BRICK);
    b.fill([17, peak + 6, -13], [27, peak + 8, -3], STONE_DARK);
    b.fill([19, peak + 8, -11], [25, peak + 9, -5], [0.15, 0.15, 0.16]);
    b.collide([-HOUSE.hx, 0, -HOUSE.hz], [HOUSE.hx, peak / 2, HOUSE.hz]);
    b.socket("base", "house-roof", [0, 0, 0], down(), "house-72", true);
    b.finish()
}

/// Hip roof: a stepped pyramid with a dormer on the front.
fn hip_roof(name: &str, roof: Rgb) -> Tile {
    let mut b = B::new(name, "roof");
    let (ox, oz) = (HOUSE.hx + 3, HOUSE.hz + 4);
    let steps = oz / 3;
    for k in 0..steps {
        let c = if k % 2 == 0 { roof } else { shade(roof, 0.88) };
        b.fill(
            [-ox + 3 * k, 2 * k, -oz + 3 * k],
            [ox - 3 * k, 2 * k + 2, oz - 3 * k],
            c,
        );
    }
    let peak = 2 * steps;
    b.fill(
        [-ox + 3 * steps, peak - 2, -2],
        [ox - 3 * steps, peak, 2],
        shade(roof, 0.7),
    );
    // Dormer.
    let fp = Fp { hx: 8, hz: 24 };
    b.fill([-8, 6, 14], [8, 18, 24], WHITE);
    b.fill([-10, 18, 12], [10, 20, 26], roof);
    b.fill([-8, 20, 12], [8, 22, 24], shade(roof, 0.88));
    let s = Style {
        wall: WHITE,
        trim: WHITE,
        accent: roof,
        sill: WHITE,
    };
    window(&mut b, fp, 0, [-5, 5], [9, 16], s, (false, false));
    for z in [-oz - 1, oz] {
        b.fill([-ox, 0, z], [ox, 1, z + 1], STONE_DARK);
    }
    b.fill([-26, 8, -14], [-18, peak + 4, -6], BRICK);
    b.fill([-27, peak + 4, -15], [-17, peak + 6, -5], STONE_DARK);
    b.collide([-HOUSE.hx, 0, -HOUSE.hz], [HOUSE.hx, peak / 2, HOUSE.hz]);
    b.socket("base", "house-roof", [0, 0, 0], down(), "house-72", true);
    b.finish()
}

// ---------------------------------------------------------------------------
// Blocks

const BLOCK: Fp = Fp { hx: 48, hz: 40 };
const SHOP_FLOOR: i32 = 36;
const STOREY: i32 = 32;

fn block_style(name: &str) -> Style {
    match name {
        "brick" => Style {
            wall: BRICK,
            trim: WHITE,
            accent: [0.24, 0.30, 0.36],
            sill: STONE_LIGHT,
        },
        "pink" => Style {
            wall: [0.93, 0.62, 0.66],
            trim: WHITE,
            accent: [0.55, 0.32, 0.40],
            sill: WHITE,
        },
        "cream" => Style {
            wall: [0.93, 0.86, 0.70],
            trim: [0.98, 0.96, 0.90],
            accent: [0.45, 0.36, 0.28],
            sill: STONE_LIGHT,
        },
        _ => Style {
            wall: [0.56, 0.80, 0.74],
            trim: WHITE,
            accent: [0.22, 0.40, 0.42],
            sill: WHITE,
        },
    }
}
pub const BLOCK_STYLES: [&str; 4] = ["brick", "pink", "cream", "mint"];

/// Striped awning over the shopfront with a scalloped valance.
fn awning(b: &mut B, fp: Fp, u: [i32; 2], y: i32, a: Rgb, c: Rgb, stripe: i32) {
    for (d0, d1, dy) in [(0, 4, 3), (4, 8, 2), (8, 11, 1)] {
        let mut x = u[0];
        let mut n = 0;
        while x < u[1] {
            let col = if n % 2 == 0 { a } else { c };
            let x1 = (x + stripe).min(u[1]);
            face_box(b, fp, 0, [x, x1], [y + dy, y + dy + 1], [d0, d1], col);
            if d0 == 8 {
                face_box(b, fp, 0, [x, x1], [y - 1, y + dy], [10, 11], col);
                if n % 2 == 0 {
                    face_box(b, fp, 0, [x, x1], [y - 2, y - 1], [10, 11], col);
                }
            }
            n += 1;
            x = x1;
        }
    }
}

/// Sign board with blocky "lettering".
fn sign(b: &mut B, fp: Fp, u: [i32; 2], v: [i32; 2], board: Rgb, ink: Rgb) {
    face_box(b, fp, 0, u, v, [0, 1], board);
    let mut x = u[0] + 2;
    while x < u[1] - 3 {
        let w = b.rng.range(2, 4);
        let h = v[1] - v[0] - 2;
        if b.rng.chance(85) {
            face_box(b, fp, 0, [x, x + w], [v[0] + 1, v[0] + 1 + h], [1, 2], ink);
            if w == 3 && h > 2 {
                face_box(
                    b,
                    fp,
                    0,
                    [x + 1, x + 2],
                    [v[0] + 2, v[0] + h],
                    [1, 2],
                    board,
                );
            }
        }
        x += w + 1;
    }
}

#[derive(Clone, Copy)]
enum Shop {
    Grocer,
    Cafe,
    Florist,
    Lobby,
}

fn shop_ground(kind: Shop) -> Tile {
    let (name, s, awn): (&str, Style, Option<(Rgb, Rgb)>) = match kind {
        Shop::Grocer => (
            "Corner shop",
            Style {
                wall: [0.90, 0.84, 0.72],
                trim: [0.40, 0.20, 0.42],
                accent: [0.55, 0.26, 0.58],
                sill: STONE_LIGHT,
            },
            Some(([0.62, 0.28, 0.64], [0.50, 0.21, 0.52])),
        ),
        Shop::Cafe => (
            "Cafe",
            Style {
                wall: [0.30, 0.36, 0.34],
                trim: [0.88, 0.84, 0.74],
                accent: [0.86, 0.26, 0.24],
                sill: STONE_LIGHT,
            },
            Some(([0.90, 0.24, 0.22], WHITE)),
        ),
        Shop::Florist => (
            "Florist",
            Style {
                wall: [0.96, 0.94, 0.88],
                trim: [0.24, 0.48, 0.34],
                accent: [0.30, 0.62, 0.42],
                sill: STONE_LIGHT,
            },
            Some(([0.30, 0.62, 0.42], [0.96, 0.95, 0.88])),
        ),
        Shop::Lobby => (
            "Apartment lobby",
            Style {
                wall: STONE_LIGHT,
                trim: STONE,
                accent: [0.25, 0.28, 0.32],
                sill: STONE,
            },
            None,
        ),
    };
    let mut b = B::new(name, "building");
    let fp = BLOCK;
    b.solid(
        [-fp.hx - 1, 0, -fp.hz - 1],
        [fp.hx + 1, 2, fp.hz + 1],
        STONE_DARK,
    );
    b.fill(
        [-fp.hx + 2, 1, -fp.hz + 2],
        [fp.hx - 2, 2, fp.hz - 2],
        [0.78, 0.74, 0.66],
    );
    for x in (-fp.hx + 2..fp.hx - 2).step_by(8) {
        for z in (-fp.hz + 2..fp.hz - 2).step_by(8) {
            if ((x + z) / 8).rem_euclid(2) == 0 {
                b.fill([x, 1, z], [x + 8, 2, z + 8], [0.66, 0.62, 0.56]);
            }
        }
    }
    let door_half = 7;
    walls(
        &mut b,
        fp,
        2,
        SHOP_FLOOR - 2,
        s,
        Some([-door_half, door_half, 24]),
    );
    band(&mut b, fp, SHOP_FLOOR - 2, SHOP_FLOOR, s.trim);
    // Shopfront: stall risers, big panes, pilasters.
    for (u0, u1) in [(-44, -11), (11, 44)] {
        face_box(&mut b, fp, 0, [u0, u1], [2, 7], [0, 1], s.trim);
        face_box(&mut b, fp, 0, [u0 + 1, u1 - 1], [7, 23], [-1, 1], GLASS);
        let mut x = u0 + 8;
        while x < u1 - 2 {
            face_box(&mut b, fp, 0, [x, x + 1], [7, 23], [0, 1], s.trim);
            x += 8;
        }
        face_box(
            &mut b,
            fp,
            0,
            [u0 + 1, u0 + 4],
            [17, 23],
            [0, 1],
            GLASS_LIGHT,
        );
        face_box(&mut b, fp, 0, [u0 + 1, u1 - 1], [7, 11], [0, 1], GLASS_DARK);
        face_box(&mut b, fp, 0, [u0, u1], [23, 24], [0, 2], s.trim);
    }
    for u in [-48, -11, 9, 46] {
        face_box(
            &mut b,
            fp,
            0,
            [u, u + 2],
            [2, SHOP_FLOOR - 2],
            [0, 2],
            s.trim,
        );
    }
    // Door frame and an open glazed door.
    face_box(
        &mut b,
        fp,
        0,
        [-door_half - 2, -door_half],
        [2, 26],
        [-2, 1],
        s.trim,
    );
    face_box(
        &mut b,
        fp,
        0,
        [door_half, door_half + 2],
        [2, 26],
        [-2, 1],
        s.trim,
    );
    face_box(
        &mut b,
        fp,
        0,
        [-door_half - 2, door_half + 2],
        [24, 26],
        [-2, 1],
        s.trim,
    );
    b.fill(
        [door_half, 2, fp.hz - 16],
        [door_half + 1, 24, fp.hz - 2],
        GLASS,
    );
    face_solid(
        &mut b,
        fp,
        0,
        [-door_half - 2, door_half + 2],
        [0, 1],
        [0, 3],
        STONE_LIGHT,
    );
    match awn {
        Some((a, c)) => {
            awning(&mut b, fp, [-46, 46], 24, a, c, 4);
            sign(
                &mut b,
                fp,
                [-30, 30],
                [28, 33],
                s.accent,
                [0.98, 0.95, 0.85],
            );
        }
        None => {
            // Glass entrance canopy and brass letterboxes.
            face_box(&mut b, fp, 0, [-14, 14], [26, 27], [0, 10], GLASS_LIGHT);
            face_box(&mut b, fp, 0, [-14, 14], [27, 28], [0, 10], IRON);
            for x in [-12, 11] {
                face_box(&mut b, fp, 0, [x, x + 1], [24, 27], [9, 10], IRON);
            }
            face_box(
                &mut b,
                fp,
                0,
                [-20, -12],
                [10, 16],
                [0, 1],
                [0.78, 0.62, 0.30],
            );
            sign(&mut b, fp, [-10, 10], [28, 32], STONE, WHITE);
        }
    }
    // Counter and shelves inside.
    b.solid([-30, 2, -10], [-6, 11, -4], WOOD);
    b.fill([-30, 11, -10], [-6, 12, -4], WOOD_LIGHT);
    for x in [-44, 34] {
        b.solid([x, 2, -36], [x + 10, 22, -32], WOOD_DARK);
        for y in [8, 14, 20] {
            b.fill([x + 1, y, -32], [x + 9, y + 1, -31], [0.85, 0.55, 0.30]);
        }
    }
    match kind {
        Shop::Florist => {
            // Buckets of flowers out front.
            for (n, u) in [-40, -32, -24, 24, 32, 40].into_iter().enumerate() {
                face_box(
                    &mut b,
                    fp,
                    0,
                    [u, u + 5],
                    [2, 6],
                    [2, 7],
                    [0.40, 0.45, 0.52],
                );
                let c = [
                    [0.95, 0.35, 0.45],
                    [0.98, 0.85, 0.30],
                    [0.95, 0.60, 0.85],
                    WHITE,
                ][n % 4];
                face_box(&mut b, fp, 0, [u, u + 5], [6, 9], [2, 7], GRASS_DARK);
                face_box(&mut b, fp, 0, [u + 1, u + 4], [9, 10], [3, 6], c);
            }
        }
        Shop::Grocer => {
            // Fruit crates under the windows.
            for (n, u) in [-42, -33, 22, 31].into_iter().enumerate() {
                face_box(&mut b, fp, 0, [u, u + 8], [2, 6], [1, 7], WOOD_LIGHT);
                let c = [
                    [0.92, 0.30, 0.22],
                    [0.98, 0.70, 0.20],
                    [0.55, 0.78, 0.30],
                    [0.96, 0.90, 0.40],
                ][n];
                face_box(&mut b, fp, 0, [u + 1, u + 7], [6, 7], [2, 6], c);
            }
        }
        _ => {}
    }
    // Side and back windows plus a service door.
    for k in [1u8, 3] {
        for u in [-30, -6, 18] {
            window(&mut b, fp, k, [u, u + 12], [10, 26], s, (false, false));
        }
    }
    face_box(&mut b, fp, 2, [-6, 6], [2, 24], [0, 1], WOOD_DARK);
    face_box(&mut b, fp, 2, [-8, 8], [24, 26], [0, 1], s.trim);
    window(&mut b, fp, 2, [-36, -20], [12, 26], s, (false, false));
    window(&mut b, fp, 2, [20, 36], [12, 26], s, (false, false));
    b.socket("base", "block-base", [0, 0, 0], down(), "block-96", true);
    b.socket(
        "top",
        "block-top",
        [0, SHOP_FLOOR, 0],
        up(),
        "block-96",
        true,
    );
    b.finish()
}

fn block_storey(style: &str) -> Tile {
    let s = block_style(style);
    let label = match style {
        "brick" => "Brick",
        "pink" => "Pink",
        "cream" => "Cream",
        _ => "Mint",
    };
    let mut b = B::new(&format!("{label} storey"), "building");
    let fp = BLOCK;
    walls(&mut b, fp, 0, STOREY - 2, s, None);
    band(&mut b, fp, STOREY - 2, STOREY, s.trim);
    if style == "brick" {
        // Mortar courses.
        for k in 0..4u8 {
            let h = fp.half(k);
            for y in (3..STOREY - 2).step_by(4) {
                face_box(
                    &mut b,
                    fp,
                    k,
                    [-h + 2, h - 2],
                    [y, y + 1],
                    [-1, 0],
                    [0.78, 0.50, 0.42],
                );
            }
        }
    }
    let front: &[i32] = &[-40, -18, 6, 28];
    match style {
        "mint" => {
            // Ribbon glazing.
            for k in 0..4u8 {
                let h = fp.half(k) - 6;
                face_box(&mut b, fp, k, [-h - 1, h + 1], [9, 23], [0, 1], s.trim);
                face_box(&mut b, fp, k, [-h, h], [10, 22], [-1, 1], GLASS);
                let mut x = -h + 7;
                while x < h {
                    face_box(&mut b, fp, k, [x, x + 1], [10, 22], [0, 1], s.trim);
                    x += 8;
                }
                face_box(&mut b, fp, k, [-h, -h + 4], [16, 22], [0, 1], GLASS_LIGHT);
                face_box(&mut b, fp, k, [-h - 2, h + 2], [8, 9], [0, 2], s.sill);
            }
        }
        _ => {
            for &u in front {
                window(
                    &mut b,
                    fp,
                    0,
                    [u, u + 12],
                    [8, 23],
                    s,
                    (style == "cream", style == "cream"),
                );
            }
            for k in [1u8, 3] {
                for u in [-28, -6, 16] {
                    window(&mut b, fp, k, [u, u + 12], [8, 23], s, (false, false));
                }
            }
            for &u in front {
                window(&mut b, fp, 2, [u, u + 12], [8, 23], s, (false, false));
            }
        }
    }
    match style {
        "pink" => {
            // Two balconies with railings over french windows.
            for u0 in [-34, 10] {
                let u1 = u0 + 24;
                face_box(&mut b, fp, 0, [u0, u1], [0, 2], [0, 7], WHITE);
                face_box(&mut b, fp, 0, [u0, u1], [2, 3], [6, 7], s.accent);
                face_box(&mut b, fp, 0, [u0, u1], [9, 10], [6, 7], s.accent);
                for x in (u0..u1).step_by(3) {
                    face_box(&mut b, fp, 0, [x, x + 1], [2, 9], [6, 7], s.accent);
                }
                for x in [u0, u1 - 1] {
                    face_box(&mut b, fp, 0, [x, x + 1], [2, 10], [0, 7], s.accent);
                }
                face_box(
                    &mut b,
                    fp,
                    0,
                    [u0 + 2, u0 + 6],
                    [2, 5],
                    [1, 5],
                    [0.70, 0.40, 0.30],
                );
                face_box(
                    &mut b,
                    fp,
                    0,
                    [u0 + 2, u0 + 6],
                    [5, 8],
                    [2, 4],
                    [0.40, 0.68, 0.36],
                );
            }
        }
        "cream" => {
            // Pilasters between bays and a cornice.
            for u in [-46, -24, 0, 22, 44] {
                face_box(&mut b, fp, 0, [u, u + 2], [0, STOREY - 2], [0, 1], s.trim);
            }
            band(&mut b, fp, STOREY - 3, STOREY - 2, STONE_LIGHT);
        }
        "brick" => {
            // Fire escape on the side.
            face_box(&mut b, fp, 1, [-20, 20], [0, 1], [0, 8], IRON);
            face_box(&mut b, fp, 1, [-20, 20], [6, 7], [7, 8], IRON);
            for x in (-20..20).step_by(4) {
                face_box(&mut b, fp, 1, [x, x + 1], [1, 7], [7, 8], IRON);
            }
            for i in 0..8 {
                face_box(
                    &mut b,
                    fp,
                    1,
                    [-18 + 4 * i, -14 + 4 * i],
                    [2 + 3 * i, 3 + 3 * i],
                    [1, 7],
                    IRON,
                );
            }
        }
        _ => {}
    }
    b.socket("base", "block-storey", [0, 0, 0], down(), "block-96", true);
    b.socket("top", "block-top", [0, STOREY, 0], up(), "block-96", true);
    b.finish()
}

fn flat_roof(garden: bool) -> Tile {
    let mut b = B::new(if garden { "Roof garden" } else { "Flat roof" }, "roof");
    let fp = BLOCK;
    let (hx, hz) = (fp.hx + 1, fp.hz + 1);
    b.solid([-hx, 0, -hz], [hx, 2, hz], [0.55, 0.55, 0.56]);
    for k in 0..4u8 {
        let h = fp.half(k) + 1;
        let f = fp.face(k) + 1;
        b.solid_k(k, [-h, 2, f - 2], [h, 6, f], [0.78, 0.76, 0.72]);
        b.fill_k(k, [-h, 6, f - 2], [h, 7, f], [0.86, 0.85, 0.82]);
    }
    b.patches(
        [-hx + 2, -hz + 2],
        [hx - 2, hz - 2],
        2,
        &[[0.50, 0.50, 0.52], [0.60, 0.60, 0.61]],
        14,
        (4, 12),
    );
    if garden {
        // Decking, planters, lawn squares and a pergola.
        b.fill([-hx + 2, 1, -hz + 2], [hx - 2, 2, hz - 2], WOOD_LIGHT);
        for z in (-hz + 2..hz - 2).step_by(3) {
            b.fill([-hx + 2, 1, z], [hx - 2, 2, z + 1], WOOD);
        }
        for (x0, z0) in [(-44, -36), (14, -36), (-44, 10), (24, 14)] {
            b.fill([x0, 2, z0], [x0 + 24, 6, z0 + 22], STONE_LIGHT);
            b.fill([x0 + 1, 6, z0 + 1], [x0 + 23, 7, z0 + 21], GRASS);
            b.patches(
                [x0 + 1, z0 + 1],
                [x0 + 23, z0 + 21],
                7,
                &[GRASS_DARK, GRASS_LIGHT],
                6,
                (2, 5),
            );
            for _ in 0..5 {
                let x = b.rng.range(x0 + 2, x0 + 20);
                let z = b.rng.range(z0 + 2, z0 + 18);
                let c = *b
                    .rng
                    .pick(&[[0.95, 0.40, 0.50], [0.98, 0.85, 0.30], WHITE, GRASS_DARK]);
                b.fill([x, 7, z], [x + 2, 9, z + 2], c);
            }
        }
        // A little tree in the biggest planter.
        b.fill([34, 7, 22], [37, 18, 25], BARK);
        b.fill([28, 16, 16], [43, 26, 31], [0.36, 0.62, 0.30]);
        b.fill([31, 26, 19], [40, 29, 28], [0.44, 0.70, 0.34]);
        for x in [-8, 8] {
            for z in [-8, 8] {
                b.fill([x, 2, z], [x + 2, 20, z + 2], WOOD_DARK);
            }
        }
        for z in (-10..12).step_by(4) {
            b.fill([-10, 20, z], [12, 21, z + 1], WOOD);
        }
        b.fill([-6, 2, -4], [6, 6, 4], WOOD);
    } else {
        // Air-conditioning units, vents, a water tank and the stair hut.
        for (x, z) in [(-36, -26), (-20, -26), (22, 18)] {
            b.fill([x, 2, z], [x + 12, 9, z + 10], [0.82, 0.83, 0.84]);
            b.fill([x + 2, 9, z + 2], [x + 10, 10, z + 8], [0.40, 0.42, 0.45]);
            b.fill([x + 4, 9, z + 3], [x + 8, 10, z + 7], [0.62, 0.64, 0.66]);
        }
        for (x, z) in [(10, -30), (-40, 24), (36, -10)] {
            b.fill([x, 2, z], [x + 3, 7, z + 3], [0.50, 0.52, 0.55]);
            b.fill([x - 1, 7, z - 1], [x + 4, 8, z + 4], [0.40, 0.42, 0.45]);
        }
        b.solid([-12, 2, 4], [12, 22, 24], [0.74, 0.72, 0.68]);
        b.fill([-14, 22, 2], [14, 24, 26], [0.55, 0.55, 0.56]);
        b.fill([-4, 2, 24], [4, 16, 25], WOOD_DARK);
        for (x, z) in [(26, -32), (38, -32), (26, -20), (38, -20)] {
            b.fill([x, 2, z], [x + 2, 14, z + 2], WOOD_DARK);
        }
        b.fill([24, 14, -34], [42, 30, -16], [0.58, 0.42, 0.30]);
        b.fill([26, 30, -32], [40, 33, -18], [0.45, 0.32, 0.24]);
        for y in [17, 22, 27] {
            b.fill([23, y, -35], [43, y + 1, -15], IRON);
        }
    }
    b.socket("base", "block-roof", [0, 0, 0], down(), "block-96", true);
    b.finish()
}

fn chapel() -> Tile {
    let mut b = B::new("Chapel", "building");
    let fp = Fp { hx: 30, hz: 32 };
    let s = Style {
        wall: [0.96, 0.95, 0.92],
        trim: [0.80, 0.78, 0.74],
        accent: [0.32, 0.40, 0.58],
        sill: STONE_LIGHT,
    };
    // Nave set back so the tower stands at the front.
    let z0 = -8;
    let nave = |z: i32| z + z0;
    b.solid(
        [-fp.hx - 1, 0, nave(-fp.hz - 1)],
        [fp.hx + 1, 2, nave(fp.hz + 1)],
        STONE,
    );
    b.fill(
        [-fp.hx + 2, 1, nave(-fp.hz + 2)],
        [fp.hx - 2, 2, nave(fp.hz - 2)],
        [0.62, 0.30, 0.26],
    );
    let wall_top = 44;
    // Nave walls (front opening into the tower).
    b.solid(
        [-fp.hx, 2, nave(-fp.hz)],
        [fp.hx, wall_top, nave(-fp.hz + 2)],
        s.wall,
    );
    b.solid(
        [-fp.hx, 2, nave(-fp.hz)],
        [-fp.hx + 2, wall_top, nave(fp.hz)],
        s.wall,
    );
    b.solid(
        [fp.hx - 2, 2, nave(-fp.hz)],
        [fp.hx, wall_top, nave(fp.hz)],
        s.wall,
    );
    b.solid(
        [-fp.hx, 2, nave(fp.hz - 2)],
        [-8, wall_top, nave(fp.hz)],
        s.wall,
    );
    b.solid(
        [8, 2, nave(fp.hz - 2)],
        [fp.hx, wall_top, nave(fp.hz)],
        s.wall,
    );
    b.solid(
        [-8, 28, nave(fp.hz - 2)],
        [8, wall_top, nave(fp.hz)],
        s.wall,
    );
    // Tall pointed windows with coloured glass.
    let glass = [
        [0.35, 0.50, 0.85],
        [0.85, 0.30, 0.35],
        [0.95, 0.80, 0.30],
        [0.40, 0.70, 0.45],
    ];
    for x in [-fp.hx - 1, fp.hx] {
        for (n, z) in [-20, -4, 12].into_iter().enumerate() {
            let z = nave(z);
            b.fill([x, 12, z - 1], [x + 1, 36, z + 7], s.trim);
            b.fill([x, 13, z], [x + 1, 33, z + 6], glass[n % 4]);
            b.fill([x, 33, z + 1], [x + 1, 35, z + 5], glass[(n + 1) % 4]);
            b.fill([x, 13, z + 2], [x + 1, 33, z + 4], glass[(n + 2) % 4]);
        }
    }
    // Steep roof.
    let steps = 15;
    for k in 0..steps {
        let w = fp.hx + 3 - 2 * k;
        if w <= 0 {
            break;
        }
        let c = if k % 2 == 0 {
            ROOF_SLATE
        } else {
            shade(ROOF_SLATE, 0.86)
        };
        b.fill(
            [-w, wall_top + 3 * k, nave(-fp.hz - 3)],
            [w, wall_top + 3 * k + 3, nave(fp.hz + 1)],
            c,
        );
    }
    // Tower with belfry and spire.
    let (tx, tz0, tz1) = (12, nave(fp.hz) - 4, nave(fp.hz) + 18);
    b.solid([-tx, 0, tz0], [tx, 2, tz1 + 1], STONE);
    b.solid([-tx, 2, tz0], [-tx + 2, 84, tz1], s.wall);
    b.solid([tx - 2, 2, tz0], [tx, 84, tz1], s.wall);
    b.solid([-tx, 2, tz0], [tx, 84, tz0 + 2], s.wall);
    b.solid([-tx, 2, tz1 - 2], [-6, 84, tz1], s.wall);
    b.solid([6, 2, tz1 - 2], [tx, 84, tz1], s.wall);
    b.solid([-6, 26, tz1 - 2], [6, 84, tz1], s.wall);
    b.fill(
        [-tx + 2, 1, tz0 + 2],
        [tx - 2, 2, tz1 - 2],
        [0.62, 0.30, 0.26],
    );
    // Door arch, rose window, clock, belfry louvres.
    b.fill([-8, 2, tz1], [-6, 28, tz1 + 1], s.trim);
    b.fill([6, 2, tz1], [8, 28, tz1 + 1], s.trim);
    b.fill([-8, 26, tz1], [8, 28, tz1 + 1], s.trim);
    b.fill([-4, 28, tz1], [4, 30, tz1 + 1], s.trim);
    b.fill([-5, 36, tz1], [5, 46, tz1 + 1], s.trim);
    b.fill([-4, 37, tz1], [4, 45, tz1 + 1], glass[0]);
    b.fill([-1, 37, tz1], [1, 45, tz1 + 1], glass[2]);
    b.fill([-4, 40, tz1], [4, 42, tz1 + 1], glass[2]);
    b.fill([-5, 54, tz1], [5, 64, tz1 + 1], WHITE);
    b.fill([-4, 55, tz1], [4, 63, tz1 + 1], [0.98, 0.97, 0.93]);
    b.fill([0, 59, tz1], [1, 63, tz1 + 2], IRON);
    b.fill([0, 59, tz1], [3, 60, tz1 + 2], IRON);
    for k in 0..4u8 {
        let (lo, hi) = turn_box(k, [-5, 70, 0], [5, 80, 1]);
        let off = turn_point(k, [0, 0, 11]);
        b.fill(
            [lo[0] + off[0], lo[1], lo[2] + off[2] + (tz0 + tz1) / 2],
            [hi[0] + off[0], hi[1], hi[2] + off[2] + (tz0 + tz1) / 2],
            [0.25, 0.22, 0.20],
        );
    }
    b.fill([-tx - 1, 84, tz0 - 1], [tx + 1, 86, tz1 + 1], s.trim);
    let cz = (tz0 + tz1) / 2;
    for k in 0..10 {
        let w = 11 - k;
        b.fill(
            [-w, 86 + 5 * k, cz - w],
            [w, 91 + 5 * k, cz + w],
            if k % 2 == 0 {
                ROOF_SLATE
            } else {
                shade(ROOF_SLATE, 0.86)
            },
        );
    }
    b.fill([0, 136, cz], [1, 146, cz + 1], LINE_YELLOW);
    b.fill([-3, 141, cz], [4, 142, cz + 1], LINE_YELLOW);
    // Pews.
    for z in (nave(-20)..nave(18)).step_by(7) {
        b.solid([-22, 2, z], [-4, 7, z + 3], WOOD);
        b.solid([4, 2, z], [22, 7, z + 3], WOOD);
    }
    face_solid(
        &mut b,
        Fp { hx: 12, hz: tz1 },
        0,
        [-8, 8],
        [0, 1],
        [0, 4],
        STONE_LIGHT,
    );
    b.socket("base", "block-base", [0, 0, 0], down(), "block-96", true);
    b.finish()
}

pub fn tiles() -> Vec<Tile> {
    let mut t = vec![];
    for c in HOUSE_COLOURS {
        t.push(house_ground(c));
        t.push(house_upper(c));
    }
    t.push(gable_roof("Gable roof (red)", ROOF_RED));
    t.push(gable_roof("Gable roof (slate)", ROOF_SLATE));
    t.push(hip_roof("Hip roof (terracotta)", [0.80, 0.45, 0.28]));
    t.push(hip_roof("Hip roof (green)", ROOF_GREEN));
    for kind in [Shop::Grocer, Shop::Cafe, Shop::Florist, Shop::Lobby] {
        t.push(shop_ground(kind));
    }
    for s in BLOCK_STYLES {
        t.push(block_storey(s));
    }
    t.push(flat_roof(false));
    t.push(flat_roof(true));
    t.push(chapel());
    t
}
