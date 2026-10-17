//! Street furniture, fences and footbridges.
use super::*;
use pal::*;

fn lamp() -> Tile {
    let mut b = B::new("Street lamp", "street");
    let post = [0.16, 0.24, 0.22];
    b.solid([-2, 0, -2], [2, 4, 2], post);
    b.fill([-3, 0, -3], [3, 1, 3], shade(post, 0.8));
    b.solid([-1, 4, -1], [1, 34, 1], post);
    b.fill([-2, 14, -2], [2, 15, 2], shade(post, 1.3));
    // Lantern: dark cap, glowing panes, finial.
    b.fill([-3, 34, -3], [3, 35, 3], post);
    b.fill([-2, 35, -2], [2, 40, 2], [1.0, 0.92, 0.62]);
    for (x, z) in [(-3, -3), (2, -3), (-3, 2), (2, 2)] {
        b.fill([x, 35, z], [x + 1, 40, z + 1], post);
    }
    b.fill([-4, 40, -4], [4, 41, 4], post);
    b.fill([-3, 41, -3], [3, 42, 3], post);
    b.fill([-1, 42, -1], [1, 44, 1], post);
    b.base("street-base", "spot");
    b.finish()
}

fn bench() -> Tile {
    let mut b = B::new("Bench", "street");
    for x in [-7, 5] {
        b.fill([x, 0, -2], [x + 2, 5, 1], IRON);
        b.fill([x, 5, -3], [x + 2, 10, -2], IRON);
    }
    for (n, z) in [-2, 0].into_iter().enumerate() {
        b.fill(
            [-8, 4, z],
            [8, 5, z + 2],
            if n == 0 { WOOD } else { WOOD_LIGHT },
        );
    }
    b.fill([-8, 6, -3], [8, 7, -2], WOOD);
    b.fill([-8, 8, -3], [8, 9, -2], WOOD_LIGHT);
    b.collide([-8, 0, -3], [8, 5, 2]);
    b.base("street-base", "spot");
    b.finish()
}

fn hydrant() -> Tile {
    let mut b = B::new("Fire hydrant", "street");
    let red = [0.86, 0.20, 0.18];
    b.solid([-2, 0, -2], [2, 7, 2], red);
    b.fill([-3, 0, -3], [3, 1, 3], shade(red, 0.8));
    b.fill([-1, 7, -1], [1, 9, 1], shade(red, 0.8));
    b.fill([-3, 4, -1], [3, 5, 1], [0.95, 0.85, 0.30]);
    b.fill([-1, 3, 2], [1, 5, 3], [0.95, 0.85, 0.30]);
    b.base("street-base", "spot");
    b.finish()
}

fn bin() -> Tile {
    let mut b = B::new("Litter bin", "street");
    let green = [0.22, 0.42, 0.32];
    b.solid([-3, 0, -3], [3, 8, 3], green);
    for k in 0..4u8 {
        b.fill_k(k, [-2, 1, 2], [-1, 7, 3], shade(green, 0.8));
        b.fill_k(k, [1, 1, 2], [2, 7, 3], shade(green, 0.8));
    }
    b.fill([-3, 8, -3], [3, 9, 3], IRON);
    b.fill([-2, 9, -2], [2, 10, 2], shade(green, 1.1));
    b.base("street-base", "spot");
    b.finish()
}

fn postbox() -> Tile {
    let mut b = B::new("Post box", "street");
    let red = [0.82, 0.16, 0.16];
    b.solid([-3, 0, -3], [3, 11, 3], red);
    b.fill([-2, 11, -2], [2, 12, 2], red);
    b.fill([-3, 0, -3], [3, 1, 3], IRON);
    b.fill([-2, 8, 3], [2, 9, 4], IRON);
    b.fill([-1, 5, 3], [1, 7, 4], WHITE);
    b.base("street-base", "spot");
    b.finish()
}

fn cafe_table() -> Tile {
    let mut b = B::new("Cafe table", "street");
    b.fill([-1, 0, -1], [1, 6, 1], IRON);
    b.solid([-4, 6, -3], [4, 7, 3], WHITE);
    b.fill([-3, 6, -4], [3, 7, 4], WHITE);
    for k in [0u8, 2] {
        b.fill_k(k, [-2, 0, 5], [2, 4, 8], WOOD);
        b.fill_k(k, [-2, 4, 7], [2, 8, 8], WOOD_DARK);
    }
    // Parasol: alternating panels in rings.
    b.fill([0, 7, 0], [1, 19, 1], WOOD_LIGHT);
    let (a, c) = ([0.90, 0.30, 0.26], WHITE);
    for (ring, y) in [(9, 17), (6, 18), (3, 19)] {
        for k in 0..4u8 {
            let col = if k % 2 == 0 { a } else { c };
            b.fill_k(k, [-ring, y, 0], [0, y + 1, ring], col);
        }
    }
    b.fill([-1, 20, -1], [2, 21, 2], c);
    b.base("street-base", "spot");
    b.finish()
}

