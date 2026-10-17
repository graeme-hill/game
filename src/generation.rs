//! Starter kit installation and seeded neighbourhood generation.
//!
//! Generation plans a grid of 12.8-unit cells (hill, stream, roads, paths,
//! lots and parks), chooses for every cell a tile and quarter turn whose edge
//! sockets match the plan, and then dresses the world through decoration
//! sockets: buildings on lots, fences, street furniture and planting. Every
//! piece is placed through the ordinary socket/occupancy checks.
use crate::kit::{self, Rng};
use crate::model::Library;
use crate::tiles::{World, *};
use bevy::prelude::*;
use std::collections::{BTreeMap, BTreeSet};

pub use kit::CELL;
const CELL_SIZE: f32 = 12.8;

pub fn starter_tiles(first: u32) -> (Vec<Tile>, SocketRules) {
    let mut ts = kit::tiles();
    for (n, t) in ts.iter_mut().enumerate() {
        t.id = first + n as u32;
    }
    (ts, kit::rules())
}

#[derive(Clone, Debug)]
pub struct GenerateOptions {
    pub seed: u64,
    pub width: usize,
    pub depth: usize,
    /// Number of building lots (houses, shops and the odd chapel).
    pub houses: usize,
    /// Planting and street-furniture density.
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
/// The sample workspace's neighbourhood.
pub fn showcase() -> GenerateOptions {
    GenerateOptions {
        seed: 12,
        width: 9,
        depth: 9,
        houses: 16,
        plant_percent: 80,
    }
}

const SIDES: [(i32, i32); 4] = [(0, -1), (1, 0), (0, 1), (-1, 0)];

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Road {
    Asphalt,
    Lane,
}
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Hill {
    Top,
    Spring,
    Brook,
    Edge,
    Stairs,
    Ramp,
    Waterfall,
    Corner,
}
#[derive(Clone, PartialEq, Eq, Debug)]
enum Use {
    Free,
    Hill(Hill),
    Road,
    Stream,
    Path,
    Lot { front: usize, shop: bool },
    Park(&'static str),
}

/// The cell plan before any tile is chosen.
struct Plan {
    w: usize,
    d: usize,
    high: Vec<bool>,
    road: Vec<Option<Road>>,
    stream: Vec<bool>,
    uses: Vec<Use>,
    /// Unordered neighbour pairs joined by water or footpath.
    water: BTreeSet<(usize, usize)>,
    paths: BTreeSet<(usize, usize)>,
}
impl Plan {
    fn new(w: usize, d: usize) -> Self {
        Self {
            w,
            d,
            high: vec![false; w * d],
            road: vec![None; w * d],
            stream: vec![false; w * d],
            uses: vec![Use::Free; w * d],
            water: BTreeSet::new(),
            paths: BTreeSet::new(),
        }
    }
    fn at(&self, x: usize, z: usize) -> usize {
        z * self.w + x
    }
    fn xz(&self, i: usize) -> (usize, usize) {
        (i % self.w, i / self.w)
    }
    fn neighbour(&self, i: usize, side: usize) -> Option<usize> {
        let (x, z) = self.xz(i);
        let (dx, dz) = SIDES[side];
        let (nx, nz) = (x as i32 + dx, z as i32 + dz);
        (nx >= 0 && nz >= 0 && (nx as usize) < self.w && (nz as usize) < self.d)
            .then(|| self.at(nx as usize, nz as usize))
    }
    fn link(set: &mut BTreeSet<(usize, usize)>, a: usize, b: usize) {
        set.insert((a.min(b), a.max(b)));
    }
    fn linked(set: &BTreeSet<(usize, usize)>, a: usize, b: usize) -> bool {
        set.contains(&(a.min(b), a.max(b)))
    }
    fn free(&self, i: usize) -> bool {
        self.uses[i] == Use::Free
    }
    /// A low side faces a lower neighbour: the cliff drops on that side.
    fn cliff(&self, i: usize, side: usize) -> bool {
        self.high[i] && self.neighbour(i, side).is_some_and(|n| !self.high[n])
    }
    /// The edge kind the tile in cell `i` must present on world side `side`
    /// ("*" on the world boundary).
    fn edge(&self, i: usize, side: usize) -> &'static str {
        let Some(n) = self.neighbour(i, side) else {
            return "*";
        };
        if self.high[i] {
            if self.high[n] {
                if Plan::linked(&self.water, i, n) {
                    return "brook";
                }
                let left = self.cliff(i, (side + 3) % 4);
                let right = self.cliff(i, (side + 1) % 4);
                return match (left, right) {
                    (false, false) => "high",
                    (true, false) => "cliff-l",
                    (false, true) => "cliff-r",
                    _ => "high-lr",
                };
            }
            if Plan::linked(&self.water, i, n) {
                return "stream";
            }
            if Plan::linked(&self.paths, i, n) {
                return "path";
            }
            return "ground";
        }
        if Plan::linked(&self.water, i, n) {
            return "stream";
        }
        if let Some(r) = self.road[i] {
            return match self.road[n] {
                Some(o) if r == Road::Lane || o == Road::Lane => "lane",
                Some(_) => "road",
                None => "curb",
            };
        }
        if Plan::linked(&self.paths, i, n) {
            return "path";
        }
        if let Use::Lot { front, .. } = self.uses[i]
            && front == side
        {
            return "frontage";
        }
        "ground"
    }
    fn signature(&self, i: usize) -> [&'static str; 4] {
        std::array::from_fn(|s| self.edge(i, s))
    }
    /// Mirror the plan so the hill sits in another corner.
    fn mirrored(self, flip_x: bool, flip_z: bool) -> Self {
        let (w, d) = (self.w, self.d);
        let map = |i: usize| {
            let (x, z) = (i % w, i / w);
            let x = if flip_x { w - 1 - x } else { x };
            let z = if flip_z { d - 1 - z } else { z };
            z * w + x
        };
        let mut out = Plan::new(w, d);
        for i in 0..w * d {
            let j = map(i);
            out.high[j] = self.high[i];
            out.road[j] = self.road[i];
            out.stream[j] = self.stream[i];
            out.uses[j] = match &self.uses[i] {
                Use::Lot { front, shop } => {
                    let mut f = *front;
                    if flip_x && f % 2 == 1 {
                        f = 4 - f;
                    }
                    if flip_z && f % 2 == 0 {
                        f = 2 - f;
                    }
                    Use::Lot {
                        front: f,
                        shop: *shop,
                    }
                }
                u => u.clone(),
            };
        }
        for &(a, b) in &self.water {
            Plan::link(&mut out.water, map(a), map(b));
        }
        for &(a, b) in &self.paths {
            Plan::link(&mut out.paths, map(a), map(b));
        }
        out
    }
}

/// Plan the neighbourhood in a frame with the hill at the -X/-Z corner.
fn plan(o: &GenerateOptions, rng: &mut Rng) -> Result<Plan, String> {
    let (w, d) = (o.width, o.depth);
    let mut p = Plan::new(w, d);
    let pw = w.div_ceil(3).clamp(2, 4);
    let pd = d.div_ceil(3).clamp(2, 4);
    // Hill: spring and brook down column 0 to a waterfall, stairs on the
    // east cliff, a ramp on the south cliff when wide enough.
    for z in 0..pd {
        for x in 0..pw {
            let i = p.at(x, z);
            p.high[i] = true;
            let east = x == pw - 1;
            let south = z == pd - 1;
            p.uses[i] = Use::Hill(match (east, south) {
                (true, true) => Hill::Corner,
                (true, false) | (false, true) => Hill::Edge,
                _ => Hill::Top,
            });
        }
    }
    for z in 0..pd {
        let i = p.at(0, z);
        p.uses[i] = Use::Hill(if z == 0 {
            Hill::Spring
        } else if z == pd - 1 {
            Hill::Waterfall
        } else {
            Hill::Brook
        });
        if z > 0 {
            let up = p.at(0, z - 1);
            Plan::link(&mut p.water, up, i);
        }
    }
    let stairs = p.at(pw - 1, 0);
    p.uses[stairs] = Use::Hill(Hill::Stairs);
    let ramp = if pw >= 3 {
        Some((p.at(1, pd - 1), 2))
    } else if pd >= 3 {
        Some((p.at(pw - 1, 1), 1))
    } else {
        None
    };
    if let Some((r, _)) = ramp {
        p.uses[r] = Use::Hill(Hill::Ramp);
    }
    // The stairs (and an east-facing ramp) land straight on the first street.
    let foot = p.at(pw, 0);
    Plan::link(&mut p.paths, stairs, foot);
    if let Some((r, 1)) = ramp {
        let (x, z) = p.xz(r);
        let foot = p.at(x + 1, z);
        Plan::link(&mut p.paths, r, foot);
    }
    // Road grid: columns east of the hill, rows south of it.
    let columns: Vec<usize> = (pw..w).step_by(3).collect();
    let rows: Vec<usize> = (pd + 1..d).step_by(3).collect();
    let lane_row = (rows.len() >= 2 && columns.len() >= 2 && rng.chance(75))
        .then(|| rows[1 + (rng.next_u64() % (rows.len() as u64 - 1)) as usize]);
    for z in 0..d {
        for &x in &columns {
            let i = p.at(x, z);
            p.road[i] = Some(Road::Asphalt);
        }
    }
    for &z in &rows {
        for x in 0..w {
            let i = p.at(x, z);
            if p.high[i] {
                continue;
            }
            if Some(z) == lane_row {
                if x > columns[0] && x < *columns.last().unwrap() && p.road[i].is_none() {
                    p.road[i] = Some(Road::Lane);
                }
            } else if p.road[i].is_none() {
                p.road[i] = Some(Road::Asphalt);
            }
        }
    }
    for i in 0..w * d {
        if p.road[i].is_some() {
            p.uses[i] = Use::Road;
        }
    }
    // Stream: from the waterfall south, optionally stepping one column east.
    let mut bends = (pd + 1..d.saturating_sub(1))
        .filter(|z| !rows.contains(z) && !rows.contains(&(z + 1)))
        .collect::<Vec<_>>();
    let bend = if !bends.is_empty() && rng.chance(60) {
        rng.shuffle(&mut bends);
        Some(bends[0])
    } else {
        None
    };
    let mut prev = p.at(0, pd - 1);
    let mut column = 0;
    for z in pd..d {
        let i = p.at(column, z);
        p.stream[i] = true;
        Plan::link(&mut p.water, prev, i);
        if p.road[i].is_none() {
            p.uses[i] = Use::Stream;
        }
        prev = i;
        if Some(z) == bend {
            let e = p.at(1, z);
            p.stream[e] = true;
            p.uses[e] = Use::Stream;
            Plan::link(&mut p.water, i, e);
            prev = e;
            column = 1;
        }
    }
    let stream_col = |z: usize| match bend {
        Some(b) if z > b => 1,
        _ => 0,
    };
    // Footpaths: the ramp down to the street, and a path across the stream.
    if let Some((r, side)) = ramp
        && side == 2
    {
        let (x, z) = p.xz(r);
        let mut prev = r;
        for zz in z + 1..d {
            let i = p.at(x, zz);
            if p.road[i].is_some() {
                Plan::link(&mut p.paths, prev, i);
                break;
            }
            if !p.free(i) {
                return Err("Ramp path is blocked".into());
            }
            p.uses[i] = Use::Path;
            Plan::link(&mut p.paths, prev, i);
            prev = i;
        }
    }
    let straight = |p: &Plan, i: usize| {
        p.uses[i] == Use::Stream
            && p.neighbour(i, 0)
                .is_some_and(|n| Plan::linked(&p.water, i, n))
            && p.neighbour(i, 2)
                .is_none_or(|n| Plan::linked(&p.water, i, n))
    };
    let crossing_rows = (pd + 1..d)
        .filter(|&z| {
            let sc = stream_col(z);
            !rows.contains(&z)
                && Some(z) != bend
                && straight(&p, p.at(sc, z))
                && (0..columns[0]).all(|x| x == sc || p.free(p.at(x, z)))
        })
        .collect::<Vec<_>>();
    if !crossing_rows.is_empty() {
        let z = crossing_rows[(rng.next_u64() % crossing_rows.len() as u64) as usize];
        let sc = stream_col(z);
        let mut prev = p.at(columns[0], z);
        for x in (0..columns[0]).rev() {
            let i = p.at(x, z);
            if x != sc {
                p.uses[i] = Use::Path;
            }
            Plan::link(&mut p.paths, prev, i);
            prev = i;
        }
    }
    // Lots facing the streets; shops prefer the main street.
    let main = rows.first().copied();
    let mut candidates = vec![];
    for i in 0..w * d {
        if !p.free(i) || p.high[i] {
            continue;
        }
        let fronts: Vec<usize> = (0..4)
            .filter(|&s| {
                p.neighbour(i, s)
                    .is_some_and(|n| p.road[n].is_some() && !p.stream[n])
            })
            .collect();
        if fronts.is_empty() {
            continue;
        }
        let on_main = fronts.iter().any(|&s| {
            let n = p.neighbour(i, s).unwrap();
            Some(p.xz(n).1) == main && p.road[n] == Some(Road::Asphalt)
        });
        candidates.push((i, fronts, on_main));
    }
    rng.shuffle(&mut candidates);
    if o.houses > candidates.len() {
        return Err(format!(
            "Could fit {} building lots facing a street; requested {}",
            candidates.len(),
            o.houses
        ));
    }
    let shops_wanted = o.houses / 3;
    candidates.sort_by_key(|c| !c.2);
    let mut shops = 0;
    for (i, fronts, on_main) in candidates.into_iter().take(o.houses) {
        let shop = on_main && shops < shops_wanted;
        shops += usize::from(shop);
        // Face the main street when possible, otherwise any street.
        let front = fronts
            .iter()
            .copied()
            .find(|&s| {
                let n = p.neighbour(i, s).unwrap();
                Some(p.xz(n).1) == main
            })
            .unwrap_or(fronts[(rng.next_u64() % fronts.len() as u64) as usize]);
        p.uses[i] = Use::Lot { front, shop };
    }
    // Parks fill the rest, with a few one-off features.
    let mut parks = (0..w * d)
        .filter(|&i| p.free(i) && !p.high[i])
        .collect::<Vec<_>>();
    rng.shuffle(&mut parks);
    let mut specials = vec!["Pond", "Playground", "Community garden", "Fountain plaza"];
    if w * d >= 64 {
        specials.push("Pond");
    }
    rng.shuffle(&mut specials);
    for i in parks {
        let near_road = (0..4).any(|s| p.neighbour(i, s).is_some_and(|n| p.road[n].is_some()));
        let pick = specials
            .iter()
            .position(|s| *s != "Fountain plaza" || near_road)
            .filter(|_| rng.chance(70));
        p.uses[i] = Use::Park(match pick {
            Some(n) => specials.remove(n),
            None if rng.chance(35) => "Wildflower meadow",
            None => "Meadow",
        });
    }
    let flip_x = rng.chance(50);
    let flip_z = rng.chance(50);
    Ok(p.mirrored(flip_x, flip_z))
}

/// Which tiles may fill a planned cell, in order of preference.
fn cell_names(p: &Plan, i: usize, rng: &mut Rng) -> Vec<&'static str> {
    match &p.uses[i] {
        Use::Hill(h) => vec![match h {
            Hill::Top => "Hill top",
            Hill::Spring => "Hill spring",
            Hill::Brook => "Hill brook",
            Hill::Edge => "Cliff",
            Hill::Stairs => "Cliff stairs",
            Hill::Ramp => "Cliff ramp",
            Hill::Waterfall => "Waterfall",
            Hill::Corner => "Cliff corner",
        }],
        Use::Road if p.stream[i] => vec!["Road bridge"],
        Use::Road => {
            let mut v = vec![
                "Road straight",
                "Road corner",
                "Road T",
                "Road crossing",
                "Lane straight",
                "Lane corner",
                "Lane T",
                "Road T lane",
                "Road lane crossing",
            ];
            if rng.chance(30) {
                v.insert(0, "Road crosswalk");
            }
            v
        }
        Use::Stream if p.paths.iter().any(|&(a, b)| a == i || b == i) => vec!["Stream crossing"],
        Use::Stream => vec!["Stream straight", "Stream bend"],
        Use::Path => vec!["Path straight", "Path corner", "Path T", "Path end"],
        Use::Lot { shop, .. } => vec![if *shop { "Shop lot" } else { "House lot" }],
        Use::Park(name) => vec![name],
        Use::Free => vec!["Meadow"],
    }
}

