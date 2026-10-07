//! Shared editor/runtime assembly rendering; document data stays immutable.
use crate::{
    animation::{BoneFrame, LocalPose, euler, forward_kinematics},
    body_render::{BodyMaterial, spawn_body_at},
    model::{Anchor, Attachment, Body, Character, Library, Mount, Prop, attachment_transform},
    voxel,
};
use bevy::{ecs::system::SystemParam, prelude::*};

type TileChunks = Vec<([i32; 3], Handle<Mesh>)>;
#[derive(Resource, Default)]
struct TileCache {
    entries: std::collections::BTreeMap<u32, (crate::tiles::Tile, TileChunks)>,
    material: Option<Handle<StandardMaterial>>,
}

#[derive(SystemParam)]
pub struct CharacterRenderer<'w, 's> {
    pub commands: Commands<'w, 's>,
    meshes: ResMut<'w, Assets<Mesh>>,
    materials: ResMut<'w, Assets<StandardMaterial>>,
    body_materials: ResMut<'w, Assets<BodyMaterial>>,
    tile_cache: ResMut<'w, TileCache>,
}

impl CharacterRenderer<'_, '_> {
    pub fn body(&mut self, body: &Body, position: Vec3) -> Entity {
        spawn_body_at(
            &mut self.commands,
            &mut self.meshes,
            &mut self.body_materials,
            body,
            position,
        )
    }

    pub fn prop(&mut self, prop: &Prop, transform: Transform) -> Option<Entity> {
        if prop.blocks.is_empty() {
            return None;
        }
        Some(
            self.commands
                .spawn((
                    Mesh3d(self.meshes.add(voxel::build_mesh(prop))),
                    MeshMaterial3d(self.materials.add(StandardMaterial {
                        base_color: Color::WHITE,
                        perceptual_roughness: 1.,
                        ..default()
                    })),
                    transform,
                ))
                .id(),
        )
    }

    /// Chunked voxel pieces share the prop mesher without inheriting its size limit.
    pub fn tile(&mut self, tile: &crate::tiles::Tile, transform: Transform) -> Vec<Entity> {
        if !self
            .tile_cache
            .entries
            .get(&tile.id)
            .is_some_and(|(saved, _)| saved == tile)
        {
            let mut chunks: std::collections::BTreeMap<[i32; 3], Vec<Option<[f32; 3]>>> =
                Default::default();
            for v in &tile.voxels {
                for z in v.min[2]..v.max[2] {
                    for y in v.min[1]..v.max[1] {
                        for x in v.min[0]..v.max[0] {
                            let p = [x, y, z];
                            let key = p.map(|a| a.div_euclid(32));
                            let local = p.map(|a| a.rem_euclid(32));
                            let cells = chunks
                                .entry(key)
                                .or_insert_with(|| vec![None; 32 * 32 * 32]);
                            cells[(local[0] + 32 * (local[1] + 32 * local[2])) as usize] =
                                Some(v.color);
                        }
                    }
                }
            }
            let chunks = chunks
                .into_iter()
                .map(|(key, cells)| (key, self.meshes.add(voxel::build_cells_mesh(&cells))))
                .collect();
            self.tile_cache
                .entries
                .insert(tile.id, (tile.clone(), chunks));
        }
        let material = if let Some(h) = &self.tile_cache.material {
            h.clone()
        } else {
            let h = self.materials.add(StandardMaterial {
                base_color: Color::WHITE,
                perceptual_roughness: 1.,
                ..default()
            });
            self.tile_cache.material = Some(h.clone());
            h
        };
        self.tile_cache.entries[&tile.id]
            .1
            .iter()
            .map(|(key, mesh)| {
                let local =
                    Transform::from_translation(Vec3::from_array(key.map(|v| v as f32 * 3.2)));
                self.commands
                    .spawn((
                        Mesh3d(mesh.clone()),
                        MeshMaterial3d(material.clone()),
                        transform.mul_transform(local),
                    ))
                    .id()
            })
            .collect()
    }