fn planter() -> Tile {
    let mut b = B::new("Planter", "street");
    b.solid([-6, 0, -6], [6, 6, 6], [0.62, 0.56, 0.50]);
    b.fill([-6, 5, -6], [6, 6, 6], STONE_LIGHT);
    b.fill([-5, 5, -5], [5, 6, 5], SOIL);
    b.fill([-4, 6, -4], [4, 12, 4], [0.30, 0.55, 0.28]);
    b.fill([-3, 12, -3], [3, 15, 3], [0.36, 0.62, 0.32]);
    for _ in 0..8 {
        let k = (b.rng.next_u64() % 4) as u8;
        let x = b.rng.range(-4, 3);
        let y = b.rng.range(7, 13);
        b.fill_k(k, [x, y, 3], [x + 1, y + 1, 4], [0.96, 0.85, 0.30]);
    }
    b.base("street-base", "spot");
    b.finish()
}

fn bus_stop() -> Tile {
    let mut b = B::new("Bus stop", "street");
    for x in [-12, 10] {
        b.solid([x, 0, -5], [x + 2, 24, -3], IRON);
    }
    b.fill([-10, 2, -5], [10, 22, -4], GLASS_LIGHT);
    b.fill([-10, 12, -5], [10, 13, -4], IRON);
    b.fill([-14, 24, -7], [14, 26, 6], [0.80, 0.25, 0.22]);
    b.fill([-13, 26, -6], [13, 27, 5], [0.90, 0.88, 0.84]);
    b.fill([-8, 4, -4], [8, 5, -1], WOOD);
    b.fill([-8, 0, -3], [-7, 4, -2], IRON);
    b.fill([7, 0, -3], [8, 4, -2], IRON);
    b.fill([12, 0, 3], [13, 26, 4], IRON);
    b.fill([11, 26, 2], [15, 30, 5], [0.20, 0.42, 0.80]);
    b.fill([12, 27, 5], [14, 29, 6], WHITE);
    b.collide([-12, 0, -5], [12, 24, -3]);
    b.base("street-base", "spot");
    b.finish()
}

fn picnic_table() -> Tile {
    let mut b = B::new("Picnic table", "street");
    b.solid([-9, 6, -4], [9, 7, 4], WOOD_LIGHT);
    for z in [-8, 5] {
        b.fill([-9, 3, z], [9, 4, z + 3], WOOD);
    }
    for x in [-7, 6] {
        b.fill([x, 0, -7], [x + 1, 6, -6], WOOD_DARK);
        b.fill([x, 0, 5], [x + 1, 6, 6], WOOD_DARK);
        b.fill([x, 2, -7], [x + 1, 3, 6], WOOD_DARK);
    }
    b.collide([-9, 0, -8], [9, 7, 8]);
    b.base("street-base", "spot");
    b.finish()
}

// ---------------------------------------------------------------------------
// Fences: 16 voxels long along X, centred on their socket.

fn picket() -> Tile {
    let mut b = B::new("Picket fence", "fence");
    b.fill([-8, 3, -1], [8, 4, 0], WHITE);
    b.fill([-8, 7, -1], [8, 8, 0], WHITE);
    for x in (-8..8).step_by(3) {
        b.fill([x, 0, 0], [x + 2, 9, 1], WHITE);
        b.fill([x, 9, 0], [x + 1, 10, 1], WHITE);
    }
    b.collide([-8, 0, -1], [8, 9, 1]);
    b.base("fence-base", "fence-16");
    b.finish()
}

fn wooden_fence() -> Tile {
    let mut b = B::new("Wooden fence", "fence");
    b.fill([-8, 0, -1], [-6, 11, 1], WOOD_DARK);
    for y in [2, 5, 8] {
        b.fill(
            [-8, y, -1],
            [8, y + 2, 0],
            if y == 5 { WOOD_LIGHT } else { WOOD },
        );
    }
    b.collide([-8, 0, -1], [8, 10, 1]);
    b.base("fence-base", "fence-16");
    b.finish()
}