/// World-side edge kinds of a tile instance (side index by facing).
fn tile_edges(t: &Tile, rotation: [f32; 4]) -> [Option<&str>; 4] {
    let mut out = [None; 4];
    let i = Instance {
        id: 0,
        tile: t.id,
        position: [0.; 3],
        rotation,
    };
    for s in &t.sockets {
        if !s.name.starts_with("edge-") {
            continue;
        }
        let (_, r) = frame(&i, s);
        let d = r * Vec3::Z;
        let side = if d.z < -0.9 {
            0
        } else if d.x > 0.9 {
            1
        } else if d.z > 0.9 {
            2
        } else {
            3
        };
        out[side] = Some(s.kind.as_str());
    }
    out
}

struct Kit<'a> {
    tiles: &'a [Tile],
    rules: &'a SocketRules,
    by_name: BTreeMap<&'a str, &'a Tile>,
    cache: Extents,
}
impl<'a> Kit<'a> {
    fn new(tiles: &'a [Tile], rules: &'a SocketRules) -> Self {
        Self {
            tiles,
            rules,
            by_name: tiles.iter().map(|t| (t.name.as_str(), t)).collect(),
            cache: Extents::default(),
        }
    }
    fn named(&self, name: &str) -> Result<&'a Tile, String> {
        self.by_name
            .get(name)
            .copied()
            .ok_or_else(|| format!("Starter library missing {name}"))
    }
    fn place(&mut self, w: &mut World, i: Instance) -> Result<(), String> {
        connect_with(w, self.tiles, self.rules, i, &mut self.cache)
    }
    /// Attach a named tile to an open socket, trying each compatible pose.
    fn attach(&mut self, w: &mut World, p: Port, name: &str) -> Result<u32, String> {
        let t = self.named(name)?;
        let target = port(w, self.tiles, p)?;
        let target = (target.0.clone(), target.1.clone());
        let mut reason = String::from("no compatible socket");
        for (s, socket) in t.sockets.iter().enumerate() {
            for r in &t.rotations {
                let i = aligned(w, self.tiles, p, t, s, *r)?;
                if !compatible((&target.0, &target.1), (&i, socket), self.rules) {
                    continue;
                }
                let id = i.id;
                match self.place(w, i) {
                    Ok(()) => return Ok(id),
                    Err(e) => reason = e,
                }
            }
        }
        Err(format!(
            "Cannot attach {name} to {}:{}: {reason}",
            p.instance, p.socket
        ))
    }
    /// Try names in order until one fits.
    fn attach_any(&mut self, w: &mut World, p: Port, names: &[&str]) -> Option<(u32, String)> {
        names
            .iter()
            .find_map(|n| self.attach(w, p, n).ok().map(|id| (id, n.to_string())))
    }
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
        .ok_or_else(|| format!("Missing socket {name}"))?;
    Ok(Port {
        instance: id,
        socket,
    })
}

