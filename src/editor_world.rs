//! Tile and graph-world editing, using the ordinary editor history and input controls.
use crate::{
    editor::{Action, Editor, Mode},
    editor_ui::{button, item, row},
};
use bevy::prelude::*;
use game::tiles::World;
use game::{
    character_render::CharacterRenderer,
    generation::{self, GenerateOptions},
    tiles::*,
};
#[derive(Clone, Debug)]
pub enum WorldAction {
    OpenTile(usize),
    OpenWorld(usize),
    ToggleRule,
    NewTile,
    DuplicateTile,
    Brush(u32),
    Play,
    Tile(i32),
    World(i32),
    Instance(i32),
    Socket(i32),
    Rotate,
    Place,
    Remove,
    NewWorld,
    Generate,
    Seed,
    Size(i32),
    Houses(i32),
    Plants(i32),
    Validate,
    Install,
    Tool(u8),
    Cursor(usize, i32),
    Paint(bool),
    SocketKind,
    SocketRequired,
    SocketTurn,
    AddSocket,
    DeleteSocket,
    Collider,
    ClearColliders,
    Pick(u32),
}
#[derive(Clone, Debug)]
pub struct WorldTools {
    pub tile: usize,
    pub world: usize,
    pub instance: usize,
    pub socket: usize,
    pub rotation: usize,
    pub cursor: [i32; 3],
    pub tool: u8,
    pub seed: u64,
    pub size: usize,
    pub houses: usize,
    pub plants: u8,
    pub rule_left: String,
    pub rule_right: String,
}
impl Default for WorldTools {
    fn default() -> Self {
        Self {
            tile: 0,
            world: 0,
            instance: 0,
            socket: 0,
            rotation: 0,
            cursor: [0; 3],
            tool: 0,
            seed: 7,
            size: 7,
            houses: 3,
            plants: 45,
            rule_left: "land".into(),
            rule_right: "land".into(),
        }
    }
}
fn cycle(n: usize, len: usize, d: i32) -> usize {
    if len == 0 {
        0
    } else {
        (n as i32 + d).rem_euclid(len as i32) as usize
    }
}
pub fn selected_world(e: &Editor) -> Option<&World> {
    e.library.worlds.get(e.world_tools.world)
}
pub fn selected_tile(e: &Editor) -> Option<&Tile> {
    e.library.tiles.get(e.world_tools.tile)
}
pub fn target_port(e: &Editor) -> Option<Port> {
    let w = selected_world(e)?;
    let i = w.instances.get(e.world_tools.instance)?;
    let t = tile(&e.library.tiles, i.tile).ok()?;
    (!t.sockets.is_empty()).then_some(Port {
        instance: i.id,
        socket: e.world_tools.socket % t.sockets.len(),
    })
}
pub fn candidate(e: &Editor) -> Result<Instance, String> {
    let w = selected_world(e).ok_or("Create a world first")?;
    let t = selected_tile(e).ok_or("Select a tile")?;
    let r = t.rotations[e.world_tools.rotation % t.rotations.len()];
    if w.instances.is_empty() {
        return Ok(Instance {
            id: w.next_id(),
            tile: t.id,
            position: [0.; 3],
            rotation: r,
        });
    }
    let p = target_port(e).ok_or("Select a socket")?;
    if w.connected(p) {
        return Err("Selected socket is occupied".into());
    }
    for s in 0..t.sockets.len() {
        let i = aligned(w, &e.library.tiles, p, t, s, r)?;
        if compatible(
            port(w, &e.library.tiles, p)?,
            (&i, &t.sockets[s]),
            &e.library.socket_rules,
        ) {
            can_place(w, &e.library.tiles, &i)?;
            return Ok(i);
        }
    }
    Err("Choose a compatible tile and orientation".into())
}
pub fn action(e: &mut Editor, a: &WorldAction) -> Result<(), String> {
    let wt = &mut e.world_tools;
    match *a {
        WorldAction::ToggleRule => {
            if e.library.socket_rules.allows(&wt.rule_left, &wt.rule_right) {
                e.library.socket_rules.pairs.retain(|p| {
                    !((p[0] == wt.rule_left && p[1] == wt.rule_right)
                        || (p[1] == wt.rule_left && p[0] == wt.rule_right))
                });
            } else {
                e.library
                    .socket_rules
                    .pairs
                    .push([wt.rule_left.clone(), wt.rule_right.clone()]);
            }
        }
        WorldAction::OpenTile(n) => {
            wt.tile = n;
            e.mode = Mode::Tiles;
            e.pitch = 0.65;
            e.distance = 24.;
            wt.socket = 0;
        }
        WorldAction::OpenWorld(n) => {
            wt.world = n;
            e.mode = Mode::Worlds;
            e.pitch = 0.65;
            e.distance = 175.;
            wt.instance = 0;
            wt.socket = 0;
        }
        WorldAction::NewTile => {
            let id = e.library.next_id();
            e.library.tiles.push(Tile {
                id,
                name: format!("Tile {id}"),
                category: "terrain".into(),
                voxels: vec![],
                sockets: vec![],
                colliders: vec![],
                rotations: (0..4).map(yaw).collect(),
            });
            wt.tile = e.library.tiles.len() - 1;
            wt.socket = 0;
        }
        WorldAction::DuplicateTile => {
            let mut t = e.library.tiles.get(wt.tile).ok_or("No tile")?.clone();
            t.id = e.library.next_id();
            t.name = format!("{} copy", t.name);
            e.library.tiles.push(t);
            wt.tile = e.library.tiles.len() - 1;
        }
        WorldAction::Brush(size) => e.brush = size,
        WorldAction::Play => {
            let w = selected_world(e).ok_or("No world")?;
            validate_world(w, &e.library.tiles, &e.library.socket_rules, true)?;
            let id = w.id.to_string();
            game::storage::save_workspace(&e.workspace_dir, &e.library)?;
            std::process::Command::new(std::env::current_exe().map_err(|e| e.to_string())?)
                .args(["--mode", "game", "--workspace"])
                .arg(&e.workspace_dir)
                .args(["--world", &id])
                .spawn()
                .map_err(|e| e.to_string())?;
            e.dirty = false;
        }
        WorldAction::Tile(d) => {
            wt.tile = cycle(wt.tile, e.library.tiles.len(), d);
            wt.rotation = 0;
            if e.mode == Mode::Tiles {
                wt.socket = 0;
            }
        }
        WorldAction::World(d) => {
            wt.world = cycle(wt.world, e.library.worlds.len(), d);
            wt.instance = 0;
            wt.socket = 0;
        }
        WorldAction::Instance(d) => {
            wt.instance = cycle(
                wt.instance,
                e.library
                    .worlds
                    .get(wt.world)
                    .map_or(0, |w| w.instances.len()),
                d,
            );
            wt.socket = 0;
        }
        WorldAction::Pick(id) => {
            if let Some(w) = e.library.worlds.get(wt.world)
                && let Some(n) = w.instances.iter().position(|i| i.id == id)
            {
                wt.instance = n;
                wt.socket = 0;
            }
        }
        WorldAction::Socket(d) => {
            let count = if e.mode == Mode::Tiles {
                e.library.tiles.get(wt.tile).map_or(0, |t| t.sockets.len())
            } else {
                e.library
                    .worlds
                    .get(wt.world)
                    .and_then(|w| w.instances.get(wt.instance))
                    .and_then(|i| tile(&e.library.tiles, i.tile).ok())
                    .map_or(0, |t| t.sockets.len())
            };
            wt.socket = cycle(wt.socket, count, d);
        }
        WorldAction::Rotate => wt.rotation += 1,
        WorldAction::Place => {
            let i = candidate(e)?;
            connect_instance(
                &mut e.library.worlds[e.world_tools.world],
                &e.library.tiles,
                &e.library.socket_rules,
                i,
            )?;
        }
        WorldAction::Remove => {
            let w = e.library.worlds.get_mut(wt.world).ok_or("No world")?;
            let id = w.instances.get(wt.instance).ok_or("No instance")?.id;
            w.remove(id);
            wt.instance = wt.instance.min(w.instances.len().saturating_sub(1));
        }
        WorldAction::NewWorld => {
            e.library.worlds.push(World::empty(e.library.next_id()));
            wt.world = e.library.worlds.len() - 1;
            wt.instance = 0;
        }
        WorldAction::Install => generation::install(&mut e.library)?,
        WorldAction::Generate => {
            let options = GenerateOptions {
                seed: wt.seed,
                width: wt.size,
                depth: wt.size,
                houses: wt.houses,
                plant_percent: wt.plants,
            };
            let w = generation::generate(
                &e.library.tiles,
                &e.library.socket_rules,
                e.library.next_id(),
                &options,
            )?;
            e.library.worlds.push(w);
            wt.world = e.library.worlds.len() - 1;
            wt.instance = 0;
        }
        WorldAction::Seed => wt.seed = wt.seed.wrapping_add(1),
        WorldAction::Size(d) => wt.size = (wt.size as i32 + d).clamp(5, 11) as usize,
        WorldAction::Houses(d) => wt.houses = (wt.houses as i32 + d).clamp(0, 30) as usize,
        WorldAction::Plants(d) => wt.plants = (i32::from(wt.plants) + d).clamp(0, 100) as u8,
        WorldAction::Validate => {
            validate_world(
                selected_world(e).ok_or("No world")?,
                &e.library.tiles,
                &e.library.socket_rules,
                true,
            )?;
            e.notice = "World complete: sockets, occupancy and entrance routes are valid.".into();
        }
        WorldAction::Tool(n) => wt.tool = n,
        WorldAction::Cursor(axis, d) => {
            if wt.tool == 1 {
                let s = e
                    .library
                    .tiles
                    .get_mut(wt.tile)
                    .and_then(|t| t.sockets.get_mut(wt.socket))
                    .ok_or("No socket")?;
                s.position[axis] += d;
            } else {
                wt.cursor[axis] = (wt.cursor[axis] + d).clamp(-128, 128);
            }
        }
        WorldAction::Paint(erase) => {
            let t = e.library.tiles.get_mut(wt.tile).ok_or("No tile")?;
            paint(
                t,
                wt.cursor,
                e.brush as i32,
                (!erase).then_some(crate::editor::PALETTE[e.palette]),
            );
        }
        WorldAction::SocketKind => {
            let mut kinds: Vec<_> = e
                .library
                .socket_rules
                .pairs
                .iter()
                .flatten()
                .cloned()
                .collect();
            kinds.extend(
                e.library
                    .tiles
                    .iter()
                    .flat_map(|t| t.sockets.iter().map(|s| s.kind.clone())),
            );
            kinds.sort();
            kinds.dedup();
            let s = e
                .library
                .tiles
                .get_mut(wt.tile)
                .and_then(|t| t.sockets.get_mut(wt.socket))
                .ok_or("No socket")?;
            let n = kinds.iter().position(|k| *k == s.kind).unwrap_or(0);
            s.kind = kinds[(n + 1) % kinds.len()].clone();
            s.profile = match s.kind.as_str() {
                "road" => "road-128",
                "land" | "side-land" | "frontage" => "land-128",
                "door" | "path" | "sidewalk" => "path-16",
                "plot-house" | "house-base" => "house-64",
                "plant" | "trunk-base" | "trunk-top" | "leaf-base" => "tree",
                "branch-mount" | "branch-base" => "branch",
                k if k.contains("roof") || k == "wall-slope" || k == "wall-gable" => "roof-64",
                _ => "wall-64",
            }
            .into();
        }
        WorldAction::SocketRequired => {
            let s = e
                .library
                .tiles
                .get_mut(wt.tile)
                .and_then(|t| t.sockets.get_mut(wt.socket))
                .ok_or("No socket")?;
            s.required = !s.required;
        }
        WorldAction::SocketTurn => {
            let s = e
                .library
                .tiles
                .get_mut(wt.tile)
                .and_then(|t| t.sockets.get_mut(wt.socket))
                .ok_or("No socket")?;
            s.rotation = (Quat::from_array(yaw(1)) * Quat::from_array(s.rotation)).to_array();
        }
        WorldAction::AddSocket => {
            let t = e.library.tiles.get_mut(wt.tile).ok_or("No tile")?;
            let mut n = t.sockets.len();
            while t.sockets.iter().any(|s| s.name == format!("socket-{n}")) {
                n += 1;
            }
            t.sockets.push(Socket {
                name: format!("socket-{n}"),
                kind: "land".into(),
                profile: "land-128".into(),
                position: wt.cursor,
                rotation: identity(),
                required: false,
            });
            wt.socket = t.sockets.len() - 1;
        }
        WorldAction::DeleteSocket => {
            let t = e.library.tiles.get_mut(wt.tile).ok_or("No tile")?;
            if wt.socket < t.sockets.len() {
                t.sockets.remove(wt.socket);
                wt.socket = 0;
            }
        }
        WorldAction::Collider => {
            let t = e.library.tiles.get_mut(wt.tile).ok_or("No tile")?;
            t.colliders.push(Bounds {
                min: wt.cursor.map(|v| v as f32 * UNIT),
                max: wt.cursor.map(|v| (v + e.brush as i32) as f32 * UNIT),
            });
        }
        WorldAction::ClearColliders => {
            e.library
                .tiles
                .get_mut(wt.tile)
                .ok_or("No tile")?
                .colliders
                .clear();
        }
    }
    Ok(())
}
fn paint(t: &mut Tile, min: [i32; 3], size: i32, color: Option<[f32; 3]>) {
    let max = min.map(|v| v + size);
    let mut next = vec![];
    for v in &t.voxels {
        if (0..3).any(|a| v.max[a] <= min[a] || v.min[a] >= max[a]) {
            next.push(v.clone());
            continue;
        }
        let mut middle = v.clone();
        for a in 0..3 {
            if middle.min[a] < min[a] {
                let mut left = middle.clone();
                left.max[a] = min[a];
                next.push(left);
                middle.min[a] = min[a];
            }
            if middle.max[a] > max[a] {
                let mut right = middle.clone();
                right.min[a] = max[a];
                next.push(right);
                middle.max[a] = max[a];
            }
        }
    }
    if let Some(color) = color {
        next.push(Volume { min, max, color });
    }
    t.voxels = next;
}
pub fn preview_bounds(e: &Editor) -> (Vec3, Vec3) {
    let mut min = Vec3::splat(f32::INFINITY);
    let mut max = Vec3::splat(f32::NEG_INFINITY);
    let instances = if e.mode == Mode::Tiles {
        selected_tile(e)
            .map(|t| {
                vec![Instance {
                    id: 0,
                    tile: t.id,
                    position: [0.; 3],
                    rotation: t.rotations[e.world_tools.rotation % t.rotations.len()],
                }]
            })
            .unwrap_or_default()
    } else {
        selected_world(e).map_or(vec![], |w| w.instances.clone())
    };
    for i in instances {
        if let Ok(t) = tile(&e.library.tiles, i.tile) {
            for b in t.occupied() {
                let b = world_bounds(&i, &b);
                min = min.min(Vec3::from_array(b.min));
                max = max.max(Vec3::from_array(b.max));
            }
        }
    }
    if min.is_finite() {
        (min, max)
    } else {
        (Vec3::splat(-1.), Vec3::ONE)
    }
}
pub fn frame_distance(e: &Editor) -> f32 {
    let (a, b) = preview_bounds(e);
    ((b - a).max_element() * 2.1).clamp(2.5, 200.)
}
pub fn panel(p: &mut ChildSpawnerCommands, e: &Editor) {
    let wt = &e.world_tools;
    row(
        p,
        vec![
            ("Tile <", Action::World(WorldAction::Tile(-1)), 65.),
            ("Tile >", Action::World(WorldAction::Tile(1)), 65.),
            ("Rotate", Action::World(WorldAction::Rotate), 70.),
        ],
    );
    item(p, selected_tile(e).map_or("No tiles", |t| t.name.as_str()));
    item(p, format!("Orientation {}°", wt.rotation % 4 * 90));
    if e.library.tiles.is_empty() {
        button(
            p,
            "Create starter library",
            Action::World(WorldAction::Install),
            230.,
        );
        return;
    }
    if e.mode == Mode::Tiles {
        row(
            p,
            vec![
                ("New", Action::World(WorldAction::NewTile), 60.),
                ("Duplicate", Action::World(WorldAction::DuplicateTile), 85.),
                ("Rename", Action::Rename(crate::editor::Target::Asset), 70.),
            ],
        );
        row(
            p,
            vec![
                ("Voxels", Action::World(WorldAction::Tool(0)), 55.),
                ("Sockets", Action::World(WorldAction::Tool(1)), 62.),
                ("Collision", Action::World(WorldAction::Tool(2)), 65.),
                ("Rules", Action::World(WorldAction::Tool(3)), 55.),
            ],
        );
        if wt.tool == 3 {
            item(p, "Socket compatibility (symmetric)");
            item(p, format!("{} ↔ {}", wt.rule_left, wt.rule_right));
            button(
                p,
                "Edit first type",
                Action::Rename(crate::editor::Target::RuleLeft),
                230.,
            );
            button(
                p,
                "Edit second type",
                Action::Rename(crate::editor::Target::RuleRight),
                230.,
            );
            item(
                p,
                if e.library.socket_rules.allows(&wt.rule_left, &wt.rule_right) {
                    "Connection allowed"
                } else {
                    "Connection disallowed"
                },
            );
            button(
                p,
                "Toggle compatibility",
                Action::World(WorldAction::ToggleRule),
                230.,
            );
            button(
                p,
                "Edit tile category",
                Action::Rename(crate::editor::Target::TileCategory),
                230.,
            );
            return;
        }
        if wt.tool == 1 {
            row(
                p,
                vec![
                    ("Socket <", Action::World(WorldAction::Socket(-1)), 95.),
                    ("Socket >", Action::World(WorldAction::Socket(1)), 95.),
                ],
            );
            if let Some(s) = selected_tile(e).and_then(|t| t.sockets.get(wt.socket)) {
                item(p, format!("{}: {}", s.name, s.kind));
                item(p, format!("Profile: {}", s.profile));
                item(p, format!("Position {:?}", s.position));
            }
            row(
                p,
                vec![
                    (
                        "Name",
                        Action::Rename(crate::editor::Target::SocketName),
                        65.,
                    ),
                    (
                        "Type",
                        Action::Rename(crate::editor::Target::SocketType),
                        65.,
                    ),
                    (
                        "Profile",
                        Action::Rename(crate::editor::Target::SocketProfile),
                        70.,
                    ),
                ],
            );
            button(
                p,
                "Next socket type",
                Action::World(WorldAction::SocketKind),
                230.,
            );
            row(
                p,
                vec![
                    (
                        "Required / optional",
                        Action::World(WorldAction::SocketRequired),
                        145.,
                    ),
                    ("Turn", Action::World(WorldAction::SocketTurn), 65.),
                ],
            );
            row(
                p,
                vec![
                    ("Add socket", Action::World(WorldAction::AddSocket), 105.),
                    (
                        "Delete socket",
                        Action::World(WorldAction::DeleteSocket),
                        110.,
                    ),
                ],
            );
        } else {
            item(p, format!("Voxel cursor {:?}", wt.cursor));
            row(
                p,
                vec![
                    ("1 voxel", Action::World(WorldAction::Brush(1)), 68.),
                    ("4", Action::World(WorldAction::Brush(4)), 40.),
                    ("8", Action::World(WorldAction::Brush(8)), 40.),
                    ("16", Action::World(WorldAction::Brush(16)), 40.),
                ],
            );
            item(p, format!("Brush {} voxels", e.brush));
        }
        for a in 0..3 {
            row(
                p,
                vec![
                    (
                        ["X -", "Y -", "Z -"][a],
                        Action::World(WorldAction::Cursor(a, -1)),
                        95.,
                    ),
                    (
                        ["X +", "Y +", "Z +"][a],
                        Action::World(WorldAction::Cursor(a, 1)),
                        95.,
                    ),
                ],
            );
        }
        if wt.tool == 0 {
            row(
                p,
                vec![
                    ("Paint", Action::World(WorldAction::Paint(false)), 95.),
                    ("Erase", Action::World(WorldAction::Paint(true)), 95.),
                ],
            );
            crate::editor_ui::palette(p, e.palette);
        }
        if wt.tool == 2 {
            item(
                p,
                format!(
                    "{} collision boxes",
                    selected_tile(e).map_or(0, |t| t.colliders.len())
                ),
            );
            button(
                p,
                "Add box at cursor",
                Action::World(WorldAction::Collider),
                230.,
            );
            button(
                p,
                "Clear collision boxes",
                Action::World(WorldAction::ClearColliders),
                230.,
            );
        }
    } else {
        row(
            p,
            vec![
                ("World <", Action::World(WorldAction::World(-1)), 75.),
                ("World >", Action::World(WorldAction::World(1)), 75.),
                ("New", Action::World(WorldAction::NewWorld), 60.),
            ],
        );
        item(p, selected_world(e).map_or("No world", |w| w.name.as_str()));
        row(
            p,
            vec![
                ("Piece <", Action::World(WorldAction::Instance(-1)), 95.),
                ("Piece >", Action::World(WorldAction::Instance(1)), 95.),
            ],
        );
        if let Some(w) = selected_world(e)
            && let Some(i) = w.instances.get(wt.instance)
        {
            item(
                p,
                format!(
                    "#{} {}",
                    i.id,
                    tile(&e.library.tiles, i.tile).map_or("?", |t| t.name.as_str())
                ),
            );
        }
        row(
            p,
            vec![
                ("Socket <", Action::World(WorldAction::Socket(-1)), 95.),
                ("Socket >", Action::World(WorldAction::Socket(1)), 95.),
            ],
        );
        if let Some(p0) = target_port(e)
            && let Some(w) = selected_world(e)
            && let Ok((_, s)) = port(w, &e.library.tiles, p0)
        {
            item(
                p,
                format!(
                    "{} ({})",
                    s.name,
                    if w.connected(p0) { "connected" } else { "open" }
                ),
            );
        }
        row(
            p,
            vec![
                ("Place", Action::World(WorldAction::Place), 95.),
                ("Remove", Action::World(WorldAction::Remove), 95.),
            ],
        );
        row(
            p,
            vec![
                ("Validate", Action::World(WorldAction::Validate), 100.),
                ("Save & play", Action::World(WorldAction::Play), 110.),
            ],
        );
        item(
            p,
            format!(
                "Seed {} · {} × {} · {} houses",
                wt.seed, wt.size, wt.size, wt.houses
            ),
        );
        row(
            p,
            vec![
                ("Seed +", Action::World(WorldAction::Seed), 70.),
                ("Size -", Action::World(WorldAction::Size(-1)), 65.),
                ("Size +", Action::World(WorldAction::Size(1)), 65.),
            ],
        );
        row(
            p,
            vec![
                ("Houses -", Action::World(WorldAction::Houses(-1)), 95.),
                ("Houses +", Action::World(WorldAction::Houses(1)), 95.),
            ],
        );
        row(
            p,
            vec![
                ("Plants -", Action::World(WorldAction::Plants(-10)), 75.),
                ("Plants +", Action::World(WorldAction::Plants(10)), 75.),
            ],
        );
        item(p, format!("Plant sockets populated: {}%", wt.plants));
        button(
            p,
            "Generate new world",
            Action::World(WorldAction::Generate),
            230.,
        );
    }
}
pub fn render(e: &Editor, r: &mut CharacterRenderer) -> Vec<Entity> {
    if e.mode == Mode::Tiles {
        return selected_tile(e)
            .map(|t| {
                r.tile(
                    t,
                    Transform::from_rotation(Quat::from_array(
                        t.rotations[e.world_tools.rotation % t.rotations.len()],
                    )),
                )
            })
            .unwrap_or_default();
    }
    let mut entities = vec![];
    if let Some(w) = selected_world(e) {
        for i in &w.instances {
            if let Ok(t) = tile(&e.library.tiles, i.tile) {
                entities.extend(r.tile(t, i.transform()));
            }
        }
    }
    entities
}
pub fn guides(e: Res<Editor>, mut g: Gizmos) {
    if !matches!(e.mode, Mode::Tiles | Mode::Worlds) || !e.guides {
        return;
    }
    let pair = if e.mode == Mode::Tiles {
        selected_tile(&e).map(|t| {
            (
                Instance {
                    id: 0,
                    tile: t.id,
                    position: [0.; 3],
                    rotation: t.rotations[e.world_tools.rotation % t.rotations.len()],
                },
                t,
            )
        })
    } else {
        selected_world(&e)
            .and_then(|w| w.instances.get(e.world_tools.instance))
            .and_then(|i| tile(&e.library.tiles, i.tile).ok().map(|t| (i.clone(), t)))
    };
    if let Some((i, t)) = pair {
        for (n, s) in t.sockets.iter().enumerate() {
            let (p, r) = frame(&i, s);
            let c = if n == e.world_tools.socket {
                Color::srgb(1., 0.75, 0.1)
            } else {
                Color::srgb(0.1, 0.85, 0.95)
            };
            g.sphere(Isometry3d::from_translation(p), 0.12, c);
            g.line(p, p + r * Vec3::Z * 0.8, c);
        }
        if e.mode == Mode::Worlds || e.world_tools.tool == 2 {
            for b in &t.colliders {
                let local = Transform::from_translation(
                    (Vec3::from_array(b.min) + Vec3::from_array(b.max)) * 0.5,
                )
                .with_scale(Vec3::from_array(b.max) - Vec3::from_array(b.min));
                g.cube(
                    i.transform().mul_transform(local),
                    Color::srgba(0.9, 0.7, 0.2, 0.7),
                );
            }
        }
    }
    if e.mode == Mode::Worlds
        && let Ok(i) = candidate(&e)
        && let Ok(t) = tile(&e.library.tiles, i.tile)
    {
        for b in &t.colliders {
            let local = Transform::from_translation(
                (Vec3::from_array(b.min) + Vec3::from_array(b.max)) * 0.5,
            )
            .with_scale(Vec3::from_array(b.max) - Vec3::from_array(b.min));
            g.cube(
                i.transform().mul_transform(local),
                Color::srgb(0.2, 1., 0.4),
            );
        }
    }
    if e.mode == Mode::Tiles && e.world_tools.tool != 1 {
        let min = Vec3::from_array(e.world_tools.cursor.map(|v| v as f32 * UNIT));
        let size = e.brush as f32 * UNIT;
        g.cube(
            Transform::from_translation(min + Vec3::splat(size * 0.5))
                .with_scale(Vec3::splat(size)),
            Color::WHITE,
        );
    }
}
/// Pick actual rendered piece bounds with real mouse input; placement still goes through socket validation.
pub fn pick(
    mouse: Res<ButtonInput<MouseButton>>,
    window: Single<&Window>,
    cameras: Query<(&Camera, &GlobalTransform), With<crate::editor_scene::PreviewCamera>>,
    options: Res<crate::launch::LaunchOptions>,
    mut e: ResMut<Editor>,
) {
    if e.mode != Mode::Worlds || !mouse.just_pressed(MouseButton::Left) {
        return;
    }
    let Some(p) = window.cursor_position() else {
        return;
    };
    let Ok((camera, transform)) = cameras.single() else {
        return;
    };
    let Some(rect) = camera.logical_viewport_rect() else {
        return;
    };
    if !rect.contains(p) {
        return;
    }
    let Ok(ray) = camera.viewport_to_world(transform, p) else {
        return;
    };
    let mut nearest = (f32::INFINITY, 0);
    if let Some(w) = selected_world(&e) {
        for i in &w.instances {
            if let Ok(t) = tile(&e.library.tiles, i.tile) {
                for b in &t.colliders {
                    let b = world_bounds(i, b);
                    let mut near = 0f32;
                    let mut far = f32::INFINITY;
                    for a in 0..3 {
                        let d = ray.direction[a];
                        if d.abs() < 1e-6 {
                            if ray.origin[a] < b.min[a] || ray.origin[a] > b.max[a] {
                                far = -1.;
                            }
                        } else {
                            let u = (b.min[a] - ray.origin[a]) / d;
                            let v = (b.max[a] - ray.origin[a]) / d;
                            near = near.max(u.min(v));
                            far = far.min(u.max(v));
                        }
                    }
                    if near <= far && near < nearest.0 {
                        nearest = (near, i.id);
                    }
                }
            }
        }
    }
    if nearest.0.is_finite() {
        e.apply(Action::World(WorldAction::Pick(nearest.1)), &options);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn custom_socket_profiles_rules_and_voxels_are_undoable() {
        let options = crate::launch::LaunchOptions {
            workspace: None,
            data_dir: std::env::temp_dir()
                .join(format!("empty-tile-editor-{}", std::process::id())),
            ..default()
        };
        let mut e = Editor::new(&options);
        e.mode = Mode::Tiles;
        e.apply(Action::World(WorldAction::NewTile), &options);
        e.apply(Action::World(WorldAction::Paint(false)), &options);
        let voxels = e.library.tiles[0].voxels.clone();
        assert!(!voxels.is_empty());
        e.apply(Action::Undo, &options);
        assert!(e.library.tiles[0].voxels.is_empty());
        e.apply(Action::Redo, &options);
        assert_eq!(e.library.tiles[0].voxels, voxels);
        e.apply(Action::World(WorldAction::AddSocket), &options);
        for (target, value) in [
            (crate::editor::Target::SocketType, "custom"),
            (crate::editor::Target::SocketProfile, "width-24"),
            (crate::editor::Target::RuleLeft, "custom"),
            (crate::editor::Target::RuleRight, "another"),
        ] {
            e.apply(Action::Rename(target), &options);
            e.naming = Some((target, value.into()));
            e.apply(Action::NameCommit, &options);
        }
        e.apply(Action::World(WorldAction::ToggleRule), &options);
        assert_eq!(e.library.tiles[0].sockets[0].kind, "custom");
        assert_eq!(e.library.tiles[0].sockets[0].profile, "width-24");
        assert!(e.library.socket_rules.allows("another", "custom"));
        e.apply(Action::Undo, &options);
        assert!(!e.library.socket_rules.allows("custom", "another"));
    }
}
