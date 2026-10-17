//! Stackless, depth-first triangle BVH shared by shadows and diffuse rays.
use crate::Mesh;
use bytemuck::{Pod, Zeroable};

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub(crate) struct Triangle {
    pub points: [[f32; 4]; 3],
    pub normal: [f32; 4],
    pub color: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub(crate) struct Node {
    pub min: [f32; 4],
    pub max: [f32; 4],
    // First triangle, triangle count (0 for a branch), escape node, reserved.
    pub meta: [u32; 4],
}

#[derive(Default)]
pub(crate) struct Bvh {
    pub nodes: Vec<Node>,
    pub triangles: Vec<Triangle>,
}

impl Bvh {
    pub fn build(mesh: &Mesh, node_offset: u32, triangle_offset: u32) -> Self {
        let mut triangles: Vec<_> = mesh
            .vertices
            .as_chunks::<3>()
            .0
            .iter()
            .map(|v| Triangle {
                points: std::array::from_fn(|i| {
                    [v[i].position[0], v[i].position[1], v[i].position[2], 0.0]
                }),
                normal: [v[0].normal[0], v[0].normal[1], v[0].normal[2], 0.0],
                color: [v[0].color[0], v[0].color[1], v[0].color[2], 0.0],
            })
            .collect();
        let mut nodes = Vec::new();
        if !triangles.is_empty() {
            build(&mut triangles, &mut nodes, triangle_offset, node_offset);
        }
        Self { nodes, triangles }
    }
}

fn build(triangles: &mut [Triangle], nodes: &mut Vec<Node>, first: u32, offset: u32) {
    let mut node = Node {
        min: [f32::INFINITY; 4],
        max: [f32::NEG_INFINITY; 4],
        meta: [first, 0, 0, 0],
    };
    for t in triangles.iter() {
        for p in t.points {
            for (axis, value) in p.iter().enumerate().take(3) {
                node.min[axis] = node.min[axis].min(*value - 0.0001);
                node.max[axis] = node.max[axis].max(*value + 0.0001);
            }
        }
    }
    node.min[3] = 0.0;
    node.max[3] = 0.0;
    let index = nodes.len();
    nodes.push(node);
    if triangles.len() <= 4 {
        nodes[index].meta[1] = triangles.len() as u32;
    } else {
        let axis = (0..3)
            .max_by(|&a, &b| (node.max[a] - node.min[a]).total_cmp(&(node.max[b] - node.min[b])))
            .unwrap();
        let middle = triangles.len() / 2;
        triangles.select_nth_unstable_by(middle, |a, b| {
            let center = |t: &Triangle| t.points.iter().map(|p| p[axis]).sum::<f32>();
            center(a).total_cmp(&center(b))
        });
        let (left, right) = triangles.split_at_mut(middle);
        build(left, nodes, first, offset);
        build(right, nodes, first + middle as u32, offset);
    }
    nodes[index].meta[2] = offset + nodes.len() as u32;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escape_links_bounds_and_dynamic_offsets() {
        let mut mesh = Mesh::default();
        mesh.cuboid([1., 2., 3.], [2., 3., 4.], [0.5; 3], 0);
        let bvh = Bvh::build(&mesh, 9, 20);
        assert_eq!(bvh.triangles.len(), 12);
        assert_eq!(bvh.nodes[0].meta[2], 9 + bvh.nodes.len() as u32);
        let mut covered = vec![false; bvh.triangles.len()];
        for (i, n) in bvh.nodes.iter().enumerate() {
            assert!(n.meta[2] > 9 + i as u32);
            assert!(n.meta[2] <= 9 + bvh.nodes.len() as u32);
            for t in n.meta[0]..n.meta[0] + n.meta[1] {
                covered[(t - 20) as usize] = true;
                for p in bvh.triangles[(t - 20) as usize].points {
                    for (a, value) in p.iter().enumerate().take(3) {
                        assert!(*value >= n.min[a] && *value <= n.max[a]);
                    }
                }
            }
        }
        assert!(covered.into_iter().all(|x| x));
        assert!(Bvh::build(&Mesh::default(), 0, 0).nodes.is_empty());
    }
}
