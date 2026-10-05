//! Sparse editable cubes with a transient occupancy grid for greedy surface meshing.
use crate::model::{Prop, VoxelBlock};
use bevy::{
    asset::RenderAssetUsages,
    mesh::{Indices, PrimitiveTopology},
    prelude::*,
};

pub const VOXEL_SIZE: f32 = 0.1;
const EDGE: i32 = 32;
type Cell = Option<[f32; 3]>;

pub fn paint(prop: &mut Prop, min: [i32; 3], size: u32, color: Cell) -> Result<(), String> {
    if !matches!(size, 1 | 2 | 4 | 8 | 16 | 32)
        || min.iter().any(|&v| v < 0 || v > EDGE - size as i32)
    {
        return Err("Brush must fit inside the 32³ grid and use size 1, 2, 4, 8, 16 or 32".into());
    }
    if color.is_some_and(|c| c.iter().any(|v| !v.is_finite() || !(0.0..=1.0).contains(v))) {
        return Err("Voxel colors must be finite values between zero and one".into());
    }
    let mut blocks = Vec::new();
    for block in &prop.blocks {
        subtract(block, min, size, &mut blocks);
    }
    if let Some(color) = color {
        blocks.push(VoxelBlock { min, size, color });
    }
    merge(&mut blocks);
    prop.blocks = blocks;
    Ok(())
}

fn subtract(block: &VoxelBlock, min: [i32; 3], size: u32, out: &mut Vec<VoxelBlock>) {
    if (0..3)
        .any(|a| block.min[a] >= min[a] + size as i32 || min[a] >= block.min[a] + block.size as i32)
    {
        out.push(block.clone());
        return;
    }
    if (0..3)
        .all(|a| min[a] <= block.min[a] && min[a] + size as i32 >= block.min[a] + block.size as i32)
    {
        return;
    }
    let half = block.size / 2;
    for z in 0..2 {
        for y in 0..2 {
            for x in 0..2 {
                let offset = [x, y, z];
                let child = VoxelBlock {
                    min: std::array::from_fn(|a| block.min[a] + offset[a] * half as i32),
                    size: half,
                    color: block.color,
                };
                subtract(&child, min, size, out);
            }
        }
    }
}

fn merge(blocks: &mut Vec<VoxelBlock>) {
    // Group sibling candidates once per level: repeated scans become costly after
    // many fine edits in a large cube.
    for size in [1, 2, 4, 8, 16] {
        let mut groups = std::collections::BTreeMap::new();
        for (index, block) in blocks.iter().enumerate() {
            if block.size != size || block.min.iter().any(|v| v % size as i32 != 0) {
                continue;
            }
            let parent = block.min.map(|v| v / (size as i32 * 2) * (size as i32 * 2));
            groups
                .entry((parent, block.color.map(f32::to_bits)))
                .or_insert_with(Vec::new)
                .push(index);
        }
        let mut remove = vec![false; blocks.len()];
        let mut parents = Vec::new();
        for ((min, color), siblings) in groups {
            if siblings.len() != 8 {
                continue;
            }
            for index in siblings {
                remove[index] = true;
            }
            parents.push(VoxelBlock {
                min,
                size: size * 2,
                color: color.map(f32::from_bits),
            });
        }
        let mut index = 0;
        blocks.retain(|_| {
            let keep = !remove[index];
            index += 1;
            keep
        });
        blocks.extend(parents);
    }
    blocks.sort_by_key(|b| (b.min, b.size));
}

fn index(p: [i32; 3]) -> usize {
    (p[0] + EDGE * (p[1] + EDGE * p[2])) as usize
}
fn occupancy(prop: &Prop) -> Vec<Cell> {
    let mut cells = vec![None; (EDGE * EDGE * EDGE) as usize];
    for b in &prop.blocks {
        for z in b.min[2]..b.min[2] + b.size as i32 {
            for y in b.min[1]..b.min[1] + b.size as i32 {
                for x in b.min[0]..b.min[0] + b.size as i32 {
                    let p = [x, y, z];
                    if p.iter().all(|v| (0..EDGE).contains(v)) {
                        cells[index(p)] = Some(b.color);
                    }
                }
            }
        }
    }
    cells
}
fn sample(cells: &[Cell], p: [i32; 3]) -> Cell {
    if p.iter().all(|v| (0..EDGE).contains(v)) {
        cells[index(p)]
    } else {
        None
    }
}