    pub fn character(
        &mut self,
        library: &Library,
        character: &Character,
        position: Vec3,
    ) -> Vec<Entity> {
        let Some(body) = library.bodies.iter().find(|body| body.id == character.body) else {
            return vec![];
        };
        if body.bones.is_empty() {
            return vec![self.body(body, position)];
        }
        let frames = forward_kinematics(body, &[]);
        let world = Transform::from_translation(position);
        let (material, transform) = BodyMaterial::posed(body, &frames, &world);
        let material = self.body_materials.add(material);
        let root = self
            .commands
            .spawn((
                Mesh3d(self.meshes.add(Cuboid::from_size(Vec3::ONE))),
                MeshMaterial3d(material.clone()),
                transform,
                VisualPose {
                    locals: vec![],
                    world,
                },
            ))
            .id();
        let mut entities = vec![root];
        for attachment in &character.attachments {
            let Some(prop) = library.props.iter().find(|prop| prop.id == attachment.prop) else {
                continue;
            };
            let Some(anchor) = prop
                .anchors
                .iter()
                .find(|anchor| anchor.id == attachment.anchor)
            else {
                continue;
            };
            let Some(mount) = body
                .mounts
                .iter()
                .find(|mount| mount.id == attachment.mount)
            else {
                continue;
            };
            let Some(mut transform) = attachment_transform(body, mount, prop, anchor, attachment)
            else {
                continue;
            };
            transform.translation += position;
            if let Some(entity) = self.prop(prop, transform) {
                self.commands.entity(entity).insert((
                    Name::new(prop.name.clone()),
                    MountedProp {
                        character: root,
                        prop: prop.id,
                        attachment: attachment.clone(),
                        mount: mount.clone(),
                        anchor: anchor.clone(),
                    },
                ));
                entities.push(entity);
            }
        }
        self.commands.entity(root).insert(AnimatedVisual {
            body: body.clone(),
            material,
            frames,
        });
        entities
    }
}

/// The proxy Transform is derived; callers animate this model-space pose/world frame.
#[derive(Component, Default)]
pub struct VisualPose {
    pub locals: Vec<LocalPose>,
    pub world: Transform,
}
#[derive(Component)]
pub struct AnimatedVisual {
    body: Body,
    material: Handle<BodyMaterial>,
    frames: Vec<BoneFrame>,
}

/// A logical bone parent, independent of the SDF proxy's changing bounds/scale.
/// Keep only binding data here; voxel geometry belongs to the loaded mesh asset.
#[derive(Component)]
pub struct MountedProp {
    pub character: Entity,
    pub prop: u32,
    pub attachment: Attachment,
    mount: Mount,
    anchor: Anchor,
}

#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct CharacterVisuals;

pub struct CharacterRenderPlugin;
impl Plugin for CharacterRenderPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<TileCache>().add_systems(
            PostUpdate,
            (update_visuals, update_mounted_props)
                .chain()
                .in_set(CharacterVisuals)
                .before(bevy::transform::TransformSystems::Propagate),
        );
    }
}

fn update_visuals(
    mut bodies: Query<(&mut AnimatedVisual, &VisualPose, &mut Transform)>,
    mut materials: ResMut<Assets<BodyMaterial>>,
) {
    for (mut visual, pose, mut proxy) in &mut bodies {
        visual.frames = forward_kinematics(&visual.body, &pose.locals);
        let (material, transform) = BodyMaterial::posed(&visual.body, &visual.frames, &pose.world);
        if let Some(mut current) = materials.get_mut(&visual.material) {
            *current = material;
        }
        *proxy = transform;
    }
}

