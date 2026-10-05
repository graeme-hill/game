//! Device-independent third-person movement and camera math.
use bevy::prelude::*;

pub const WALK_SPEED: f32 = 2.6;
pub const RUN_SPEED: f32 = 6.5;
pub const ARENA_LIMIT: f32 = 90.;

#[derive(Resource, Default, Clone, Copy)]
pub struct PlayerInput {
    pub movement: Vec2,
    pub look: Vec2,
    pub speed: f32,
    pub jump: bool,
    pub recenter: bool,
    pub zoom: f32,
}
#[derive(Component, Debug)]
pub struct PlayerController {
    pub position: Vec3,
    pub velocity: Vec3,
    pub facing: f32,
    pub vertical_speed: f32,
    pub grounded: bool,
    pub phase: f32,
    pub still_time: f32,
    /// standing, idle, walking, running
    pub weights: [f32; 4],
}
impl Default for PlayerController {
    fn default() -> Self {
        Self {
            position: Vec3::ZERO,
            velocity: Vec3::ZERO,
            facing: 0.,
            vertical_speed: 0.,
            grounded: true,
            phase: 0.,
            still_time: 0.,
            weights: [1., 0., 0., 0.],
        }
    }
}
#[derive(Resource)]
pub struct FollowCamera {
    pub yaw: f32,
    pub pitch: f32,
    pub distance: f32,
    pub target: Vec3,
    pub captured: bool,
}
impl Default for FollowCamera {
    fn default() -> Self {
        Self {
            yaw: 0.,
            pitch: 0.25,
            distance: 8.5,
            target: Vec3::Y * 1.6,
            captured: true,
        }
    }
}

pub fn deadzone(stick: Vec2) -> Vec2 {
    let length = stick.length().min(1.);
    if length < 0.15 {
        Vec2::ZERO
    } else {
        stick.normalize_or_zero() * ((length - 0.15) / 0.85)
    }
}
pub fn camera_relative(movement: Vec2, yaw: f32) -> Vec3 {
    Quat::from_rotation_y(yaw) * Vec3::new(movement.x, 0., -movement.y).clamp_length_max(1.)
}
pub fn locomotion_weights(speed: f32, still_time: f32) -> [f32; 4] {
    let moving = (speed / WALK_SPEED).clamp(0., 1.);
    let running = ((speed - WALK_SPEED) / (RUN_SPEED - WALK_SPEED)).clamp(0., 1.);
    let idle = ((still_time - 2.5) / 1.0).clamp(0., 1.);
    [
        (1. - moving) * (1. - idle),
        (1. - moving) * idle,
        moving * (1. - running),
        running,
    ]
}
pub fn step_player(player: &mut PlayerController, input: &PlayerInput, yaw: f32, dt: f32) {
    let dt = dt.clamp(0., 0.05);
    let desired = camera_relative(input.movement, yaw) * input.speed;
    // Finite acceleration with stronger braking makes release predictable.
    let delta = desired - player.velocity;
    player.velocity += delta.clamp_length_max(if desired.length_squared() < 0.01 {
        24. * dt
    } else {
        18. * dt
    });
    if player.velocity.length_squared() > 0.001 {
        let desired_yaw = (-player.velocity.x).atan2(-player.velocity.z);
        let delta = (desired_yaw - player.facing + std::f32::consts::PI)
            .rem_euclid(std::f32::consts::TAU)
            - std::f32::consts::PI;
        player.facing += delta * (1. - (-14. * dt).exp());
    }
    player.position += player.velocity * dt;
    player.position.x = player.position.x.clamp(-ARENA_LIMIT, ARENA_LIMIT);
    player.position.z = player.position.z.clamp(-ARENA_LIMIT, ARENA_LIMIT);
    if input.jump && player.grounded {
        player.vertical_speed = 7.5;
        player.grounded = false;
    }
    if !player.grounded {
        player.vertical_speed -= 22. * dt;
        player.position.y += player.vertical_speed * dt;
        if player.position.y <= 0. {
            player.position.y = 0.;
            player.vertical_speed = 0.;
            player.grounded = true;
        }
    }
    let speed = player.velocity.length();
    player.still_time = if speed < 0.05 {
        player.still_time + dt
    } else {
        0.
    };
    let desired_weights = locomotion_weights(speed, player.still_time);
    for (current, next) in player.weights.iter_mut().zip(desired_weights) {
        *current += (next - *current) * (1. - (-12. * dt).exp());
    }
    // Shared stride phase prevents foot phase jumps when walk/run weights change.
    let cadence = player.weights[2] / 1.0 + player.weights[3] / 0.65;
    player.phase = (player.phase + dt * cadence).rem_euclid(1.);
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn analog_deadzone_blends_and_diagonal_speed() {
        assert_eq!(deadzone(Vec2::splat(0.05)), Vec2::ZERO);
        assert!((deadzone(Vec2::X).x - 1.).abs() < 0.0001);
        let weights = locomotion_weights((WALK_SPEED + RUN_SPEED) * 0.5, 0.);
        assert!(
            weights
                .iter()
                .zip([0., 0., 0.5, 0.5])
                .all(|(a, b)| (a - b).abs() < 0.0001)
        );
        assert_eq!(locomotion_weights(0., 5.), [0., 1., 0., 0.]);
        assert!((camera_relative(Vec2::ONE, 0.).length() - 1.).abs() < 0.0001);
        assert!(
            (camera_relative(Vec2::Y, std::f32::consts::FRAC_PI_2) + Vec3::X).length() < 0.0001
        );
    }
    #[test]
    fn accelerates_brakes_jumps_and_lands_independently_of_frame_rate() {
        let simulate = |hz| {
            let mut p = PlayerController::default();
            let input = PlayerInput {
                movement: Vec2::Y,
                speed: RUN_SPEED,
                ..default()
            };
            for _ in 0..hz {
                step_player(&mut p, &input, 0., 1. / hz as f32);
            }
            assert!(p.position.z < -5. && p.weights[3] > 0.9);
            step_player(
                &mut p,
                &PlayerInput {
                    jump: true,
                    ..default()
                },
                0.,
                1. / hz as f32,
            );
            assert!(p.position.y > 0.);
            for _ in 0..hz {
                step_player(&mut p, &PlayerInput::default(), 0., 1. / hz as f32);
            }
            assert_eq!(p.position.y, 0.);
            assert!(p.grounded);
            assert!(p.velocity.length() < 0.001);
            p.position.z
        };
        assert!((simulate(30) - simulate(120)).abs() < 0.12);
    }
}
