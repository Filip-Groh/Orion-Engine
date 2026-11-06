use bytemuck::{Pod, Zeroable};

// Two vec4 slots in the scene buffer. Branches have no triangles and store the
// index immediately after their subtree. Leaves store a contiguous triangle range.
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub(super) struct BvhNode {
    min: [f32; 3],
    first_or_escape: u32,
    max: [f32; 3],
    triangle_count: u32,
}

impl BvhNode {
    pub fn new(first_or_escape: u32, triangle_count: u32) -> Self {
        Self {
            min: [f32::INFINITY; 3],
            first_or_escape,
            max: [f32::NEG_INFINITY; 3],
            triangle_count,
        }
    }

    pub fn include(&mut self, point: [f32; 3]) {
        for (axis, value) in point.into_iter().enumerate() {
            self.min[axis] = self.min[axis].min(value);
            self.max[axis] = self.max[axis].max(value);
        }
    }
}

pub(super) fn node_count(leaf_count: usize) -> usize {
    leaf_count.saturating_mul(2).saturating_sub(1)
}

pub(super) fn build(leaves: &mut [BvhNode]) -> Vec<BvhNode> {
    let mut nodes = Vec::with_capacity(node_count(leaves.len()));
    if !leaves.is_empty() {
        append_subtree(leaves, &mut nodes);
    }
    nodes
}

fn append_subtree(leaves: &mut [BvhNode], nodes: &mut Vec<BvhNode>) {
    if let [leaf] = leaves {
        nodes.push(*leaf);
        return;
    }

    let mut branch = BvhNode::new(0, 0);
    for leaf in leaves.iter() {
        branch.include(leaf.min);
        branch.include(leaf.max);
    }
    let extents = [0, 1, 2].map(|axis| f64::from(branch.max[axis]) - f64::from(branch.min[axis]));
    let axis = (0..3)
        .max_by(|&a, &b| extents[a].total_cmp(&extents[b]))
        .expect("three bounding box axes");
    let midpoint = leaves.len() / 2;
    leaves.select_nth_unstable_by(midpoint, |a, b| {
        a.min[axis]
            .midpoint(a.max[axis])
            .total_cmp(&b.min[axis].midpoint(b.max[axis]))
            .then_with(|| a.first_or_escape.cmp(&b.first_or_escape))
    });
    let (left, right) = leaves.split_at_mut(midpoint);

    let index = nodes.len();
    nodes.push(branch);
    append_subtree(left, nodes);
    append_subtree(right, nodes);
    nodes[index].first_or_escape = nodes.len() as u32;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flattened_branches_skip_their_subtrees_and_keep_triangle_ranges() {
        let mut leaves =
            [(6.0, 0, 2), (4.0, 2, 1), (2.0, 3, 4), (0.0, 7, 1)].map(|(x, first, count)| {
                let mut leaf = BvhNode::new(first, count);
                leaf.include([x, 0.0, 0.0]);
                leaf.include([x + 1.0, 1.0, 0.0]);
                leaf
            });
        let nodes = build(&mut leaves);
        assert_eq!(
            nodes
                .iter()
                .map(|node| (node.first_or_escape, node.triangle_count))
                .collect::<Vec<_>>(),
            [(7, 0), (4, 0), (7, 1), (3, 4), (7, 0), (2, 1), (0, 2)]
        );
        assert_eq!(nodes[0].min, [0.0; 3]);
        assert_eq!(nodes[0].max, [7.0, 1.0, 0.0]);
        assert!(build(&mut []).is_empty());
    }
}
