//! Deterministic authored voxel kit and bounded constraint generation.
use crate::tiles::World;
use crate::{model::Library, tiles::*};
use bevy::prelude::*;
use std::collections::BTreeMap;

const GRASS: [f32; 3] = [0.30, 0.53, 0.22];
const PAVE: [f32; 3] = [0.65, 0.66, 0.61];
const ROAD: [f32; 3] = [0.17, 0.20, 0.23];
const WALL: [f32; 3] = [0.87, 0.74, 0.49];
const ROOF: [f32; 3] = [0.57, 0.19, 0.12];
fn up() -> Quat {
    Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2)
}
fn down() -> Quat {
    up() * Quat::from_rotation_y(std::f32::consts::PI)
}
fn socket(name: &str, kind: &str, p: [i32; 3], r: Quat, profile: &str, required: bool) -> Socket {
    Socket {
        name: name.into(),
        kind: kind.into(),
        position: p,
        rotation: r.to_array(),
        profile: profile.into(),
        required,
    }
}
fn piece(id: u32, name: &str, category: &str) -> Tile {
    Tile {
        id,
        name: name.into(),
        category: category.into(),
        voxels: vec![],
        colliders: vec![],
        sockets: vec![],
        rotations: (0..4).map(yaw).collect(),
    }
}
fn volume(t: &mut Tile, min: [i32; 3], max: [i32; 3], color: [f32; 3], solid: bool) {
    if (0..3).any(|a| min[a] >= max[a]) {
        return;
    }
    t.voxels.push(Volume { min, max, color });
    if solid {
        t.colliders.push(Bounds {
            min: min.map(|v| v as f32 * UNIT),
            max: max.map(|v| v as f32 * UNIT),
        });
    }
}
fn terrain(id: u32, name: &str, mask: u8, plot: bool) -> Tile {
    let mut t = piece(id, name, if plot { "plot" } else { "terrain" });
    if mask == 0 {
        volume(&mut t, [-64, -4, -64], [64, 0, 64], GRASS, true);
        if plot {
            volume(&mut t, [-64, 0, -64], [-32, 2, 64], GRASS, true);
            volume(&mut t, [32, 0, -64], [64, 2, 64], GRASS, true);
            volume(&mut t, [-32, 0, -64], [-8, 2, 0], GRASS, true);
            volume(&mut t, [8, 0, -64], [32, 2, 0], GRASS, true);
            // A house and its path replace the reserved topsoil; no coplanar overlay.
            t.sockets.push(socket(
                "house",
                "plot-house",
                [0, 0, 32],
                up(),
                "house-64",
                true,
            ));
        } else {
            volume(&mut t, [-64, 0, -64], [64, 2, 64], GRASS, true);
            for (n, p) in [[-40, 2, -40], [40, 2, 40]].into_iter().enumerate() {
                t.sockets.push(socket(
                    &format!("plant-{n}"),
                    "plant",
                    p,
                    up(),
                    "tree",
                    false,
                ));
            }
        }
    } else {
        let edges = [-64, -32, 32, 64];
        for z in 0..3 {
            for x in 0..3 {
                let asphalt = (x == 1 && z == 1)
                    || (x == 1 && z == 0 && mask & 1 != 0)
                    || (x == 2 && z == 1 && mask & 2 != 0)
                    || (x == 1 && z == 2 && mask & 4 != 0)
                    || (x == 0 && z == 1 && mask & 8 != 0);
                volume(
                    &mut t,
                    [edges[x], -4, edges[z]],
                    [edges[x + 1], if asphalt { 0 } else { 2 }, edges[z + 1]],
                    if asphalt { ROAD } else { PAVE },
                    true,
                );
            }
        }
        // Dashed centre lines stop before intersection centres; corner uses an L marking.
        for arm in 0..4 {
            if mask & (1 << arm) == 0 {
                continue;
            }
            for z in [-60, -44] {
                let mut v = piece(0, "", "unused");
                volume(
                    &mut v,
                    [-1, -1, z],
                    [1, 0, z + 8],
                    [0.96, 0.88, 0.54],
                    false,
                );
                for b in v.voxels {
                    let r = Quat::from_array(yaw(4 - arm));
                    let a = r * Vec3::from_array(b.min.map(|v| v as f32));
                    let c = r * Vec3::from_array(b.max.map(|v| v as f32));
                    volume(
                        &mut t,
                        a.min(c).round().to_array().map(|v| v as i32),
                        a.max(c).round().to_array().map(|v| v as i32),
                        b.color,
                        false,
                    );
                }
            }
        }
        if mask == 5 {
            for z in [-28, -12, 4, 20] {
                volume(
                    &mut t,
                    [-1, -1, z],
                    [1, 0, z + 8],
                    [0.96, 0.88, 0.54],
                    false,
                );
            }
        }
    }
    for (n, p) in [[0, -2, -64], [64, -2, 0], [0, -2, 64], [-64, -2, 0]]
        .into_iter()
        .enumerate()
    {
        let r = Quat::from_array(yaw(((6 - n) % 4) as u8));
        let road = mask & (1 << n) != 0;
        t.sockets.push(socket(
            &format!("edge-{n}"),
            if road {
                "road"
            } else if plot && n == 0 {
                "frontage"
            } else if mask != 0 {
                "side-land"
            } else {
                "land"
            },
            p,
            r,
            if road { "road-128" } else { "land-128" },
            road || plot && n == 0,
        ));
        if mask != 0 && !road {
            t.sockets.push(socket(
                &format!("access-{n}"),
                "sidewalk",
                [p[0], 2, p[2]],
                r,
                "path-16",
                false,
            ));
        }
    }
    t
}
pub fn starter_tiles(first: u32) -> (Vec<Tile>, SocketRules) {
    let mut ts = vec![
        terrain(first, "Grass", 0, false),
        terrain(first + 1, "House plot", 0, true),
        terrain(first + 2, "Road straight", 5, false),
        terrain(first + 3, "Road corner", 3, false),
        terrain(first + 4, "Road T", 11, false),
        terrain(first + 5, "Road crossing", 15, false),
    ];
    let mut straight = piece(first + 6, "Walkway straight", "walkway");
    volume(&mut straight, [-8, -2, 0], [8, 0, 16], PAVE, true);
    straight.sockets = vec![
        socket(
            "start",
            "path",
            [0, 0, 0],
            Quat::from_rotation_y(std::f32::consts::PI),
            "path-16",
            true,
        ),
        socket("end", "path", [0, 0, 16], Quat::IDENTITY, "path-16", true),
    ];
    ts.push(straight);
    let mut corner = piece(first + 7, "Walkway corner", "walkway");
    volume(&mut corner, [-8, -2, 0], [8, 0, 16], PAVE, true);
    corner.sockets = vec![
        socket(
            "start",
            "path",
            [0, 0, 0],
            Quat::from_rotation_y(std::f32::consts::PI),
            "path-16",
            true,
        ),
        socket(
            "end",
            "path",
            [8, 0, 8],
            Quat::from_rotation_y(std::f32::consts::FRAC_PI_2),
            "path-16",
            true,
        ),
    ];
    ts.push(corner);
    let mut floor = piece(first + 8, "House floor", "floor");
    volume(
        &mut floor,
        [-32, 0, -32],
        [32, 2, 32],
        [0.63, 0.43, 0.26],
        true,
    );
    floor.sockets.push(socket(
        "ridge",
        "ridge-mount",
        [0, 60, 0],
        up(),
        "ridge-64",
        true,
    ));
    floor.sockets.push(socket(
        "base",
        "house-base",
        [0, 0, 0],
        down(),
        "house-64",
        true,
    ));
    for (n, p) in [[0, 2, -32], [32, 2, 0], [0, 2, 32], [-32, 2, 0]]
        .into_iter()
        .enumerate()
    {
        floor.sockets.push(socket(
            &format!("wall-{n}"),
            if n == 0 {
                "floor-door"
            } else if n % 2 == 1 {
                "floor-side"
            } else {
                "floor-wall"
            },
            p,
            Quat::from_array(yaw(((4 - n) % 4) as u8)) * up(),
            "wall-64",
            true,
        ));
    }
    ts.push(floor);
    for (offset, name, category, kind) in [
        (9, "Front door", "door", "door-base"),
        (10, "Wall window", "wall", "wall-base"),
        (11, "Side wall", "wall", "side-base"),
    ] {
        let mut w = piece(first + offset, name, category);
        let half = if offset == 11 { 30 } else { 32 };
        if offset == 9 {
            volume(&mut w, [-half, 0, 0], [-9, 42, 2], WALL, true);
            volume(&mut w, [9, 0, 0], [half, 42, 2], WALL, true);
            volume(&mut w, [-9, 36, 0], [9, 42, 2], WALL, true);
            w.sockets.push(socket(
                "entrance",
                "door",
                [0, 0, 0],
                Quat::from_rotation_y(std::f32::consts::PI),
                "path-16",
                true,
            ));
        } else {
            volume(&mut w, [-half, 0, 0], [half, 42, 2], WALL, true);
            volume(&mut w, [-10, 16, 0], [10, 30, 1], [0.28, 0.59, 0.69], false);
        }
        w.sockets
            .push(socket("base", kind, [0, 0, 0], down(), "wall-64", true));
        w.sockets.push(socket(
            "roof",
            if offset == 11 {
                "wall-slope"
            } else {
                "wall-gable"
            },
            [0, 42, 0],
            up(),
            "roof-64",
            true,
        ));
        ts.push(w);
    }
    let mut slope = piece(first + 12, "Roof slope", "roof");
    for z in (0..32).step_by(2) {
        volume(
            &mut slope,
            [-32, z / 2, z],
            [32, z / 2 + 1, z + 2],
            ROOF,
            true,
        );
    }
    slope.sockets.push(socket(
        "base",
        "roof-slope",
        [0, 0, 0],
        down(),
        "roof-64",
        true,
    ));
    ts.push(slope);
    let mut gable = piece(first + 13, "Roof gable", "roof");
    for x in (-30i32..30).step_by(2) {
        let height = (32 - x.abs().max((x + 2).abs())) / 2;
        volume(&mut gable, [x, 0, 0], [x + 2, height, 2], WALL, true);
    }
    gable.sockets.push(socket(
        "base",
        "roof-gable",
        [0, 0, 0],
        down(),
        "roof-64",
        true,
    ));
    ts.push(gable);
    let mut trunk = piece(first + 14, "Tree trunk", "tree");
    volume(
        &mut trunk,
        [-3, 0, -3],
        [3, 16, 3],
        [0.35, 0.23, 0.13],
        true,
    );
    trunk.sockets = vec![
        socket("base", "trunk-base", [0, 0, 0], down(), "tree", true),
        socket("top", "trunk-top", [0, 16, 0], up(), "tree", true),
        socket(
            "branch",
            "branch-mount",
            [3, 12, 0],
            Quat::from_rotation_y(std::f32::consts::FRAC_PI_2),
            "branch",
            false,
        ),
    ];
    ts.push(trunk);
    let mut branch = piece(first + 15, "Tree branch", "tree");
    volume(
        &mut branch,
        [0, 0, -2],
        [12, 4, 2],
        [0.35, 0.23, 0.13],
        true,
    );
    branch.sockets = vec![
        socket(
            "base",
            "branch-base",
            [0, 0, 0],
            Quat::from_rotation_y(-std::f32::consts::FRAC_PI_2),
            "branch",
            true,
        ),
        socket("tip", "trunk-top", [10, 4, 0], up(), "tree", true),
    ];
    ts.push(branch);
    let mut leaves = piece(first + 16, "Tree foliage", "tree");
    volume(
        &mut leaves,
        [-10, 0, -10],
        [10, 12, 10],
        [0.20, 0.42, 0.16],
        false,
    );
    volume(
        &mut leaves,
        [-7, 12, -7],
        [7, 17, 7],
        [0.28, 0.49, 0.19],
        false,
    );
    leaves
        .sockets
        .push(socket("base", "leaf-base", [0, 0, 0], down(), "tree", true));
    ts.push(leaves);
    let mut ridge = piece(first + 17, "Roof ridge", "roof");
    volume(&mut ridge, [-1, 0, -32], [1, 2, 32], ROOF, true);
    ridge.sockets.push(socket(
        "base",
        "ridge-base",
        [0, 0, 0],
        down(),
        "ridge-64",
        true,
    ));
    ts.push(ridge);
    let pairs = [
        ("ridge-mount", "ridge-base"),
        ("road", "road"),
        ("land", "land"),
        ("land", "side-land"),
        ("side-land", "side-land"),
        ("frontage", "side-land"),
        ("plot-house", "house-base"),
        ("floor-door", "door-base"),
        ("floor-wall", "wall-base"),
        ("floor-side", "side-base"),
        ("wall-slope", "roof-slope"),
        ("wall-gable", "roof-gable"),
        ("path", "path"),
        ("door", "path"),
        ("sidewalk", "path"),
        ("plant", "trunk-base"),
        ("trunk-top", "trunk-base"),
        ("trunk-top", "leaf-base"),
        ("branch-mount", "branch-base"),
    ];
    (
        ts,
        SocketRules {
            pairs: pairs
                .into_iter()
                .map(|(a, b)| [a.into(), b.into()])
                .collect(),
        },
    )
}
#[derive(Clone, Debug)]
pub struct GenerateOptions {
    pub seed: u64,
    pub width: usize,
    pub depth: usize,
    pub houses: usize,
    pub plant_percent: u8,
}
impl Default for GenerateOptions {
    fn default() -> Self {
        Self {
            seed: 7,
            width: 5,
            depth: 5,
            houses: 3,
            plant_percent: 45,
        }
    }
}
struct Random(u64);
impl Random {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e3779b97f4a7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d049bb133111eb);
        z ^ (z >> 31)
    }
}
fn named<'a>(tiles: &'a [Tile], name: &str) -> Result<&'a Tile, String> {
    tiles
        .iter()
        .find(|t| t.name == name)
        .ok_or_else(|| format!("Starter library missing {name}"))
}
fn attach(
    w: &mut World,
    ts: &[Tile],
    rules: &SocketRules,
    p: Port,
    name: &str,
) -> Result<u32, String> {
    let t = named(ts, name)?;
    let mut reason = String::new();
    for (s, _) in t.sockets.iter().enumerate() {
        for r in &t.rotations {
            let i = aligned(w, ts, p, t, s, *r)?;
            if !compatible(port(w, ts, p)?, (&i, &t.sockets[s]), rules) {
                continue;
            }
            let id = i.id;
            let mut trial = w.clone();
            let result = connect_instance(&mut trial, ts, rules, i);
            if let Err(ref err) = result {
                reason = err.clone();
            }
            if result.is_ok() {
                *w = trial;
                return Ok(id);
            }
        }
    }
    Err(format!(
        "Cannot attach {name} to {}:{}: {reason}",
        p.instance, p.socket
    ))
}
fn find_port(w: &World, ts: &[Tile], id: u32, name: &str) -> Result<Port, String> {
    let i = w
        .instances
        .iter()
        .find(|i| i.id == id)
        .ok_or("Missing instance")?;
    let socket = tile(ts, i.tile)?
        .sockets
        .iter()
        .position(|s| s.name == name)
        .ok_or("Missing named socket")?;
    Ok(Port {
        instance: id,
        socket,
    })
}
fn house(w: &mut World, ts: &[Tile], rules: &SocketRules, plot: u32) -> Result<(), String> {
    let base = attach(
        w,
        ts,
        rules,
        find_port(w, ts, plot, "house")?,
        "House floor",
    )?;
    attach(w, ts, rules, find_port(w, ts, base, "ridge")?, "Roof ridge")?;
    for n in 0..4 {
        let wall = attach(
            w,
            ts,
            rules,
            find_port(w, ts, base, &format!("wall-{n}"))?,
            if n == 0 {
                "Front door"
            } else if n % 2 == 1 {
                "Side wall"
            } else {
                "Wall window"
            },
        )?;
        attach(
            w,
            ts,
            rules,
            find_port(w, ts, wall, "roof")?,
            if n % 2 == 1 {
                "Roof slope"
            } else {
                "Roof gable"
            },
        )?;
        if n == 0 {
            let mut p = find_port(w, ts, wall, "entrance")?;
            for _ in 0..4 {
                let path = attach(w, ts, rules, p, "Walkway straight")?;
                let i = w.instances.iter().find(|i| i.id == path).unwrap();
                p = (0..tile(ts, i.tile)?.sockets.len())
                    .map(|socket| Port {
                        instance: path,
                        socket,
                    })
                    .find(|p| !w.connected(*p))
                    .unwrap_or(p);
            }
        }
    }
    Ok(())
}
/// A bounded backtracking road solver. Fixed perimeter and central streets guarantee a connected demonstration network;
/// compatible rotated candidates are selected by constraints, with explicit boundary exits.
pub fn generate(
    ts: &[Tile],
    rules: &SocketRules,
    id: u32,
    o: &GenerateOptions,
) -> Result<World, String> {
    if !(5..=11).contains(&o.width) || !(5..=11).contains(&o.depth) || o.plant_percent > 100 {
        return Err("World dimensions must be 5..11; plant percentage 0..100".into());
    }
    let mut rng = Random(o.seed);
    let mut w = World::empty(id);
    w.name = "Neighbourhood".into();
    w.seed = o.seed;
    w.bounds = Bounds {
        min: [-6.4, -2., -6.4],
        max: [
            o.width as f32 * 12.8 - 6.4,
            20.,
            o.depth as f32 * 12.8 - 6.4,
        ],
    };
    w.spawn = [12.8, 0., 12.8];
    let mut masks = BTreeMap::new();
    let road = |x: usize, z: usize| {
        x == 1 || x == o.width - 2 || z == 1 || z == o.depth - 2 || x == o.width / 2
    };
    for z in 0..o.depth {
        for x in 0..o.width {
            let mut mask = 0u8;
            if road(x, z) {
                for (n, (dx, dz)) in [(0, -1), (1, 0), (0, 1), (-1, 0)].into_iter().enumerate() {
                    let nx = x as i32 + dx;
                    let nz = z as i32 + dz;
                    if nx < 0
                        || nz < 0
                        || nx >= o.width as i32
                        || nz >= o.depth as i32
                        || road(nx as usize, nz as usize)
                    {
                        mask |= 1 << n;
                    }
                }
            }
            masks.insert((x, z), mask);
        }
    }
    // Replace peripheral through streets by closed corners along an inset rectangular loop.
    for z in 0..o.depth {
        for x in 0..o.width {
            if x == 0 || z == 0 || x == o.width - 1 || z == o.depth - 1 {
                masks.insert((x, z), 0);
            }
        }
    }
    for z in 1..o.depth - 1 {
        for x in 1..o.width - 1 {
            let mut mask = 0;
            if road(x, z) {
                for (n, (dx, dz)) in [(0, -1), (1, 0), (0, 1), (-1, 0)].into_iter().enumerate() {
                    let nx = (x as i32 + dx) as usize;
                    let nz = (z as i32 + dz) as usize;
                    if masks[&(nx, nz)] != 0 {
                        mask |= 1 << n;
                    }
                }
            }
            masks.insert((x, z), mask);
        }
    }
    // One central cross street supplies four-way intersections when width permits.
    if o.depth >= 7 {
        let z = o.depth / 2;
        for x in 1..o.width - 1 {
            masks.insert((x, z), 10);
        }
        for zz in 1..o.depth - 1 {
            for x in 1..o.width - 1 {
                if masks[&(x, zz)] == 0 {
                    continue;
                }
                let mut m = 0;
                for (n, (dx, dz)) in [(0, -1), (1, 0), (0, 1), (-1, 0)].into_iter().enumerate() {
                    if masks[&((x as i32 + dx) as usize, (zz as i32 + dz) as usize)] != 0 {
                        m |= 1 << n;
                    }
                }
                masks.insert((x, zz), m);
            }
        }
    }
    let mut slots = vec![];
    for (&(x, z), &mask) in &masks {
        let mut candidates = vec![];
        for t in ts.iter().filter(|t| {
            t.category == "terrain"
                && !t.voxels.is_empty()
                && t.sockets
                    .iter()
                    .filter(|s| s.profile == "land-128" || s.profile == "road-128")
                    .count()
                    == 4
        }) {
            for r in &t.rotations {
                let i = Instance {
                    id: 0,
                    tile: t.id,
                    position: [x as f32 * 12.8, 0., z as f32 * 12.8],
                    rotation: *r,
                };
                let mut found = 0;
                for s in &t.sockets {
                    if s.kind == "road" {
                        let (_, r) = frame(&i, s);
                        let d = r * Vec3::Z;
                        let n = if d.z < -0.9 {
                            0
                        } else if d.x > 0.9 {
                            1
                        } else if d.z > 0.9 {
                            2
                        } else {
                            3
                        };
                        found |= 1 << n;
                    }
                }
                if found == mask {
                    candidates.push(i);
                }
            }
        }
        if candidates.is_empty() {
            return Err(format!("No terrain candidate for road mask {mask}"));
        }
        let shift = rng.next() as usize % candidates.len();
        candidates.rotate_left(shift);
        slots.push(candidates);
    }
    fn solve(
        w: &mut World,
        ts: &[Tile],
        rules: &SocketRules,
        slots: &[Vec<Instance>],
        at: usize,
        budget: &mut usize,
    ) -> bool {
        if at == slots.len() {
            return true;
        }
        if *budget == 0 {
            return false;
        }
        *budget -= 1;
        for candidate in &slots[at] {
            let mut trial = w.clone();
            let mut i = candidate.clone();
            i.id = trial.next_id();
            if connect_instance(&mut trial, ts, rules, i).is_ok()
                && solve(&mut trial, ts, rules, slots, at + 1, budget)
            {
                *w = trial;
                return true;
            }
        }
        false
    }
    if !solve(&mut w, ts, rules, &slots, 0, &mut 10000) {
        return Err("Terrain constraints exhausted search budget".into());
    }
    let mut plots: Vec<_> = w
        .instances
        .iter()
        .filter(|i| tile(ts, i.tile).is_ok_and(|t| t.name == "Grass"))
        .map(|i| i.id)
        .collect();
    let shift = rng.next() as usize % plots.len();
    plots.rotate_left(shift);
    let mut houses = 0;
    for id in plots {
        if houses == o.houses {
            break;
        }
        let original = w.instances.iter().find(|i| i.id == id).unwrap().clone();
        for r in &named(ts, "House plot")?.rotations {
            let mut trial = w.clone();
            trial.remove(id);
            let mut plot = original.clone();
            plot.tile = named(ts, "House plot")?.id;
            plot.rotation = *r;
            if connect_instance(&mut trial, ts, rules, plot).is_err() {
                continue;
            }
            let p = find_port(&trial, ts, id, "edge-0")?;
            if !trial.connected(p) {
                continue;
            }
            let result = house(&mut trial, ts, rules, id);

            if result.is_ok() {
                w = trial;
                houses += 1;
                break;
            }
        }
    }
    if houses != o.houses {
        return Err(format!(
            "Could fit {houses} houses with complete sidewalk paths; requested {}",
            o.houses
        ));
    }
    let plants: Vec<_> = w
        .instances
        .iter()
        .flat_map(|i| {
            tile(ts, i.tile)
                .unwrap()
                .sockets
                .iter()
                .enumerate()
                .filter(|(_, s)| s.kind == "plant")
                .map(move |(socket, _)| Port {
                    instance: i.id,
                    socket,
                })
        })
        .collect();
    for p in plants {
        if rng.next() % 100 >= u64::from(o.plant_percent) {
            continue;
        }
        let mut trial = w.clone();
        let mut top = p;
        let result = (|| -> Result<(), String> {
            for level in 0..2 + rng.next() % 3 {
                let trunk = attach(&mut trial, ts, rules, top, "Tree trunk")?;
                if level > 0 && rng.next().is_multiple_of(2) {
                    let branch_port = find_port(&trial, ts, trunk, "branch")?;
                    let branch = attach(&mut trial, ts, rules, branch_port, "Tree branch")?;
                    let tip = find_port(&trial, ts, branch, "tip")?;
                    attach(&mut trial, ts, rules, tip, "Tree foliage")?;
                }
                top = find_port(&trial, ts, trunk, "top")?;
            }
            attach(&mut trial, ts, rules, top, "Tree foliage")?;
            Ok(())
        })();
        if result.is_ok() {
            w = trial;
        }
    }
    if let Some(door) = w
        .instances
        .iter()
        .find(|i| tile(ts, i.tile).is_ok_and(|t| t.category == "door"))
    {
        let s = tile(ts, door.tile)?
            .sockets
            .iter()
            .find(|s| s.kind == "door")
            .ok_or("Door has no entrance")?;
        let (p, r) = frame(door, s);
        let outward = r * Vec3::Z;
        w.spawn = (p + outward * 10.2).to_array();
        w.spawn[1] = 0.;
        w.spawn_yaw = outward.x.atan2(outward.z);
    }
    validate_world(&w, ts, rules, true)?;
    Ok(w)
}
pub fn install(l: &mut Library) -> Result<(), String> {
    if !l.tiles.is_empty() {
        return Err(
            "Workspace already has tiles; use world generation without replacing assets".into(),
        );
    }
    let (ts, rules) = starter_tiles(l.next_id());
    l.tiles = ts;
    l.socket_rules = rules;
    let options = GenerateOptions {
        width: 7,
        depth: 7,
        ..Default::default()
    };
    l.worlds
        .push(generate(&l.tiles, &l.socket_rules, l.next_id(), &options)?);
    l.validate()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn starter_world_contains_real_assets_and_complete_routes() {
        let (ts, rules) = starter_tiles(100);
        validate_tiles(&ts, &rules).unwrap();
        for t in &ts {
            assert!(!t.voxels.is_empty(), "{}", t.name);
        }
        let o = GenerateOptions {
            width: 7,
            depth: 7,
            ..Default::default()
        };
        let w = generate(&ts, &rules, 200, &o).unwrap();
        assert_eq!(w, generate(&ts, &rules, 200, &o).unwrap());
        for name in [
            "Grass",
            "House plot",
            "Road straight",
            "Road corner",
            "Road T",
            "Road crossing",
            "Front door",
            "Roof slope",
            "Roof gable",
            "Roof ridge",
            "Tree trunk",
            "Tree branch",
            "Tree foliage",
        ] {
            assert!(
                w.instances
                    .iter()
                    .any(|i| tile(&ts, i.tile).unwrap().name == name),
                "{name}"
            );
        }
        assert_eq!(
            w.instances
                .iter()
                .filter(|i| tile(&ts, i.tile).unwrap().category == "door")
                .count(),
            o.houses
        );
        let path = w
            .instances
            .iter()
            .find(|i| tile(&ts, i.tile).unwrap().category == "walkway")
            .unwrap()
            .id;
        let mut broken = w.clone();
        broken.remove(path);
        assert!(validate_world(&broken, &ts, &rules, true).is_err());
        let mut broken = w.clone();
        broken.connections[0].b = broken.connections[0].a;
        assert!(validate_world(&broken, &ts, &rules, false).is_err());
        let mut overlapping = w.clone();
        let mut copy = w.instances[0].clone();
        copy.id = w.next_id();
        overlapping.instances.push(copy);
        assert!(validate_world(&overlapping, &ts, &rules, true).is_err());
        let mut impossible = o.clone();
        impossible.houses = 100;
        assert!(generate(&ts, &rules, 200, &impossible).is_err());
    }
    #[test]
    fn different_seeds_and_sizes_remain_valid() {
        let (ts, rules) = starter_tiles(100);
        for (seed, size) in [(0, 5), (23, 6), (81, 8)] {
            let options = GenerateOptions {
                seed,
                width: size,
                depth: size,
                houses: 2,
                plant_percent: 30,
            };
            let w = generate(&ts, &rules, 200, &options).unwrap();
            validate_world(&w, &ts, &rules, true).unwrap();
        }
    }
    #[test]
    fn rotated_corner_connections_and_mismatched_profiles() {
        let (ts, rules) = starter_tiles(100);
        let road = named(&ts, "Road straight").unwrap();
        let corner = named(&ts, "Road corner").unwrap();
        for turn in 0..4 {
            let mut w = World::empty(200);
            let i = Instance {
                id: 1,
                tile: road.id,
                position: [0.; 3],
                rotation: yaw(turn),
            };
            connect_instance(&mut w, &ts, &rules, i).unwrap();
            let p = find_port(&w, &ts, 1, "edge-0").unwrap();
            let id = attach(&mut w, &ts, &rules, p, "Road corner").unwrap();
            assert!(w.connected(p));
            assert_eq!(
                w.instances.iter().find(|i| i.id == id).unwrap().tile,
                corner.id
            );
        }
        let mut other = road
            .sockets
            .iter()
            .find(|s| s.kind == "road")
            .unwrap()
            .clone();
        let a = other.clone();
        other.rotation =
            (Quat::from_array(a.rotation) * Quat::from_rotation_y(std::f32::consts::PI)).to_array();
        let i = Instance {
            id: 1,
            tile: road.id,
            position: [0.; 3],
            rotation: identity(),
        };
        assert!(compatible((&i, &a), (&i, &other), &rules));
        other.profile = "road-64".into();
        assert!(!compatible((&i, &a), (&i, &other), &rules));
    }
    #[test]
    fn complete_world_and_tile_documents_round_trip() {
        let mut l = Library::default();
        install(&mut l).unwrap();
        let dir = std::env::temp_dir().join(format!("tile-roundtrip-{}", std::process::id()));
        crate::storage::save_workspace(&dir, &l).unwrap();
        let loaded = crate::storage::load_workspace(&dir).unwrap();
        assert_eq!(loaded.tiles.len(), l.tiles.len());
        assert_eq!(loaded.worlds, l.worlds);
        assert_eq!(loaded.socket_rules, l.socket_rules);
        std::fs::remove_dir_all(dir).unwrap();
        let legacy: Library =
            serde_json::from_str(r#"{"version":1,"bodies":[],"props":[],"characters":[]}"#)
                .unwrap();
        assert!(legacy.tiles.is_empty());
        legacy.validate().unwrap();
    }
}

#[cfg(test)]
mod graph_regressions {
    use super::*;
    #[test]
    fn rejects_closed_path_loops_and_sidewalk_to_sidewalk_routes() {
        let (mut ts, rules) = starter_tiles(100);
        let mut w = World::empty(200);
        let corner = named(&ts, "Walkway corner").unwrap();
        connect_instance(
            &mut w,
            &ts,
            &rules,
            Instance {
                id: 1,
                tile: corner.id,
                position: [0., 0.2, 0.],
                rotation: identity(),
            },
        )
        .unwrap();
        let mut last = 1;
        for _ in 0..3 {
            let p = find_port(&w, &ts, last, "end").unwrap();
            last = attach(&mut w, &ts, &rules, p, "Walkway corner").unwrap();
        }
        assert_eq!(w.connections.len(), 4);
        assert!(
            validate_world(&w, &ts, &rules, true)
                .unwrap_err()
                .contains("one door")
        );
        let world = generate(
            &ts,
            &rules,
            200,
            &GenerateOptions {
                width: 7,
                depth: 7,
                plant_percent: 0,
                ..Default::default()
            },
        )
        .unwrap();
        let door = ts.iter_mut().find(|t| t.category == "door").unwrap();
        door.category = "terrain".into();
        door.sockets
            .iter_mut()
            .find(|s| s.kind == "door")
            .unwrap()
            .kind = "sidewalk".into();
        assert!(
            validate_world(&world, &ts, &rules, true)
                .unwrap_err()
                .contains("one door")
        );
    }
    #[test]
    fn tilted_boxes_do_not_use_their_bounding_boxes_as_solid_geometry() {
        let b = Bounds {
            min: [-2., -0.1, -0.1],
            max: [2., 0.1, 0.1],
        };
        let rotation = Quat::from_rotation_y(std::f32::consts::FRAC_PI_4).to_array();
        let a = Instance {
            id: 1,
            tile: 1,
            position: [0.; 3],
            rotation,
        };
        let c = Instance {
            id: 2,
            tile: 1,
            position: [0.5, 0., 0.5],
            rotation,
        };
        assert!(intersects(&world_bounds(&a, &b), &world_bounds(&c, &b)));
        assert!(!OrientedBox::new(&a, &b).overlaps(&OrientedBox::new(&c, &b)));
    }
}

#[cfg(test)]
mod traversal_regression {
    use super::*;
    #[test]
    fn sample_character_fits_the_generated_entrance_and_stops_at_back_wall() {
        let library = crate::storage::load_workspace(std::path::Path::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/test_workspace"
        )))
        .unwrap();
        let (lo, hi) = library.bodies[0].bounds().unwrap();
        let (ts, rules) = starter_tiles(100);
        let w = generate(
            &ts,
            &rules,
            200,
            &GenerateOptions {
                width: 7,
                depth: 7,
                plant_percent: 0,
                ..Default::default()
            },
        )
        .unwrap();
        let terrain = crate::terrain_collision::TerrainCollision::from_world(&w, &ts).unwrap();
        let start = Vec3::from_array(w.spawn);
        let mut player = crate::controller::PlayerController {
            position: start,
            ..default()
        };
        let input = crate::controller::PlayerInput {
            movement: Vec2::Y,
            speed: 2.6,
            ..default()
        };
        for _ in 0..500 {
            crate::terrain_collision::step_on_terrain(
                &mut player,
                &input,
                w.spawn_yaw,
                1. / 60.,
                &terrain,
                hi.y - lo.y,
            );
        }
        let forward = Quat::from_rotation_y(w.spawn_yaw) * -Vec3::Z;
        let progress = (player.position - start).dot(forward);
        assert!(
            progress > 14. && progress < 16.4,
            "Entrance/wall traversal stopped at {progress}, avatar height {}",
            hi.y - lo.y
        );
        assert!((player.position.y - 0.2).abs() < 0.02);
    }
}