struct Quad {
    points: [[f32; 3]; 4],
    normal: [f32; 3],
    color: [f32; 3],
}
fn quads(cells: &[Cell]) -> Vec<Quad> {
    let mut out = Vec::new();
    for axis in 0..3 {
        let u = (axis + 1) % 3;
        let v = (axis + 2) % 3;
        for sign in [-1, 1] {
            for slice in 0..EDGE {
                let mut mask = vec![None; (EDGE * EDGE) as usize];
                for j in 0..EDGE {
                    for i in 0..EDGE {
                        let mut p = [0; 3];
                        p[axis] = slice;
                        p[u] = i;
                        p[v] = j;
                        let mut neighbor = p;
                        neighbor[axis] += sign;
                        if sample(cells, neighbor).is_none() {
                            mask[(i + EDGE * j) as usize] = sample(cells, p);
                        }
                    }
                }
                for j in 0..EDGE {
                    for i in 0..EDGE {
                        let Some(color) = mask[(i + EDGE * j) as usize] else {
                            continue;
                        };
                        let mut width = 1;
                        while i + width < EDGE
                            && mask[(i + width + EDGE * j) as usize] == Some(color)
                        {
                            width += 1;
                        }
                        let mut height = 1;
                        while j + height < EDGE
                            && (0..width).all(|dx| {
                                mask[(i + dx + EDGE * (j + height)) as usize] == Some(color)
                            })
                        {
                            height += 1;
                        }
                        for dy in 0..height {
                            for dx in 0..width {
                                mask[(i + dx + EDGE * (j + dy)) as usize] = None;
                            }
                        }
                        let corners = if sign > 0 {
                            [
                                [i, j],
                                [i + width, j],
                                [i + width, j + height],
                                [i, j + height],
                            ]
                        } else {
                            [
                                [i, j],
                                [i, j + height],
                                [i + width, j + height],
                                [i + width, j],
                            ]
                        };
                        let points = corners.map(|c| {
                            let mut p = [0.0; 3];
                            p[axis] = (slice + i32::from(sign > 0)) as f32 * VOXEL_SIZE;
                            p[u] = c[0] as f32 * VOXEL_SIZE;
                            p[v] = c[1] as f32 * VOXEL_SIZE;
                            p
                        });
                        let mut normal = [0.0; 3];
                        normal[axis] = sign as f32;
                        out.push(Quad {
                            points,
                            normal,
                            color,
                        });
                    }
                }
            }
        }
    }
    out
}

pub fn build_mesh(prop: &Prop) -> Mesh {
    build_cells_mesh(&occupancy(prop))
}

pub fn build_cells_mesh(cells: &[Option<[f32; 3]>]) -> Mesh {
    let faces = quads(cells);
    let mut positions = Vec::new();
    let mut normals = Vec::new();
    let mut colors = Vec::new();
    let mut indices = Vec::new();
    for q in faces {
        let base = positions.len() as u32;
        positions.extend(q.points);
        normals.extend([q.normal; 4]);
        let color = LinearRgba::from(Color::srgb(q.color[0], q.color[1], q.color[2]));
        colors.extend([[color.red, color.green, color.blue, 1.0]; 4]);
        indices.extend([base, base + 1, base + 2, base, base + 2, base + 3]);
    }
    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
    .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, colors)
    .with_inserted_indices(Indices::U32(indices))
}

#[derive(Debug)]
pub struct MeshStats {
    pub quad_count: usize,
    pub occupied_cells: usize,
    pub block_count: usize,
}
pub fn mesh_stats(prop: &Prop) -> MeshStats {
    let cells = occupancy(prop);
    MeshStats {
        quad_count: quads(&cells).len(),
        occupied_cells: cells.iter().filter(|c| c.is_some()).count(),
        block_count: prop.blocks.len(),
    }
}

