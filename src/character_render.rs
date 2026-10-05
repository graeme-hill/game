//! Shared editor/runtime assembly rendering; document data stays immutable.
use crate::{
    animation::{LocalPose, euler, forward_kinematics},
    body_render::{BodyMaterial, spawn_body_at},
    model::{Attachment, Body, Character, Library, Prop, attachment_transform},
    voxel,
};
use bevy::{ecs::system::SystemParam, prelude::*};

#[derive(SystemParam)]
pub struct CharacterRenderer<'w, 's> {
    pub commands: Commands<'w, 's>,
    meshes: ResMut<'w, Assets<Mesh>>,
    materials: ResMut<'w, Assets<StandardMaterial>>,
    body_materials: ResMut<'w, Assets<BodyMaterial>>,
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
        let mut attached = vec![];
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
                entities.push(entity);
                attached.push((entity, prop.clone(), attachment.clone()));
            }
        }
        self.commands.entity(root).insert(AnimatedVisual {
            body: body.clone(),
            material,
            attached,
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
    attached: Vec<(Entity, Prop, Attachment)>,
}

pub struct CharacterRenderPlugin;
impl Plugin for CharacterRenderPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            PostUpdate,
            update_visuals.before(bevy::transform::TransformSystems::Propagate),
        );
    }
}

fn update_visuals(
    mut bodies: Query<(&AnimatedVisual, &VisualPose, &mut Transform)>,
    mut props: Query<&mut Transform, Without<AnimatedVisual>>,
    mut materials: ResMut<Assets<BodyMaterial>>,
) {
    for (visual, pose, mut proxy) in &mut bodies {
        let frames = forward_kinematics(&visual.body, &pose.locals);
        let (material, transform) = BodyMaterial::posed(&visual.body, &frames, &pose.world);
        if let Some(mut current) = materials.get_mut(&visual.material) {
            *current = material;
        }
        *proxy = transform;
        for (entity, prop, attachment) in &visual.attached {
            let Some(mount) = visual.body.mounts.iter().find(|m| m.id == attachment.mount) else {
                continue;
            };
            let Some(index) = visual.body.bones.iter().position(|b| b.id == mount.bone) else {
                continue;
            };
            let Some(anchor) = prop.anchors.iter().find(|a| a.id == attachment.anchor) else {
                continue;
            };
            let frame = frames[index];
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
            if let Ok(mut transform) = props.get_mut(*entity) {
                *transform = pose.world.mul_transform(local);
            }
        }
    }
}