#[cfg(test)]
mod roof_regression {
    use super::*;
    #[test]
    fn gables_meet_the_underside_of_both_roof_slopes_without_voxel_gaps() {
        let (ts, rules) = starter_tiles(100);
        let w = generate(
            &ts,
            &rules,
            200,
            &GenerateOptions {
                width: 7,
                depth: 7,
                plant_percent: 0,
                ..Default::default()
            },
        )
        .unwrap();
        let base = w
            .instances
            .iter()
            .find(|i| tile(&ts, i.tile).unwrap().category == "floor")
            .unwrap();
        let collision = crate::terrain_collision::TerrainCollision::from_world(&w, &ts).unwrap();
        for x in -32..32 {
            for z in [-31.5, 31.5] {
                let column: Vec<_> = (44..62)
                    .map(|y| {
                        let p = base.transform().transform_point(Vec3::new(
                            (x as f32 + 0.5) * UNIT,
                            (y as f32 + 0.5) * UNIT,
                            z * UNIT,
                        ));
                        collision
                            .boxes
                            .iter()
                            .any(|b| (0..3).all(|a| p[a] > b.min[a] && p[a] < b.max[a]))
                    })
                    .collect();
                let top = column.iter().rposition(|v| *v).expect("Roof surface");
                assert!(
                    column[..=top].iter().all(|v| *v),
                    "Roof gap at x={x}, z={z}"
                );
            }
        }
    }
}