fn update_mounted_props(
    mut commands: Commands,
    bodies: Query<(&AnimatedVisual, &VisualPose)>,
    mut props: Query<(Entity, &MountedProp, &mut Transform), Without<AnimatedVisual>>,
) {
    for (entity, binding, mut transform) in &mut props {
        if let Ok((visual, pose)) = bodies.get(binding.character) {
            let mount = &binding.mount;
            let anchor = &binding.anchor;
            let attachment = &binding.attachment;
            let Some(index) = visual.body.bones.iter().position(|b| b.id == mount.bone) else {
                continue;
            };
            let frame = visual.frames[index];
            let mount_rotation = frame.rotation * euler(mount.rotation);
            let rotation =
                mount_rotation * euler(attachment.rotation) * euler(anchor.rotation).inverse();
            let target = frame.end
                + frame.rotation * Vec3::from_array(mount.offset)
                + mount_rotation * Vec3::from_array(attachment.offset);
            let local = Transform {
                translation: target
                    - rotation * (Vec3::from_array(anchor.position) * attachment.scale),
                rotation,
                scale: Vec3::splat(attachment.scale),
            };
            *transform = pose.world.mul_transform(local);
        } else {
            // Match parenting lifetime when a character is removed/replaced.
            commands.entity(entity).despawn();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;
    use std::path::Path;

    #[test]
    fn workspace_props_follow_animated_bones_and_character_lifetime() {
        let workspace = Path::new(env!("CARGO_MANIFEST_DIR")).join("test_workspace");
        let library = crate::storage::load_workspace(&workspace).unwrap();
        let snapshot = crate::storage::load(&workspace.join("library.json")).unwrap();
        assert_eq!(library.bodies, snapshot.bodies);
        assert_eq!(library.props, snapshot.props);
        assert_eq!(library.characters, snapshot.characters);
        let body = library.bodies[0].clone();
        let character = library.characters[0].clone();
        let mut app = App::new();
        app.init_resource::<Assets<Mesh>>()
            .init_resource::<Assets<StandardMaterial>>()
            .init_resource::<Assets<BodyMaterial>>()
            .add_plugins(CharacterRenderPlugin);
        let entities = app
            .world_mut()
            .run_system_once(move |mut renderer: CharacterRenderer| {
                renderer.character(&library, &library.characters[0], Vec3::new(2., 3., 4.))
            })
            .unwrap();
        assert_eq!(entities.len(), 3, "body, hat and backpack must all spawn");
        let world = Transform::from_xyz(7., 2., -4.).with_rotation(Quat::from_rotation_y(1.2));
        for clip in &character.animations {
            for phase in [0., 0.23, 0.51, 0.79] {
                let locals =
                    crate::animation::blend_clips(&body, &[(clip, clip.duration * phase, 1.)]);
                let frames = forward_kinematics(&body, &locals);
                *app.world_mut().get_mut::<VisualPose>(entities[0]).unwrap() =
                    VisualPose { locals, world };
                app.update();
                for entity in &entities[1..] {
                    let binding = app.world().get::<MountedProp>(*entity).unwrap();
                    let transform = app.world().get::<Transform>(*entity).unwrap();
                    let index = body
                        .bones
                        .iter()
                        .position(|b| b.id == binding.mount.bone)
                        .unwrap();
                    let frame = frames[index];
                    let target = world.transform_point(
                        frame.end + frame.rotation * Vec3::from_array(binding.mount.offset),
                    );
                    let anchor =
                        transform.transform_point(Vec3::from_array(binding.anchor.position));
                    assert!(
                        anchor.distance(target) < 0.00001,
                        "{}: anchor detached",
                        clip.name
                    );
                    let expected = world.rotation * frame.rotation * euler(binding.mount.rotation);
                    let actual = transform.rotation * euler(binding.anchor.rotation);
                    for axis in [Vec3::X, Vec3::Y, Vec3::Z] {
                        assert!((actual * axis).distance(expected * axis) < 0.00001);
                    }
                    assert_eq!(transform.scale, Vec3::splat(binding.attachment.scale));
                    assert!(app.world().get::<Mesh3d>(*entity).is_some());
                }
            }
        }
        app.world_mut().despawn(entities[0]);
        app.update();
        for entity in entities {
            assert!(app.world().get_entity(entity).is_err());
        }
    }
}
