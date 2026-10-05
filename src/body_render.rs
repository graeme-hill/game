//! The mesh is only a rasterization proxy. The fragment shader renders the
//! smooth bone volume and writes depth at the actual SDF surface.
use bevy::{
    mesh::MeshVertexBufferLayoutRef,
    pbr::{MaterialPipeline, MaterialPipelineKey},
    prelude::*,
    render::render_resource::{
        AsBindGroup, Face, RenderPipelineDescriptor, ShaderType, SpecializedMeshPipelineError,
    },
    shader::ShaderRef,
};

use crate::{
    animation::{BoneFrame, forward_kinematics},
    model::{Body, BodyShape, MAX_BONES},
};

#[derive(Clone, Debug, ShaderType)]
struct BodyUniform {
    starts: [Vec4; MAX_BONES],
    ends: [Vec4; MAX_BONES],
    colors: [Vec4; MAX_BONES],
    bounds_min: Vec4,
    bounds_max: Vec4,
    // Count, smooth union radius, outside outline width (pixels), unused.
    settings: Vec4,
}

#[derive(Asset, TypePath, AsBindGroup, Clone, Debug)]
pub struct BodyMaterial {
    #[uniform(0)]
    body: BodyUniform,
}

impl Material for BodyMaterial {
    fn fragment_shader() -> ShaderRef {
        "shaders/body.wgsl".into()
    }

    fn enable_prepass() -> bool {
        false
    }

    fn enable_shadows() -> bool {
        false
    }

    fn specialize(
        _pipeline: &MaterialPipeline,
        descriptor: &mut RenderPipelineDescriptor,
        _layout: &MeshVertexBufferLayoutRef,
        _key: MaterialPipelineKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        // Exit faces also work when the camera enters the bounding proxy.
        descriptor.primitive.cull_mode = Some(Face::Front);
        Ok(())
    }
}

pub struct BodyRenderPlugin;

impl Plugin for BodyRenderPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(MaterialPlugin::<BodyMaterial>::default());
    }
}

/// Creates a body in model space at the world origin. The returned proxy's
/// transform is part of its bounds; callers rebuild it when the body changes.
pub fn spawn_body(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<BodyMaterial>,
    body: &Body,
) -> Entity {
    spawn_body_at(commands, meshes, materials, body, Vec3::ZERO)
}

/// Place the SDF and its proxy together. Shader primitives are world-space, so
/// moving only the proxy's Transform would leave the visible surface behind.
pub fn spawn_body_at(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<BodyMaterial>,
    body: &Body,
    position: Vec3,
) -> Entity {
    if body.bones.is_empty() {
        return commands
            .spawn((Name::new("Empty body"), Transform::default()))
            .id();
    }
    let frames = forward_kinematics(body, &[]);
    let (material, transform) =
        BodyMaterial::posed(body, &frames, &Transform::from_translation(position));
    commands
        .spawn((
            Name::new("Raymarched body"),
            Mesh3d(meshes.add(Cuboid::from_size(Vec3::ONE))),
            MeshMaterial3d(materials.add(material)),
            transform,
        ))
        .id()
}

impl BodyMaterial {
    /// Updates the existing GPU uniform and unit proxy without allocating meshes.
    pub fn posed(body: &Body, frames: &[BoneFrame], world: &Transform) -> (Self, Transform) {
        let mut uniform = BodyUniform {
            starts: [Vec4::ZERO; MAX_BONES],
            ends: [Vec4::ZERO; MAX_BONES],
            colors: [Vec4::ZERO; MAX_BONES],
            bounds_min: Vec4::ZERO,
            bounds_max: Vec4::ZERO,
            settings: Vec4::new(0.0, 0.14, 1.15, 0.0),
        };
        let mut lower = Vec3::splat(f32::INFINITY);
        let mut upper = Vec3::splat(f32::NEG_INFINITY);
        for (index, (bone, frame)) in body.bones.iter().zip(frames).take(MAX_BONES).enumerate() {
            let end = world.transform_point(frame.end);
            let start = match bone.shape {
                BodyShape::Sphere => end,
                BodyShape::Capsule => world.transform_point(frame.start),
            };
            let radius = bone.radius.max(0.01);
            uniform.starts[index] = start.extend(radius);
            uniform.ends[index] = end.extend(0.0);
            let color = LinearRgba::from(Color::srgb(bone.color[0], bone.color[1], bone.color[2]));
            uniform.colors[index] = Vec4::new(color.red, color.green, color.blue, 1.0);
            lower = lower.min(start.min(end) - Vec3::splat(radius));
            upper = upper.max(start.max(end) + Vec3::splat(radius));
            uniform.settings.x += 1.;
        }
        if !lower.is_finite() {
            lower = Vec3::ZERO;
            upper = Vec3::ONE;
        }
        lower -= Vec3::splat(0.2);
        upper += Vec3::splat(0.2);
        uniform.bounds_min = lower.extend(0.0);
        uniform.bounds_max = upper.extend(0.0);
        (
            Self { body: uniform },
            Transform::from_translation((lower + upper) * 0.5).with_scale(upper - lower),
        )
    }
}
