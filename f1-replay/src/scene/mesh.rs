use bevy::asset::RenderAssetUsages;
use bevy::prelude::*;
use bevy::render::mesh::{Indices, PrimitiveTopology};

use super::track::P2;

/// Generates a thick polyline (ribbon) mesh from a 2D centerline.
/// `half_width` is the thickness in meters (e.g., 8.0 = 16m wide track).
/// `is_closed` connects the last point back to the first.
pub fn build_ribbon_mesh(centerline: &[P2], half_width: f32, is_closed: bool) -> Mesh {
    let n = centerline.len();
    if n < 2 {
        return Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::default(),
        );
    }

    let mut positions = Vec::with_capacity(n * 2);
    let mut normals = Vec::with_capacity(n * 2);
    let mut uvs = Vec::with_capacity(n * 2);
    let mut indices = Vec::with_capacity(n * 6);

    // Helper to get the averaged normal (perpendicular to the tangent)
    let get_normal = |i: usize| -> P2 {
        let prev = if i == 0 {
            if is_closed { n - 1 } else { 0 }
        } else {
            i - 1
        };
        let next = if i == n - 1 {
            if is_closed { 0 } else { n - 1 }
        } else {
            i + 1
        };

        let p1 = centerline[prev];
        let p2 = centerline[next];

        let dx = p2.x - p1.x;
        let dy = p2.y - p1.y;
        let len = (dx * dx + dy * dy).sqrt();
        if len < 1e-6 {
            return P2::new(0.0, 1.0);
        }

        // Rotate tangent (dx, dy) 90 degrees CCW to get the normal (-dy, dx)
        P2::new(-dy / len, dx / len)
    };

    // Generate vertices (Left and Right for each centerline point)
    for i in 0..n {
        let p = centerline[i];
        let normal = get_normal(i);

        let left = P2::new(p.x + normal.x * half_width, p.y + normal.y * half_width);
        let right = P2::new(p.x - normal.x * half_width, p.y - normal.y * half_width);

        // Bevy 2D uses Z=0 for flat things
        positions.push([left.x, left.y, 0.0]);
        positions.push([right.x, right.y, 0.0]);

        normals.push([0.0, 0.0, 1.0]); // Facing camera
        normals.push([0.0, 0.0, 1.0]);

        uvs.push([0.0, i as f32 / n as f32]);
        uvs.push([1.0, i as f32 / n as f32]);
    }

    // Generate indices (two triangles per quad)
    for i in 0..n {
        let i_next = if i == n - 1 { 0 } else { i + 1 };
        if !is_closed && i == n - 1 {
            break;
        }

        let v0 = (i * 2) as u32; // Current Left
        let v1 = (i * 2 + 1) as u32; // Current Right
        let v2 = (i_next * 2) as u32; // Next Left
        let v3 = (i_next * 2 + 1) as u32; // Next Right

        // CCW winding for Bevy's default 2D camera (looking down -Z)
        indices.push(v0);
        indices.push(v2);
        indices.push(v1);
        indices.push(v1);
        indices.push(v2);
        indices.push(v3);
    }

    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.insert_indices(Indices::U32(indices));

    mesh
}
