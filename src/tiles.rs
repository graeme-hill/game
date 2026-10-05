//! Socket-connected spatial graphs. Grids are a generator convention, never world storage.
use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

pub const UNIT: f32 = 0.1;
pub fn identity() -> [f32; 4] {
    [0., 0., 0., 1.]
}
pub fn yaw(turns: u8) -> [f32; 4] {
    Quat::from_rotation_y(f32::from(turns) * std::f32::consts::FRAC_PI_2).to_array()
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Volume {
    pub min: [i32; 3],
    pub max: [i32; 3],
    pub color: [f32; 3],
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Bounds {
    pub min: [f32; 3],
    pub max: [f32; 3],
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Socket {
    pub name: String,
    pub kind: String,
    pub position: [i32; 3],
    /// Local +Z faces out of the connection; +Y defines the frame's up direction.
    pub rotation: [f32; 4],
    pub profile: String,
    pub required: bool,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Tile {
    pub id: u32,
    pub name: String,
    pub category: String,
    /// Ordered voxel cuboids. Later cuboids replace earlier cells (e.g. road markings).
    pub voxels: Vec<Volume>,
    pub sockets: Vec<Socket>,
    pub rotations: Vec<[f32; 4]>,
    pub colliders: Vec<Bounds>,
}
impl Tile {
    /// Render occupancy includes non-solid foliage and decoration; collision remains independently authored.
    pub fn occupied(&self) -> Vec<Bounds> {
        let mut boxes = self.colliders.clone();
        boxes.extend(self.voxels.iter().map(|v| Bounds {
            min: v.min.map(|n| n as f32 * UNIT),
            max: v.max.map(|n| n as f32 * UNIT),
        }));
        boxes
    }
}
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct SocketRules {
    pub pairs: Vec<[String; 2]>,
}
impl SocketRules {
    pub fn allows(&self, a: &str, b: &str) -> bool {
        self.pairs
            .iter()
            .any(|p| (p[0] == a && p[1] == b) || (p[1] == a && p[0] == b))
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Instance {
    pub id: u32,
    pub tile: u32,
    pub position: [f32; 3],
    pub rotation: [f32; 4],
}
impl Instance {
    pub fn transform(&self) -> Transform {
        Transform::from_translation(Vec3::from_array(self.position))
            .with_rotation(Quat::from_array(self.rotation))
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Port {
    pub instance: u32,
    pub socket: usize,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Connection {
    pub a: Port,
    pub b: Port,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct World {
    pub id: u32,
    pub name: String,
    pub seed: u64,
    pub spawn: [f32; 3],
    #[serde(default)]
    pub spawn_yaw: f32,
    pub instances: Vec<Instance>,
    pub connections: Vec<Connection>,
    /// Explicit road exits on the outer boundary; never an implicit dangling edge.
    pub exits: Vec<Port>,
    pub bounds: Bounds,
}
impl World {
    pub fn empty(id: u32) -> Self {
        Self {
            id,
            name: "New world".into(),
            seed: 1,
            spawn: [0., 0.2, 0.],
            spawn_yaw: 0.,
            instances: vec![],
            connections: vec![],
            exits: vec![],
            bounds: Bounds {
                min: [-100., -10., -100.],
                max: [100., 50., 100.],
            },
        }
    }
    pub fn next_id(&self) -> u32 {
        self.instances.iter().map(|v| v.id).max().unwrap_or(0) + 1
    }
    pub fn connected(&self, p: Port) -> bool {
        self.connections.iter().any(|c| c.a == p || c.b == p)
    }
    pub fn remove(&mut self, id: u32) {
        self.instances.retain(|i| i.id != id);
        self.connections
            .retain(|c| c.a.instance != id && c.b.instance != id);
        self.exits.retain(|p| p.instance != id);
    }
}
pub fn tile(tiles: &[Tile], id: u32) -> Result<&Tile, String> {
    tiles
        .iter()
        .find(|t| t.id == id)
        .ok_or_else(|| format!("Missing tile {id}"))
}
pub fn port<'a>(
    world: &'a World,
    tiles: &'a [Tile],
    p: Port,
) -> Result<(&'a Instance, &'a Socket), String> {
    let i = world
        .instances
        .iter()
        .find(|i| i.id == p.instance)
        .ok_or("Missing instance")?;
    let s = tile(tiles, i.tile)?
        .sockets
        .get(p.socket)
        .ok_or("Missing socket")?;
    Ok((i, s))
}
pub fn frame(i: &Instance, s: &Socket) -> (Vec3, Quat) {
    let t = i.transform();
    (
        t.transform_point(Vec3::from_array(s.position.map(|v| v as f32 * UNIT))),
        t.rotation * Quat::from_array(s.rotation),
    )
}
pub fn compatible(a: (&Instance, &Socket), b: (&Instance, &Socket), rules: &SocketRules) -> bool {
    if !rules.allows(&a.1.kind, &b.1.kind) || a.1.profile != b.1.profile {
        return false;
    }
    let (pa, ra) = frame(a.0, a.1);
    let (pb, rb) = frame(b.0, b.1);
    pa.distance(pb) < 0.002
        && (ra * Vec3::Z).dot(rb * Vec3::Z) < -0.999
        && (ra * Vec3::Y).dot(rb * Vec3::Y) > 0.999
}
pub fn aligned(
    world: &World,
    tiles: &[Tile],
    target: Port,
    candidate: &Tile,
    socket: usize,
    rotation: [f32; 4],
) -> Result<Instance, String> {
    let (i, s) = port(world, tiles, target)?;
    let (p, _) = frame(i, s);
    let local = candidate
        .sockets
        .get(socket)
        .ok_or("Missing candidate socket")?;
    Ok(Instance {
        id: world.next_id(),
        tile: candidate.id,
        position: (p - Quat::from_array(rotation)
            * Vec3::from_array(local.position.map(|v| v as f32 * UNIT)))
        .to_array(),
        rotation,
    })
}
pub fn world_bounds(i: &Instance, b: &Bounds) -> Bounds {
    let mut min = Vec3::splat(f32::INFINITY);
    let mut max = Vec3::splat(f32::NEG_INFINITY);
    for x in [b.min[0], b.max[0]] {
        for y in [b.min[1], b.max[1]] {
            for z in [b.min[2], b.max[2]] {
                let p = i.transform().transform_point(Vec3::new(x, y, z));
                min = min.min(p);
                max = max.max(p);
            }
        }
    }
    Bounds {
        min: min.to_array(),
        max: max.to_array(),
    }
}
pub fn intersects(a: &Bounds, b: &Bounds) -> bool {
    (0..3).all(|k| a.max[k] > b.min[k] + 0.002 && b.max[k] > a.min[k] + 0.002)
}
/// Separating-axis test keeps arbitrary authored orientations useful, including tilted branches.
#[derive(Clone)]
pub struct OrientedBox {
    pub center: Vec3,
    pub half: Vec3,
    pub axes: [Vec3; 3],
    pub bounds: Bounds,
}
impl OrientedBox {
    pub fn new(i: &Instance, b: &Bounds) -> Self {
        let rotation = Quat::from_array(i.rotation);
        Self {
            center: i
                .transform()
                .transform_point((Vec3::from_array(b.min) + Vec3::from_array(b.max)) * 0.5),
            half: (Vec3::from_array(b.max) - Vec3::from_array(b.min)) * 0.5,
            axes: [rotation * Vec3::X, rotation * Vec3::Y, rotation * Vec3::Z],
            bounds: world_bounds(i, b),
        }
    }
    pub fn overlaps(&self, other: &Self) -> bool {
        if !intersects(&self.bounds, &other.bounds) {
            return false;
        }
        let delta = other.center - self.center;
        let separated = |axis: Vec3| {
            let Some(axis) = axis.try_normalize() else {
                return false;
            };
            let ra = (0..3)
                .map(|n| self.half[n] * axis.dot(self.axes[n]).abs())
                .sum::<f32>();
            let rb = (0..3)
                .map(|n| other.half[n] * axis.dot(other.axes[n]).abs())
                .sum::<f32>();
            delta.dot(axis).abs() >= ra + rb - 0.002
        };
        !self
            .axes
            .iter()
            .chain(other.axes.iter())
            .copied()
            .any(separated)
            && !self
                .axes
                .iter()
                .any(|a| other.axes.iter().any(|b| separated(a.cross(*b))))
    }
}
pub fn can_place(world: &World, tiles: &[Tile], instance: &Instance) -> Result<(), String> {
    let mut occupied = vec![];
    for other in &world.instances {
        for b in tile(tiles, other.tile)?.occupied() {
            occupied.push((other.id, OrientedBox::new(other, &b)));
        }
    }
    for b in tile(tiles, instance.tile)?.occupied() {
        let candidate = OrientedBox::new(instance, &b);
        let c = &candidate.bounds;
        if (0..3).any(|a| {
            c.min[a] < world.bounds.min[a] - 0.002 || c.max[a] > world.bounds.max[a] + 0.002
        }) {
            return Err("Piece is outside world bounds".into());
        }
        for (id, other) in &occupied {
            if candidate.overlaps(other) {
                return Err(format!("Piece overlaps instance {id}"));
            }
        }
    }
    Ok(())
}
/// Connect all coincident compatible ports, rejecting mismatches and double use.
pub fn connect_instance(
    world: &mut World,
    tiles: &[Tile],
    rules: &SocketRules,
    i: Instance,
) -> Result<(), String> {
    if i.id == 0 || i.id == u32::MAX || world.instances.iter().any(|old| old.id == i.id) {
        return Err("Invalid or duplicate instance ID".into());
    }
    can_place(world, tiles, &i)?;
    let mut links = vec![];
    for (si, s) in tile(tiles, i.tile)?.sockets.iter().enumerate() {
        for other in &world.instances {
            for (oi, os) in tile(tiles, other.tile)?.sockets.iter().enumerate() {
                let (p, r) = frame(&i, s);
                let (op, or) = frame(other, os);
                if p.distance(op) > 0.002 || (r * Vec3::Z).dot(or * Vec3::Z) > -0.999 {
                    continue;
                }
                let a = Port {
                    instance: i.id,
                    socket: si,
                };
                let b = Port {
                    instance: other.id,
                    socket: oi,
                };
                if !compatible((&i, s), (other, os), rules) {
                    return Err(format!("Incompatible sockets {} / {}", s.name, os.name));
                }
                if world.connected(b) || links.iter().any(|c: &Connection| c.a == a || c.b == b) {
                    return Err("Socket already occupied".into());
                }
                links.push(Connection { a, b });
            }
        }
    }
    // Preserve the usable space immediately outside every still-open socket.
    // A matching connection consumes that space; an unrelated solid cannot bury it.
    let candidate_shapes = tile(tiles, i.tile)?
        .occupied()
        .iter()
        .map(|b| OrientedBox::new(&i, b))
        .collect::<Vec<_>>();
    let mut existing_shapes = vec![];
    for old in &world.instances {
        existing_shapes.extend(
            tile(tiles, old.tile)?
                .occupied()
                .iter()
                .map(|b| OrientedBox::new(old, b)),
        );
    }
    let mut all = world.instances.iter().collect::<Vec<_>>();
    all.push(&i);
    for owner in &all {
        for (socket, s) in tile(tiles, owner.tile)?.sockets.iter().enumerate() {
            let p = Port {
                instance: owner.id,
                socket,
            };
            if world.connected(p)
                || world.exits.contains(&p)
                || links.iter().any(|c| c.a == p || c.b == p)
            {
                continue;
            }
            let (position, rotation) = frame(owner, s);
            let point = position + rotation * Vec3::Z * 0.025;
            let shapes = if owner.id == i.id {
                &existing_shapes
            } else {
                &candidate_shapes
            };
            for shape in shapes {
                let d = point - shape.center;
                if (0..3).all(|a| d.dot(shape.axes[a]).abs() < shape.half[a] - 0.002) {
                    return Err(format!(
                        "Placement blocks unused socket {}:{}",
                        owner.id, s.name
                    ));
                }
            }
        }
    }
    world.instances.push(i);
    world.connections.extend(links);
    Ok(())
}
fn valid_quat(q: [f32; 4]) -> bool {
    q.iter().all(|v| v.is_finite()) && (Quat::from_array(q).length_squared() - 1.).abs() < 0.001
}
pub fn validate_tiles(tiles: &[Tile], rules: &SocketRules) -> Result<(), String> {
    if rules.pairs.iter().flatten().any(|s| s.trim().is_empty()) {
        return Err("Socket rule types cannot be empty".into());
    }
    for t in tiles {
        if t.rotations.is_empty() || t.rotations.iter().any(|q| !valid_quat(*q)) {
            return Err(format!("Invalid rotations in {}", t.name));
        }
        let mut names = BTreeSet::new();
        for s in &t.sockets {
            if !names.insert(&s.name)
                || s.kind.is_empty()
                || s.profile.is_empty()
                || s.position.iter().any(|p| p.unsigned_abs() > 512)
                || !valid_quat(s.rotation)
            {
                return Err(format!("Invalid socket in {}", t.name));
            }
        }
        for v in &t.voxels {
            if (0..3).any(|k| {
                v.min[k] >= v.max[k]
                    || v.min[k] < -512
                    || v.max[k] > 512
                    || !v.color[k].is_finite()
                    || !(0. ..=1.).contains(&v.color[k])
            }) {
                return Err(format!("Invalid voxel volume in {}", t.name));
            }
        }
        for b in &t.colliders {
            validate_bounds(b)?;
        }
    }
    Ok(())
}
fn validate_bounds(b: &Bounds) -> Result<(), String> {
    if (0..3).any(|k| {
        !b.min[k].is_finite()
            || !b.max[k].is_finite()
            || b.min[k] >= b.max[k]
            || b.min[k].abs() > 10000.
            || b.max[k].abs() > 10000.
    }) {
        Err("Invalid bounds".into())
    } else {
        Ok(())
    }
}
/// Draft validation checks references/frames. Completion additionally checks structure and routes.
pub fn validate_world(
    w: &World,
    tiles: &[Tile],
    rules: &SocketRules,
    complete: bool,
) -> Result<(), String> {
    validate_bounds(&w.bounds)?;
    if !w.spawn_yaw.is_finite() || w.spawn.iter().any(|v| !v.is_finite()) {
        return Err("Invalid spawn".into());
    }
    let mut ids = BTreeSet::new();
    let mut used = BTreeSet::new();
    for i in &w.instances {
        let t = tile(tiles, i.tile)?;
        if !ids.insert(i.id)
            || i.id == 0
            || i.id == u32::MAX
            || i.position
                .iter()
                .any(|v| !v.is_finite() || v.abs() > 10000.)
            || !valid_quat(i.rotation)
            || !t.rotations.iter().any(|q| {
                Quat::from_array(*q)
                    .angle_between(Quat::from_array(i.rotation))
                    .abs()
                    < 0.002
            })
        {
            return Err("Invalid instance ID/transform/orientation".into());
        }
    }
    for c in &w.connections {
        if c.a.instance == c.b.instance
            || !used.insert(c.a)
            || !used.insert(c.b)
            || !compatible(port(w, tiles, c.a)?, port(w, tiles, c.b)?, rules)
        {
            return Err("Invalid or multiply-used connection".into());
        }
    }
    for p in &w.exits {
        let (i, s) = port(w, tiles, *p)?;
        let (pos, _) = frame(i, s);
        if !used.insert(*p)
            || s.kind != "road"
            || ![0, 2].iter().any(|&a| {
                (pos[a] - w.bounds.min[a]).abs() < 0.002 || (pos[a] - w.bounds.max[a]).abs() < 0.002
            })
        {
            return Err("Road exit must lie on the world boundary".into());
        }
    }
    if !complete {
        return Ok(());
    }
    let mut placed = World::empty(w.id);
    placed.bounds = w.bounds.clone();
    for i in &w.instances {
        connect_instance(&mut placed, tiles, rules, i.clone())?;
        for (n, s) in tile(tiles, i.tile)?.sockets.iter().enumerate() {
            if s.required
                && !used.contains(&Port {
                    instance: i.id,
                    socket: n,
                })
            {
                return Err(format!(
                    "Unconnected required socket: {} / {}",
                    i.id, s.name
                ));
            }
        }
    }
    for c in &placed.connections {
        if !w
            .connections
            .iter()
            .any(|saved| (saved.a == c.a && saved.b == c.b) || (saved.a == c.b && saved.b == c.a))
        {
            return Err("Touching sockets must have an explicit connection".into());
        }
    }
    if w.instances.is_empty() {
        return Err("Place terrain before playing the world".into());
    }
    let collision = crate::terrain_collision::TerrainCollision::from_world(w, tiles)?;
    if !collision
        .ground(Vec3::from_array(w.spawn), w.spawn[1] + 0.02, 0.26)
        .is_some_and(|y| w.spawn[1] - y < 0.3)
    {
        return Err("Spawn must be supported by terrain".into());
    }
    // Every path component is an unbranched door-to-sidewalk route. Optional sidewalk ports do not grow paths.
    let mut graph: BTreeMap<u32, Vec<u32>> = BTreeMap::new();
    for c in &w.connections {
        let (_, a) = port(w, tiles, c.a)?;
        let (_, b) = port(w, tiles, c.b)?;
        if ["path", "door", "sidewalk"].contains(&a.kind.as_str())
            && ["path", "door", "sidewalk"].contains(&b.kind.as_str())
        {
            graph.entry(c.a.instance).or_default().push(c.b.instance);
            graph.entry(c.b.instance).or_default().push(c.a.instance);
        }
    }
    let mut visited = BTreeSet::new();
    for i in &w.instances {
        if tile(tiles, i.tile)?.category != "walkway" || visited.contains(&i.id) {
            continue;
        }
        let mut queue = vec![i.id];
        let mut doors = 0;
        let mut sidewalks = 0;
        while let Some(id) = queue.pop() {
            let inst = w.instances.iter().find(|v| v.id == id).unwrap();
            let t = tile(tiles, inst.tile)?;
            if t.category == "walkway" {
                if !visited.insert(id) {
                    continue;
                }
                let neighbors = graph.get(&id).ok_or("Isolated walkway")?;
                if neighbors.len() != 2 {
                    return Err("Walkway must have two connections".into());
                }
                queue.extend(neighbors);
            } else if t.category == "door" {
                doors += 1;
            } else if t.category == "terrain" {
                sidewalks += 1;
            } else {
                return Err("Invalid walkway endpoint".into());
            }
        }
        if doors != 1 || sidewalks != 1 {
            return Err("Each walkway must connect one door to one sidewalk".into());
        }
    }
    Ok(())
}

#[cfg(test)]
mod clearance_tests {
    use super::*;
    #[test]
    fn attachment_cannot_bury_another_unused_socket() {
        let socket = Socket {
            name: "join".into(),
            kind: "k".into(),
            position: [0; 3],
            rotation: identity(),
            profile: "p".into(),
            required: false,
        };
        let mut parent = Tile {
            id: 1,
            name: "Parent".into(),
            category: "test".into(),
            voxels: vec![],
            sockets: vec![socket.clone()],
            rotations: vec![identity()],
            colliders: vec![],
        };
        let mut second = socket.clone();
        second.name = "keep-free".into();
        second.position = [1, 0, 0];
        parent.sockets.push(second);
        let mut plug = socket;
        plug.rotation = yaw(2);
        let child = Tile {
            id: 2,
            name: "Child".into(),
            category: "test".into(),
            voxels: vec![Volume {
                min: [-1, -1, 0],
                max: [3, 1, 3],
                color: [0.5; 3],
            }],
            sockets: vec![plug],
            rotations: vec![identity()],
            colliders: vec![],
        };
        let ts = vec![parent, child];
        let rules = SocketRules {
            pairs: vec![["k".into(), "k".into()]],
        };
        let mut w = World::empty(3);
        connect_instance(
            &mut w,
            &ts,
            &rules,
            Instance {
                id: 1,
                tile: 1,
                position: [0.; 3],
                rotation: identity(),
            },
        )
        .unwrap();
        let error = connect_instance(
            &mut w,
            &ts,
            &rules,
            Instance {
                id: 2,
                tile: 2,
                position: [0.; 3],
                rotation: identity(),
            },
        )
        .unwrap_err();
        assert!(error.contains("blocks unused socket"));
        assert_eq!(w.instances.len(), 1);
    }
}
