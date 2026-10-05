//! Static piece collision and device-independent character movement.
use crate::tiles::World;
use crate::{
    controller::{PlayerController, PlayerInput, step_player},
    tiles::*,
};
use bevy::prelude::*;
#[derive(Resource, Default, Clone)]
pub struct TerrainCollision {
    pub boxes: Vec<Bounds>,
    oriented: Vec<OrientedBox>,
}
impl TerrainCollision {
    pub fn from_world(w: &World, ts: &[Tile]) -> Result<Self, String> {
        let mut boxes = vec![];
        let mut oriented = vec![];
        for i in &w.instances {
            for b in &tile(ts, i.tile)?.colliders {
                boxes.push(world_bounds(i, b));
                oriented.push(OrientedBox::new(i, b));
            }
        }
        Ok(Self { boxes, oriented })
    }
    pub fn ground(&self, p: Vec3, ceiling: f32, radius: f32) -> Option<f32> {
        if !self.oriented.is_empty() {
            return self
                .oriented
                .iter()
                .filter_map(|b| {
                    if p.x + radius <= b.bounds.min[0]
                        || p.x - radius >= b.bounds.max[0]
                        || p.z + radius <= b.bounds.min[2]
                        || p.z - radius >= b.bounds.max[2]
                    {
                        return None;
                    }
                    [
                        (0., 0.),
                        (-radius, 0.),
                        (radius, 0.),
                        (0., -radius),
                        (0., radius),
                    ]
                    .into_iter()
                    .filter_map(|(x, z)| {
                        let origin = Vec3::new(p.x + x, ceiling + 0.001, p.z + z) - b.center;
                        let mut near = 0f32;
                        let mut far = f32::INFINITY;
                        for a in 0..3 {
                            let o = origin.dot(b.axes[a]);
                            let d = (-Vec3::Y).dot(b.axes[a]);
                            if d.abs() < 1e-6 {
                                if o.abs() > b.half[a] {
                                    return None;
                                }
                            } else {
                                let u = (-b.half[a] - o) / d;
                                let v = (b.half[a] - o) / d;
                                near = near.max(u.min(v));
                                far = far.min(u.max(v));
                            }
                        }
                        (near <= far && far >= 0.).then_some(ceiling + 0.001 - near)
                    })
                    .max_by(f32::total_cmp)
                })
                .max_by(f32::total_cmp);
        }
        self.boxes
            .iter()
            .filter(|b| {
                b.max[1] <= ceiling + 0.001
                    && p.x + radius > b.min[0]
                    && p.x - radius < b.max[0]
                    && p.z + radius > b.min[2]
                    && p.z - radius < b.max[2]
            })
            .map(|b| b.max[1])
            .max_by(f32::total_cmp)
    }
    fn blocked(&self, p: Vec3, r: f32, h: f32) -> bool {
        let body = Bounds {
            min: [p.x - r, p.y + 0.005, p.z - r],
            max: [p.x + r, p.y + h, p.z + r],
        };
        if self.oriented.is_empty() {
            return self.boxes.iter().any(|b| intersects(&body, b));
        }
        let shape = OrientedBox::new(
            &Instance {
                id: 0,
                tile: 0,
                position: [0.; 3],
                rotation: identity(),
            },
            &body,
        );
        self.oriented.iter().any(|b| shape.overlaps(b))
    }
    pub fn camera_position(&self, target: Vec3, desired: Vec3) -> Vec3 {
        let delta = desired - target;
        let distance = delta.length();
        let steps = (distance / 0.08).ceil() as usize;
        for n in 1..=steps {
            let p = target + delta * (n as f32 / steps as f32);
            if self.blocked(p - Vec3::Y * 0.1, 0.12, 0.2) {
                return target + delta * ((n.saturating_sub(2)) as f32 / steps as f32);
            }
        }
        desired
    }
}
pub fn step_on_terrain(
    player: &mut PlayerController,
    input: &PlayerInput,
    yaw: f32,
    dt: f32,
    terrain: &TerrainCollision,
    height: f32,
) {
    let dt = dt.clamp(0., 0.05);
    let start = player.position;
    let grounded = player.grounded;
    let vertical = player.vertical_speed;
    step_player(player, input, yaw, dt); // Reuse acceleration, facing, stride and animation blending.
    player.position = start;
    player.grounded = grounded;
    player.vertical_speed = vertical;
    if input.jump && grounded {
        player.vertical_speed = 7.5;
        player.grounded = false;
    }
    let steps = ((player.velocity.length() + player.vertical_speed.abs()) * dt / 0.08)
        .ceil()
        .max(1.) as usize;
    let sub = dt / steps as f32;
    let radius = 0.26;
    for _ in 0..steps {
        for axis in [0, 2] {
            let mut p = player.position;
            p[axis] += player.velocity[axis] * sub;
            if terrain.blocked(p, radius, height) {
                let mut step = p;
                step.y += 0.25;
                if player.grounded
                    && !terrain.blocked(step, radius, height)
                    && let Some(y) = terrain.ground(step, step.y, radius)
                {
                    step.y = y;
                    if !terrain.blocked(step, radius, height) {
                        player.position = step;
                    }
                }
            } else {
                player.position = p;
            }
        }
        if player.grounded {
            if let Some(y) = terrain.ground(player.position, player.position.y + 0.02, radius)
                && player.position.y - y < 0.06
            {
                player.position.y = y;
            } else {
                player.grounded = false;
            }
        }
        if !player.grounded {
            player.vertical_speed -= 22. * sub;
            let old_y = player.position.y;
            let mut p = player.position;
            p.y += player.vertical_speed * sub;
            if player.vertical_speed <= 0.
                && let Some(y) = terrain.ground(p, old_y + 0.002, radius)
                && p.y <= y
            {
                p.y = y;
                player.vertical_speed = 0.;
                player.grounded = true;
            } else if player.vertical_speed > 0. && terrain.blocked(p, radius, height) {
                player.vertical_speed = 0.;
                p.y = old_y;
            }
            player.position = p;
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn curb_wall_and_landing() {
        let t = TerrainCollision {
            oriented: vec![],
            boxes: vec![
                Bounds {
                    min: [-10., -1., -10.],
                    max: [0., 0., 10.],
                },
                Bounds {
                    min: [0., -1., -10.],
                    max: [10., 0.2, 10.],
                },
                Bounds {
                    min: [2., 0.2, -10.],
                    max: [2.2, 4., 10.],
                },
            ],
        };
        let mut p = PlayerController {
            position: Vec3::new(-1., 0., 0.),
            ..default()
        };
        let input = PlayerInput {
            movement: Vec2::X,
            speed: 2.6,
            ..default()
        };
        for _ in 0..120 {
            step_on_terrain(&mut p, &input, 0., 1. / 60., &t, 1.8);
        }
        assert!(p.position.x > 1. && p.position.x < 1.75);
        assert!((p.position.y - 0.2).abs() < 0.01);
        step_on_terrain(
            &mut p,
            &PlayerInput {
                jump: true,
                ..default()
            },
            0.,
            1. / 60.,
            &t,
            1.8,
        );
        assert!(p.position.y > 0.2);
        for _ in 0..120 {
            step_on_terrain(&mut p, &PlayerInput::default(), 0., 1. / 60., &t, 1.8);
        }
        assert!(p.grounded);
        assert!((p.position.y - 0.2).abs() < 0.01);
    }
}