/// Weighted random order of names.
fn weighted(rng: &mut Rng, items: &[(&'static str, u32)]) -> Vec<&'static str> {
    let mut pool = items.to_vec();
    let mut out = vec![];
    while !pool.is_empty() {
        let total: u32 = pool.iter().map(|i| i.1).sum();
        let mut roll = (rng.next_u64() % u64::from(total.max(1))) as u32;
        let n = pool
            .iter()
            .position(|i| {
                if roll < i.1 {
                    true
                } else {
                    roll -= i.1;
                    false
                }
            })
            .unwrap_or(0);
        out.push(pool.remove(n).0);
    }
    out
}

const PARK_PLANTS: &[(&str, u32)] = &[
    ("Oak tree", 6),
    ("Round tree", 6),
    ("Maple tree", 4),
    ("Golden tree", 2),
    ("Blossom tree", 3),
    ("Birch tree", 2),
    ("Poplar tree", 2),
    ("Pine tree", 2),
    ("Round bush", 4),
    ("Big bush", 2),
    ("Flowering bush", 3),
    ("Hydrangea", 2),
    ("Tulips", 2),
    ("Lavender", 2),
    ("Sunflowers", 1),
    ("Tall grass", 1),
    ("Mushrooms", 1),
    ("Boulder", 1),
    ("Rock cluster", 1),
    ("Tree stump", 1),
    ("Picnic table", 1),
    ("Bench", 1),
];
const HILL_PLANTS: &[(&str, u32)] = &[
    ("Pine tree", 6),
    ("Birch tree", 4),
    ("Oak tree", 3),
    ("Golden tree", 2),
    ("Maple tree", 2),
    ("Boulder", 3),
    ("Rock cluster", 2),
    ("Tree stump", 1),
    ("Mushrooms", 1),
    ("Tall grass", 2),
    ("Big bush", 1),
];
const BANK_PLANTS: &[(&str, u32)] = &[
    ("Birch tree", 3),
    ("Poplar tree", 2),
    ("Round bush", 3),
    ("Tall grass", 3),
    ("Boulder", 2),
    ("Stone", 2),
    ("Blossom tree", 1),
];
const FRONT_YARD: &[(&str, u32)] = &[
    ("Tulips", 3),
    ("Lavender", 2),
    ("Flower bed", 3),
    ("Flowering bush", 3),
    ("Round bush", 2),
    ("Hydrangea", 2),
    ("Sunflowers", 1),
];
const SIDE_YARD: &[(&str, u32)] = &[
    ("Round bush", 3),
    ("Big bush", 2),
    ("Flowering bush", 2),
    ("Hydrangea", 2),
    ("Poplar tree", 2),
    ("Birch tree", 1),
];
const BACK_YARD: &[(&str, u32)] = &[
    ("Apple tree", 4),
    ("Round tree", 3),
    ("Blossom tree", 3),
    ("Vegetable patch", 4),
    ("Garden shed", 2),
    ("Maple tree", 2),
    ("Picnic table", 1),
    ("Flowering bush", 1),
];

/// Choose a building for a lot and stack it to completion.
fn build(
    k: &mut Kit,
    w: &mut World,
    rng: &mut Rng,
    lot: u32,
    shop: bool,
    chapel: &mut bool,
) -> Result<(), String> {
    let site = find_port(w, k.tiles, lot, "building")?;
    if shop {
        if !*chapel && rng.chance(25) {
            k.attach(w, site, "Chapel")?;
            *chapel = true;
            return Ok(());
        }
        let ground = *rng.pick(&["Corner shop", "Cafe", "Florist", "Apartment lobby"]);
        let mut top = k.attach(w, site, ground)?;
        let style = *rng.pick(&["Brick storey", "Pink storey", "Cream storey", "Mint storey"]);
        for _ in 0..rng.range(1, 4) {
            let p = find_port(w, k.tiles, top, "top")?;
            top = k.attach(w, p, style)?;
        }
        let p = find_port(w, k.tiles, top, "top")?;
        k.attach(
            w,
            p,
            if rng.chance(35) {
                "Roof garden"
            } else {
                "Flat roof"
            },
        )?;
        // Cafe tables out front, planters and bins elsewhere.
        let lot_tile = tile(
            k.tiles,
            w.instances.iter().find(|i| i.id == lot).unwrap().tile,
        )?;
        let sockets: Vec<(usize, i32)> = lot_tile
            .sockets
            .iter()
            .enumerate()
            .filter(|(_, s)| s.kind == "street")
            .map(|(n, s)| (n, s.position[2]))
            .collect();
        for (n, z) in sockets {
            let p = Port {
                instance: lot,
                socket: n,
            };
            let names: &[&str] = if z < 0 {
                &["Litter bin"]
            } else if ground == "Cafe" {
                &["Cafe table"]
            } else {
                &["Planter", "Bench", "Litter bin"]
            };
            if rng.chance(75) {
                let mut names = names.to_vec();
                rng.shuffle(&mut names);
                k.attach_any(w, p, &names);
            }
        }
    } else {
        let colour = *rng.pick(&kit::HOUSE_COLOURS);
        let mut top = k.attach(w, site, &format!("House ground floor ({colour})"))?;
        if rng.chance(55) {
            let p = find_port(w, k.tiles, top, "top")?;
            top = k.attach(w, p, &format!("House upper floor ({colour})"))?;
        }
        let p = find_port(w, k.tiles, top, "top")?;
        let roof = *rng.pick(&[
            "Gable roof (red)",
            "Gable roof (slate)",
            "Hip roof (terracotta)",
            "Hip roof (green)",
        ]);
        k.attach(w, p, roof)?;
        if rng.chance(70) {
            let fence = *rng.pick(&[
                "Picket fence",
                "Wooden fence",
                "Low hedge",
                "Stone wall",
                "Iron railing",
            ]);
            for n in 0..6 {
                let p = find_port(w, k.tiles, lot, &format!("fence-{n}"))?;
                let _ = k.attach(w, p, fence);
            }
        }
    }
    Ok(())
}

pub fn generate(
    ts: &[Tile],
    rules: &SocketRules,
    id: u32,
    o: &GenerateOptions,
) -> Result<World, String> {
    if !(5..=11).contains(&o.width) || !(5..=11).contains(&o.depth) || o.plant_percent > 100 {
        return Err("World dimensions must be 5..11; plant percentage 0..100".into());
    }
    let mut rng = Rng::new(o.seed.wrapping_mul(0x2545_f491_4f6c_dd1d) ^ 0x5eed);
    let p = plan(o, &mut rng)?;
    let mut k = Kit::new(ts, rules);
    let mut w = World::empty(id);
    w.name = "Neighbourhood".into();
    w.seed = o.seed;
    let half = CELL_SIZE / 2.;
    w.bounds = Bounds {
        min: [-half, -2., -half],
        max: [
            o.width as f32 * CELL_SIZE - half,
            24.,
            o.depth as f32 * CELL_SIZE - half,
        ],
    };
    // Terrain: one matching tile per planned cell.
    let mut cells = vec![0u32; o.width * o.depth];
    for (i, cell) in cells.iter_mut().enumerate() {
        let (x, z) = p.xz(i);
        let want = p.signature(i);
        let mut placed = false;
        for name in cell_names(&p, i, &mut rng) {
            let t = k.named(name)?;
            let mut fits: Vec<[f32; 4]> = t
                .rotations
                .iter()
                .copied()
                .filter(|r| {
                    tile_edges(t, *r)
                        .iter()
                        .zip(want)
                        .all(|(have, want)| want == "*" || *have == Some(want))
                })
                .collect();
            if fits.is_empty() {
                continue;
            }
            rng.shuffle(&mut fits);
            let instance = Instance {
                id: w.next_id(),
                tile: t.id,
                position: [x as f32 * CELL_SIZE, 0., z as f32 * CELL_SIZE],
                rotation: fits[0],
            };
            *cell = instance.id;
            k.place(&mut w, instance)
                .map_err(|e| format!("Cell {x},{z} ({name}): {e}"))?;
            placed = true;
            break;
        }
        if !placed {
            return Err(format!("No tile fits cell {x},{z} with edges {want:?}"));
        }
    }
    // Required decorations: bridges at path crossings.
    for &id in &cells {
        if let Ok(p) = find_port(&w, ts, id, "crossing") {
            let names = if rng.chance(70) {
                ["Wooden footbridge", "Stepping stones"]
            } else {
                ["Stepping stones", "Wooden footbridge"]
            };
            k.attach_any(&mut w, p, &names)
                .ok_or("Cannot bridge the stream crossing")?;
        }
    }
    // Buildings.
    let mut chapel = false;
    let mut lots = vec![];
    for (i, &id) in cells.iter().enumerate() {
        if let Use::Lot { shop, .. } = p.uses[i] {
            lots.push((id, shop));
        }
    }
    let mut order = lots.clone();
    rng.shuffle(&mut order);
    for (id, shop) in order {
        build(&mut k, &mut w, &mut rng, id, shop, &mut chapel)?;
    }
    // Street furniture and planting, terrain cell by terrain cell.
    let density = u64::from(o.plant_percent);
    for &id in &cells {
        let inst = w.instances.iter().find(|i| i.id == id).unwrap().clone();
        let t = tile(ts, inst.tile)?;
        for (n, s) in t.sockets.iter().enumerate() {
            let port = Port {
                instance: id,
                socket: n,
            };
            if w.connected(port) {
                continue;
            }
            let names: Vec<&str> = match (s.kind.as_str(), t.category.as_str()) {
                ("street", "road") if s.name.ends_with('a') || t.name == "Road bridge" => {
                    if !rng.chance(80) {
                        continue;
                    }
                    vec!["Street lamp"]
                }
                ("street", "road") => {
                    if !rng.chance(density * 2 / 5) {
                        continue;
                    }
                    let mut items = vec![
                        ("Bench", 3),
                        ("Litter bin", 3),
                        ("Fire hydrant", 2),
                        ("Post box", 1),
                        ("Planter", 2),
                    ];
                    if t.name == "Road straight" {
                        items.push(("Bus stop", 1));
                    }
                    weighted(&mut rng, &items)
                }
                ("street", "park" | "path") => {
                    if !rng.chance(70) {
                        continue;
                    }
                    vec!["Bench"]
                }
                ("plant", "road") => {
                    if !rng.chance(density / 2) {
                        continue;
                    }
                    vec!["Street tree"]
                }
                ("plant", "hill") => {
                    if !rng.chance(density) {
                        continue;
                    }
                    weighted(&mut rng, HILL_PLANTS)
                }
                ("plant", "water") => {
                    if !rng.chance(density) {
                        continue;
                    }
                    weighted(&mut rng, BANK_PLANTS)
                }
                ("plant", "lot") if t.name == "Shop lot" => {
                    if !rng.chance(density) {
                        continue;
                    }
                    vec!["Hydrangea", "Round bush", "Poplar tree"]
                }
                ("plant", "lot") => {
                    if !rng.chance(density) {
                        continue;
                    }
                    let z = s.position[2];
                    weighted(
                        &mut rng,
                        if z > 40 {
                            FRONT_YARD
                        } else if z > -20 {
                            SIDE_YARD
                        } else {
                            BACK_YARD
                        },
                    )
                }
                ("plant", _) if t.name == "Fountain plaza" => {
                    vec!["Street tree", "Round tree", "Blossom tree"]
                }
                ("plant", _) => {
                    if !rng.chance(density) {
                        continue;
                    }
                    let mut items = PARK_PLANTS.to_vec();
                    if t.name == "Meadow" || t.name == "Wildflower meadow" {
                        items.push(("Gazebo", 1));
                    }
                    weighted(&mut rng, &items)
                }
                _ => continue,
            };
            let names: Vec<&str> = names.into_iter().take(5).collect();
            k.attach_any(&mut w, port, &names);
        }
    }
    // Roads leaving the map are explicit exits.
    for i in &w.instances {
        for (n, s) in tile(ts, i.tile)?.sockets.iter().enumerate() {
            let port = Port {
                instance: i.id,
                socket: n,
            };
            if s.kind == "road" && !w.connected(port) {
                w.exits.push(port);
            }
        }
    }
    // Spawn on the road in front of a house, facing its door.
    let spawn = lots
        .iter()
        .filter(|(_, shop)| !shop)
        .chain(lots.iter())
        .find_map(|(id, _)| {
            let lot = w.instances.iter().find(|i| i.id == *id)?;
            let out = Quat::from_array(lot.rotation) * Vec3::Z;
            let p = Vec3::from_array(lot.position) + out * (half + 4.2);
            Some((Vec3::new(p.x, 0., p.z), out))
        });
    if let Some((p, out)) = spawn {
        w.spawn = p.to_array();
        w.spawn_yaw = out.x.atan2(out.z);
    } else if let Some(i) = w
        .instances
        .iter()
        .find(|i| tile(ts, i.tile).is_ok_and(|t| t.category == "road"))
    {
        w.spawn = [i.position[0], 0., i.position[2]];
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
    l.worlds.push(generate(
        &l.tiles,
        &l.socket_rules,
        l.next_id(),
        &showcase(),
    )?);
    l.validate()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn small() -> GenerateOptions {
        GenerateOptions {
            width: 7,
            depth: 7,
            houses: 6,
            ..Default::default()
        }
    }
    fn names(w: &World, ts: &[Tile]) -> BTreeSet<String> {
        w.instances
            .iter()
            .map(|i| tile(ts, i.tile).unwrap().name.clone())
            .collect()
    }

    #[test]
    fn kit_tiles_are_real_assets_that_fit_their_cells_and_rules() {
        let (ts, rules) = starter_tiles(100);
        validate_tiles(&ts, &rules).unwrap();
        let unique: BTreeSet<_> = ts.iter().map(|t| t.name.as_str()).collect();
        assert_eq!(unique.len(), ts.len(), "tile names are unique");
        for t in &ts {
            assert!(!t.voxels.is_empty(), "{}", t.name);
            for s in &t.sockets {
                assert!(
                    rules.pairs.iter().flatten().any(|k| *k == s.kind),
                    "{} socket {} has no connection rule",
                    t.name,
                    s.name
                );
            }
            // Terrain cells stay inside their footprint so neighbours never overlap.
            if t.sockets.iter().any(|s| s.name == "edge-0") {
                for b in t.occupied() {
                    assert!(
                        [0, 2]
                            .iter()
                            .all(|&a| b.min[a] >= -6.401 && b.max[a] <= 6.401)
                            && b.min[1] >= -1.601,
                        "{} reaches outside its cell: {:?}..{:?}",
                        t.name,
                        b.min,
                        b.max
                    );
                }
            }
        }
    }

    #[test]
    fn generation_is_deterministic_and_valid_across_seeds_and_sizes() {
        let (ts, rules) = starter_tiles(100);
        let w = generate(&ts, &rules, 200, &small()).unwrap();
        assert_eq!(w, generate(&ts, &rules, 200, &small()).unwrap());
        for (seed, size, houses) in [(0, 5, 2), (23, 6, 4), (81, 8, 8), (5, 11, 20), (8, 7, 3)] {
            let o = GenerateOptions {
                seed,
                width: size,
                depth: size,
                houses,
                plant_percent: 30,
            };
            let w = generate(&ts, &rules, 200, &o)
                .unwrap_or_else(|e| panic!("seed {seed} size {size}: {e}"));
            validate_world(&w, &ts, &rules, true).unwrap();
            let lots = w
                .instances
                .iter()
                .filter(|i| tile(&ts, i.tile).unwrap().category == "lot")
                .count();
            assert_eq!(lots, houses, "seed {seed}");
        }
        let impossible = GenerateOptions {
            houses: 100,
            ..small()
        };
        assert!(generate(&ts, &rules, 200, &impossible).is_err());
    }

    #[test]
    fn showcase_uses_the_whole_kit() {
        let (ts, rules) = starter_tiles(100);
        let w = generate(&ts, &rules, 200, &showcase()).unwrap();
        let used = names(&w, &ts);
        for name in [
            "Hill top",
            "Hill spring",
            "Cliff",
            "Cliff corner",
            "Cliff stairs",
            "Cliff ramp",
            "Waterfall",
            "Stream straight",
            "Stream crossing",
            "Road bridge",
            "Road straight",
            "Road crossing",
            "Road T lane",
            "Lane straight",
            "House lot",
            "Shop lot",
            "Street lamp",
        ] {
            assert!(used.contains(name), "{name}");
        }
        let categories: BTreeSet<_> = w
            .instances
            .iter()
            .map(|i| tile(&ts, i.tile).unwrap().category.as_str())
            .collect();
        for c in [
            "building", "roof", "tree", "bush", "flowers", "fence", "street", "bridge", "park",
            "path",
        ] {
            assert!(categories.contains(c), "{c}");
        }
        // Every lot carries a finished building.
        for i in w.instances.iter() {
            let t = tile(&ts, i.tile).unwrap();
            if t.category == "lot" {
                let p = find_port(&w, &ts, i.id, "building").unwrap();
                assert!(w.connected(p), "lot {} has no building", i.id);
            }
        }
        assert!(!w.exits.is_empty());
    }

    #[test]
    fn road_pieces_connect_in_every_turn_and_profiles_must_match() {
        let (ts, rules) = starter_tiles(100);
        let mut k = Kit::new(&ts, &rules);
        let road = k.named("Road straight").unwrap();
        let corner = k.named("Road corner").unwrap();
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
            let id = k.attach(&mut w, p, "Road corner").unwrap();
            assert!(w.connected(p));
            assert_eq!(
                w.instances.iter().find(|i| i.id == id).unwrap().tile,
                corner.id
            );
            // A lot cannot face the carriageway.
            let mut w = World::empty(200);
            connect_instance(&mut w, &ts, &rules, w_instance(road.id, turn)).unwrap();
            let p = find_port(&w, &ts, 1, "edge-0").unwrap();
            assert!(k.attach(&mut w, p, "House lot").is_err());
        }
        let a = road
            .sockets
            .iter()
            .find(|s| s.kind == "road")
            .unwrap()
            .clone();
        let mut other = a.clone();
        other.rotation =
            (Quat::from_array(a.rotation) * Quat::from_rotation_y(std::f32::consts::PI)).to_array();
        let i = w_instance(road.id, 0);
        assert!(compatible((&i, &a), (&i, &other), &rules));
        other.profile = "road-64".into();
        assert!(!compatible((&i, &a), (&i, &other), &rules));
    }
    fn w_instance(tile: u32, turn: u8) -> Instance {
        Instance {
            id: 1,
            tile,
            position: [0.; 3],
            rotation: yaw(turn),
        }
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
        assert!(
            install(&mut l).is_err(),
            "an existing kit is never replaced"
        );
    }

    #[test]
    fn sample_character_walks_from_the_road_through_the_front_door_to_the_back_wall() {
        let library = crate::storage::load_workspace(std::path::Path::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/test_workspace"
        )))
        .unwrap();
        let (lo, hi) = library.bodies[0].bounds().unwrap();
        let (ts, rules) = starter_tiles(100);
        let w = generate(&ts, &rules, 200, &small()).unwrap();
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
        let forward = Quat::from_rotation_y(w.spawn_yaw) * -Vec3::Z;
        let mut on_curb = false;
        for _ in 0..600 {
            crate::terrain_collision::step_on_terrain(
                &mut player,
                &input,
                w.spawn_yaw,
                1. / 60.,
                &terrain,
                hi.y - lo.y,
            );
            let progress = (player.position - start).dot(forward);
            on_curb |= (1.5..3.).contains(&progress) && (player.position.y - 0.2).abs() < 0.01;
        }
        let progress = (player.position - start).dot(forward);
        assert!(on_curb, "the walk climbs the curb onto the sidewalk");
        assert!(
            progress > 12.2 && progress < 12.8,
            "Entrance/wall traversal stopped at {progress}, avatar height {}",
            hi.y - lo.y
        );
        assert!(
            (player.position.y - 0.4).abs() < 0.02,
            "inside on the house floor"
        );
    }
}

#[cfg(test)]
mod walkway_rules {
    use super::*;
    /// The validator's walkway rule still holds for user-made walkway tiles.
    #[test]
    fn closed_walkway_loops_are_rejected() {
        let slab = Volume {
            min: [-8, -2, 0],
            max: [8, 0, 16],
            color: [0.6; 3],
        };
        let corner = Tile {
            id: 1,
            name: "Walk".into(),
            category: "walkway".into(),
            voxels: vec![slab],
            sockets: vec![
                Socket {
                    name: "start".into(),
                    kind: "path".into(),
                    position: [0, 0, 0],
                    rotation: Quat::from_rotation_y(std::f32::consts::PI).to_array(),
                    profile: "path-16".into(),
                    required: true,
                },
                Socket {
                    name: "end".into(),
                    kind: "path".into(),
                    position: [8, 0, 8],
                    rotation: Quat::from_rotation_y(std::f32::consts::FRAC_PI_2).to_array(),
                    profile: "path-16".into(),
                    required: true,
                },
            ],
            rotations: (0..4).map(yaw).collect(),
            colliders: vec![Bounds {
                min: [-0.8, -0.2, 0.],
                max: [0.8, 0., 1.6],
            }],
        };
        let ts = vec![corner];
        let rules = SocketRules {
            pairs: vec![["path".into(), "path".into()]],
        };
        let mut k = Kit::new(&ts, &rules);
        let mut w = World::empty(9);
        k.place(
            &mut w,
            Instance {
                id: 1,
                tile: 1,
                position: [0., 0.2, 0.],
                rotation: identity(),
            },
        )
        .unwrap();
        let mut last = 1;
        for _ in 0..3 {
            let p = find_port(&w, &ts, last, "end").unwrap();
            last = k.attach(&mut w, p, "Walk").unwrap();
        }
        assert_eq!(w.connections.len(), 4);
        assert!(
            validate_world(&w, &ts, &rules, true)
                .unwrap_err()
                .contains("one door")
        );
    }
}