#[derive(Debug, Clone, Copy)]
pub struct VoxelHit {
    pub cell: [i32; 3],
    pub normal: [i32; 3],
    pub distance: f32,
}
/// Origin/direction are in prop-local world units; returned distance is in world units.
pub fn raycast(prop: &Prop, origin: Vec3, direction: Vec3) -> Option<VoxelHit> {
    let direction = direction.try_normalize()?;
    if !origin.is_finite() {
        return None;
    }
    // DDA traverses at most 96 cells after entering the editing volume.
    let mut near = f32::NEG_INFINITY;
    let mut far = f32::INFINITY;
    let mut normal = [0; 3];
    for a in 0..3 {
        if direction[a].abs() < 1e-8 {
            if !(0.0..EDGE as f32 * VOXEL_SIZE).contains(&origin[a]) {
                return None;
            }
            continue;
        }
        let t0 = -origin[a] / direction[a];
        let t1 = (EDGE as f32 * VOXEL_SIZE - origin[a]) / direction[a];
        if t0.min(t1) > near {
            near = t0.min(t1);
            normal = [0; 3];
            normal[a] = if direction[a] > 0.0 { -1 } else { 1 };
        }
        far = far.min(t0.max(t1));
    }
    if far < near.max(0.0) {
        return None;
    }
    let cells = occupancy(prop);
    let mut distance = near.max(0.0);
    let start = origin + direction * (distance + 0.00001);
    let mut cell = start.to_array().map(|v| (v / VOXEL_SIZE).floor() as i32);
    for _ in 0..100 {
        if cell.iter().any(|v| !(0..EDGE).contains(v)) {
            return None;
        }
        if sample(&cells, cell).is_some() {
            return Some(VoxelHit {
                cell,
                normal,
                distance,
            });
        }
        let next: [f32; 3] = std::array::from_fn(|a| {
            if direction[a].abs() < 1e-8 {
                f32::INFINITY
            } else {
                let boundary = (cell[a] + i32::from(direction[a] > 0.0)) as f32 * VOXEL_SIZE;
                (boundary - origin[a]) / direction[a]
            }
        });
        let axis = (0..3).min_by(|&a, &b| next[a].total_cmp(&next[b]))?;
        distance = next[axis];
        if distance > far {
            return None;
        }
        let step = if direction[axis] > 0.0 { 1 } else { -1 };
        cell[axis] += step;
        normal = [0; 3];
        normal[axis] = -step;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    fn empty() -> Prop {
        Prop {
            id: 1,
            name: "Test".into(),
            blocks: vec![],
            anchors: vec![],
        }
    }
    const RED: [f32; 3] = [1.0, 0.0, 0.0];
    #[test]
    fn large_cube_is_sparse_and_six_quads() {
        let mut p = empty();
        paint(&mut p, [0; 3], 16, Some(RED)).unwrap();
        let stats = mesh_stats(&p);
        assert_eq!(
            (stats.block_count, stats.occupied_cells, stats.quad_count),
            (1, 4096, 6)
        );
    }
    #[test]
    fn adjacent_blocks_merge_surfaces_and_cull_internal_faces() {
        let mut p = empty();
        paint(&mut p, [0; 3], 8, Some(RED)).unwrap();
        paint(&mut p, [8, 0, 0], 8, Some(RED)).unwrap();
        assert_eq!(mesh_stats(&p).quad_count, 6);
        paint(&mut p, [8, 0, 0], 8, Some([0.0, 1.0, 0.0])).unwrap();
        assert_eq!(mesh_stats(&p).quad_count, 10);
    }
    #[test]
    fn fine_erase_and_repaint_restore_coarse_cube() {
        let mut p = empty();
        paint(&mut p, [0; 3], 16, Some(RED)).unwrap();
        paint(&mut p, [3, 4, 5], 1, None).unwrap();
        assert_eq!(mesh_stats(&p).occupied_cells, 4095);
        assert!(p.blocks.len() < 32);
        paint(&mut p, [3, 4, 5], 1, Some(RED)).unwrap();
        assert_eq!(p.blocks.len(), 1);
        paint(&mut p, [0; 3], 16, None).unwrap();
        assert!(p.blocks.is_empty());
    }
    #[test]
    fn bounds_fail_without_mutation() {
        let mut p = empty();
        assert!(paint(&mut p, [31; 3], 2, Some(RED)).is_err());
        assert!(paint(&mut p, [-1, 0, 0], 1, None).is_err());
        assert!(paint(&mut p, [0; 3], 3, Some(RED)).is_err());
        assert!(p.blocks.is_empty());
    }
    #[test]
    fn ray_picks_surface_cell_and_normal() {
        let mut p = empty();
        paint(&mut p, [0; 3], 16, Some(RED)).unwrap();
        let hit = raycast(&p, Vec3::new(0.55, 0.55, 3.0), Vec3::NEG_Z).unwrap();
        assert_eq!(hit.cell, [5, 5, 15]);
        assert_eq!(hit.normal, [0, 0, 1]);
        assert!((hit.distance - 1.4).abs() < 0.0001);
        assert!(raycast(&p, Vec3::new(-1.0, 0.5, 3.0), Vec3::NEG_Z).is_none());
    }
    #[test]
    fn mesh_winding_matches_normals() {
        let mut p = empty();
        paint(&mut p, [0; 3], 1, Some(RED)).unwrap();
        for q in quads(&occupancy(&p)) {
            let p = q.points.map(Vec3::from_array);
            assert!(
                (p[1] - p[0])
                    .cross(p[2] - p[0])
                    .dot(Vec3::from_array(q.normal))
                    > 0.0
            );
        }
    }
    #[test]
    fn mixed_brushes_match_dense_reference_and_survive_serialization() {
        let mut p = empty();
        let mut dense = vec![None; (EDGE * EDGE * EDGE) as usize];
        let mut seed = 0x82a9_f403_u32;
        let mut random = || {
            seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
            seed
        };
        for operation in 0..80 {
            let size = 1 << (random() % 5);
            let min = std::array::from_fn(|_| (random() % (33 - size)) as i32);
            let color = if operation % 3 == 0 {
                None
            } else if operation % 2 == 0 {
                Some(RED)
            } else {
                Some([0.0, 0.5, 1.0])
            };
            paint(&mut p, min, size, color).unwrap();
            for z in min[2]..min[2] + size as i32 {
                for y in min[1]..min[1] + size as i32 {
                    for x in min[0]..min[0] + size as i32 {
                        dense[index([x, y, z])] = color;
                    }
                }
            }
            assert_eq!(occupancy(&p), dense, "operation {operation}");
            // Stored cubes never overlap, even after unaligned coarse brush cuts.
            let volume: usize = p
                .blocks
                .iter()
                .map(|b| (b.size * b.size * b.size) as usize)
                .sum();
            assert_eq!(volume, dense.iter().filter(|c| c.is_some()).count());
            let json = serde_json::to_string(&p).unwrap();
            p = serde_json::from_str(&json).unwrap();
        }
        let faces = quads(&dense);
        let meshed_area: f32 = faces
            .iter()
            .map(|q| {
                let p = q.points.map(Vec3::from_array);
                (p[1] - p[0]).cross(p[3] - p[0]).length()
            })
            .sum();
        let mut dense_area = 0;
        for z in 0..EDGE {
            for y in 0..EDGE {
                for x in 0..EDGE {
                    let p = [x, y, z];
                    if sample(&dense, p).is_none() {
                        continue;
                    }
                    for axis in 0..3 {
                        for sign in [-1, 1] {
                            let mut neighbor = p;
                            neighbor[axis] += sign;
                            if sample(&dense, neighbor).is_none() {
                                dense_area += 1;
                            }
                        }
                    }
                }
            }
        }
        assert!((meshed_area - dense_area as f32 * VOXEL_SIZE.powi(2)).abs() < 0.01);
    }
    #[test]
    fn vertex_colors_convert_palette_srgb_to_linear() {
        let mut prop = empty();
        paint(&mut prop, [0; 3], 1, Some([0.5, 0.25, 1.0])).unwrap();
        let mesh = build_mesh(&prop);
        let Some(bevy::mesh::VertexAttributeValues::Float32x4(colors)) =
            mesh.attribute(Mesh::ATTRIBUTE_COLOR)
        else {
            panic!("Mesh must provide RGBA vertex colors");
        };
        assert_eq!(colors.len(), 24);
        for color in colors {
            assert!((color[0] - 0.21404114).abs() < 0.00001);
            assert!((color[1] - 0.05087609).abs() < 0.00001);
            assert_eq!(&color[2..], &[1.0, 1.0]);
        }
    }
}