fn hedge_fence() -> Tile {
    let mut b = B::new("Low hedge", "fence");
    let c = [0.28, 0.52, 0.26];
    b.solid([-8, 0, -2], [8, 8, 2], c);
    b.fill([-8, 8, -2], [8, 9, 2], mix(c, [1., 1., 0.8], 0.2));
    for _ in 0..14 {
        let k = if b.rng.chance(50) { 0 } else { 2 };
        let x = b.rng.range(-8, 7);
        let y = b.rng.range(0, 8);
        let tone = *b.rng.pick(&[shade(c, 0.8), mix(c, [1., 1., 0.7], 0.3)]);
        b.fill_k(k, [x, y, 1], [x + 1, y + 1, 2], tone);
    }
    b.base("fence-base", "fence-16");
    b.finish()
}

fn stone_wall() -> Tile {
    let mut b = B::new("Stone wall", "fence");
    b.solid([-8, 0, -2], [8, 6, 2], STONE);
    for y in [0, 3] {
        let mut x = -8 + if y == 3 { 2 } else { 0 };
        while x < 8 {
            let w = b.rng.range(3, 6);
            let c = shade(STONE, 0.88 + b.rng.unit() * 0.2);
            b.fill([x.max(-8), y, -2], [(x + w).min(8), y + 3, 2], c);
            b.fill([x.max(-8), y, -2], [(x + 1).min(8), y + 3, 2], STONE_DARK);
            x += w;
        }
    }
    b.fill([-8, 6, -2], [8, 7, 2], STONE_LIGHT);
    b.base("fence-base", "fence-16");
    b.finish()
}

fn railing() -> Tile {
    let mut b = B::new("Iron railing", "fence");
    b.fill([-8, 1, -1], [8, 2, 0], IRON);
    b.fill([-8, 9, -1], [8, 10, 0], IRON);
    for x in (-8..8).step_by(2) {
        b.fill([x, 0, -1], [x + 1, 11, 0], IRON);
    }
    b.fill([-8, 0, -1], [-7, 12, 0], IRON);
    b.collide([-8, 0, -1], [8, 10, 0]);
    b.base("fence-base", "fence-16");
    b.finish()
}

// ---------------------------------------------------------------------------
// Footbridges span a stream crossing; y = 0 is the bank's lawn.

fn footbridge() -> Tile {
    let mut b = B::new("Wooden footbridge", "bridge");
    // An arched deck in 0.2 rises; the outer steps rest on the lawn.
    let deck = [
        (-28, -22, 0, 1),
        (22, 28, 0, 1),
        (-22, -16, 1, 3),
        (16, 22, 1, 3),
        (-16, -10, 3, 5),
        (10, 16, 3, 5),
        (-10, 10, 5, 7),
    ];
    for (x0, x1, y0, y1) in deck {
        b.solid([x0, y0, -8], [x1, y1, 8], WOOD);
        for z in (-8..8).step_by(4) {
            b.fill([x0, y1 - 1, z], [x1, y1, z + 1], WOOD_DARK);
        }
    }
    // Rails and posts down to the bank steps.
    for z in [-8, 7] {
        for (x, top) in [
            (-28, 1),
            (-22, 3),
            (-16, 5),
            (-10, 7),
            (8, 7),
            (14, 5),
            (20, 3),
            (26, 1),
        ] {
            b.fill([x, top, z], [x + 2, top + 9, z + 1], WOOD_DARK);
        }
        for (x0, x1, y) in [
            (-28, -22, 9),
            (-22, -16, 11),
            (-16, -10, 13),
            (-10, 10, 15),
            (10, 16, 13),
            (16, 22, 11),
            (22, 28, 9),
        ] {
            b.fill([x0, y, z], [x1, y + 1, z + 1], WOOD_LIGHT);
        }
    }
    for x in [-18, 16] {
        for z in [-8, 6] {
            b.fill([x, -6, z], [x + 2, 3, z + 2], WOOD_DARK);
        }
    }
    b.base("bridge-base", "span-48");
    b.finish()
}

fn stepping_stones() -> Tile {
    let mut b = B::new("Stepping stones", "bridge");
    for (i, x) in [-14, -6, 2, 10].into_iter().enumerate() {
        let z = if i % 2 == 0 { -4 } else { -1 };
        let c = *b.rng.pick(&[STONE, STONE_LIGHT]);
        b.solid([x, -6, z], [x + 6, -4, z + 6], c);
        b.fill([x + 1, -4, z + 1], [x + 3, -3, z + 3], shade(c, 1.08));
    }
    b.base("bridge-base", "span-48");
    b.finish()
}

pub fn tiles() -> Vec<Tile> {
    vec![
        lamp(),
        bench(),
        hydrant(),
        bin(),
        postbox(),
        cafe_table(),
        planter(),
        bus_stop(),
        picnic_table(),
        picket(),
        wooden_fence(),
        hedge_fence(),
        stone_wall(),
        railing(),
        footbridge(),
        stepping_stones(),
    ]
}
