/// Shared with saved-layout validation so live splits remain restorable.
pub const MAX_SPLIT_DEPTH: usize = 32;

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Axis {
    Columns,
    Rows,
}

#[derive(Clone, Copy)]
pub enum Direction {
    Left,
    Right,
    Up,
    Down,
}

/// Prefer a pane aligned with the current pane, then the closest facing edge
/// and perpendicular center. Input rectangles are [x, y, width, height].
pub fn focus_neighbor(rects: &[[f32; 4]], current: usize, direction: Direction) -> Option<usize> {
    let source = *rects.get(current)?;
    if source[2] <= 0. || source[3] <= 0. {
        return None;
    }
    let vertical = matches!(direction, Direction::Up | Direction::Down);
    let backwards = matches!(direction, Direction::Left | Direction::Up);
    let primary = usize::from(vertical);
    let secondary = 1 - primary;
    let center = |rect: [f32; 4], axis: usize| rect[axis] + rect[axis + 2] / 2.;
    rects
        .iter()
        .enumerate()
        .filter_map(|(index, &rect)| {
            if index == current || rect[2] <= 0. || rect[3] <= 0. {
                return None;
            }
            let delta = center(rect, primary) - center(source, primary);
            if (backwards && delta >= 0.) || (!backwards && delta <= 0.) {
                return None;
            }
            let overlaps = rect[secondary] < source[secondary] + source[secondary + 2]
                && source[secondary] < rect[secondary] + rect[secondary + 2];
            let gap = if backwards {
                source[primary] - rect[primary] - rect[primary + 2]
            } else {
                rect[primary] - source[primary] - source[primary + 2]
            };
            Some((
                index,
                [
                    if overlaps { 0. } else { 1. },
                    gap.max(0.),
                    (center(rect, secondary) - center(source, secondary)).abs(),
                    delta.abs(),
                ],
            ))
        })
        .min_by(|(a, left), (b, right)| {
            left.iter()
                .zip(right)
                .map(|(a, b)| a.total_cmp(b))
                .find(|order| !order.is_eq())
                .unwrap_or_else(|| a.cmp(b))
        })
        .map(|(index, _)| index)
}

/// Layout owns pane references, never terminal processes. Moving a leaf preserves
/// the same session; removing a branch collapses it around its surviving panes.
#[derive(Debug, PartialEq)]
pub enum PaneTree<T> {
    Pane(T),
    Split {
        axis: Axis,
        children: Vec<Self>,
        weights: Vec<f32>,
    },
}
impl<T: Clone + PartialEq> PaneTree<T> {
    /// Position in the depth-first order used by layout serialization.
    pub fn pane_index(&self, target: &T) -> Option<usize> {
        fn visit<T: PartialEq>(tree: &PaneTree<T>, target: &T, index: &mut usize) -> Option<usize> {
            match tree {
                PaneTree::Pane(pane) => {
                    let current = *index;
                    *index += 1;
                    (pane == target).then_some(current)
                }
                PaneTree::Split { children, .. } => children.iter().find_map(|child| visit(child, target, index)),
            }
        }
        visit(self, target, &mut 0)
    }
    /// Give siblings equal space at every split without changing topology.
    pub fn equalize(&mut self) {
        if let Self::Split {
            children, weights, ..
        } = self
        {
            weights.clear();
            weights.resize(children.len(), 1.);
            for child in children {
                child.equalize();
            }
        }
    }
    /// Exchange leaf identities without changing split geometry. Missing targets
    /// leave the entire tree untouched.
    pub fn swap(&mut self, first: &T, second: &T) -> bool {
        fn contains<T: PartialEq>(tree: &PaneTree<T>, target: &T) -> bool {
            match tree {
                PaneTree::Pane(pane) => pane == target,
                PaneTree::Split { children, .. } => {
                    children.iter().any(|child| contains(child, target))
                }
            }
        }
        fn exchange<T: Clone + PartialEq>(tree: &mut PaneTree<T>, first: &T, second: &T) {
            match tree {
                PaneTree::Pane(pane) if pane == first => *pane = second.clone(),
                PaneTree::Pane(pane) if pane == second => *pane = first.clone(),
                PaneTree::Split { children, .. } => {
                    for child in children {
                        exchange(child, first, second);
                    }
                }
                _ => {}
            }
        }
        if first == second || !contains(self, first) || !contains(self, second) {
            return false;
        }
        exchange(self, first, second);
        true
    }

    pub fn can_split(&self, target: &T) -> bool {
        fn depth<T: PartialEq>(tree: &PaneTree<T>, target: &T, level: usize) -> Option<usize> {
            match tree {
                PaneTree::Pane(pane) => (pane == target).then_some(level),
                PaneTree::Split { children, .. } => children.iter().find_map(|child| depth(child, target, level + 1)),
            }
        }
        depth(self, target, 0).is_some_and(|depth| depth < MAX_SPLIT_DEPTH)
    }

    pub fn split(&mut self, target: &T, new: T, axis: Axis) -> bool {
        match self {
            Self::Pane(pane) if pane == target => {
                *self = Self::Split {
                    axis,
                    children: vec![Self::Pane(pane.clone()), Self::Pane(new)],
                    weights: vec![1., 1.],
                };
                true
            }
            Self::Split { children, .. } => children
                .iter_mut()
                .any(|child| child.split(target, new.clone(), axis)),
            _ => false,
        }
    }

    pub fn remove(self, target: &T) -> Option<Self> {
        match self {
            Self::Pane(pane) => (pane != *target).then_some(Self::Pane(pane)),
            Self::Split {
                axis,
                children,
                weights,
            } => {
                let (mut children, weights): (Vec<_>, Vec<_>) = children
                    .into_iter()
                    .zip(weights)
                    .filter_map(|(child, weight)| child.remove(target).map(|child| (child, weight)))
                    .unzip();
                match children.len() {
                    0 => None,
                    1 => children.pop(),
                    _ => Some(Self::Split {
                        axis,
                        children,
                        weights,
                    }),
                }
            }
        }
    }

    pub fn at_mut(&mut self, path: &[usize]) -> Option<&mut Self> {
        let Some((&index, tail)) = path.split_first() else {
            return Some(self);
        };
        match self {
            Self::Split { children, .. } => children.get_mut(index)?.at_mut(tail),
            Self::Pane(_) => None,
        }
    }
}

/// Adjust only the two neighbors of a divider. Delta and minimum are in weight
/// units, converted by the UI from the split's content size at drag start.
pub fn resized_pair(left: f32, right: f32, delta: f32, minimum: f32) -> (f32, f32) {
    let total = left + right;
    let minimum = minimum.max(0.001).min(total / 2.);
    let left = (left + delta).clamp(minimum, total - minimum);
    (left, total - left)
}

#[cfg(test)]
mod tests {
    #[test]
    fn split_limit_tracks_target_depth_not_total_panes() {
        let mut tree = PaneTree::Pane(0);
        for next in 1..=MAX_SPLIT_DEPTH {
            assert!(tree.can_split(&(next - 1)));
            assert!(tree.split(&(next - 1), next, Axis::Columns));
        }
        assert!(!tree.can_split(&MAX_SPLIT_DEPTH));
        assert!(!tree.can_split(&999));
        // A shallow sibling still has room even when another branch is deep.
        assert!(tree.can_split(&0));
    }

    #[test]
    fn saved_pane_index_follows_nested_layout_after_swap_and_removal() {
        use super::{Axis, PaneTree};
        let mut tree = PaneTree::Split {
            axis: Axis::Columns,
            weights: vec![1., 2.],
            children: vec![PaneTree::Pane(10), PaneTree::Split {
                axis: Axis::Rows,
                weights: vec![1., 1.],
                children: vec![PaneTree::Pane(20), PaneTree::Pane(30)],
            }],
        };
        assert_eq!(tree.pane_index(&30), Some(2));
        assert!(tree.swap(&10, &30));
        assert_eq!(tree.pane_index(&30), Some(0));
        assert_eq!(tree.pane_index(&10), Some(2));
        let tree = tree.remove(&20).unwrap();
        assert_eq!(tree.pane_index(&10), Some(1));
        assert_eq!(tree.pane_index(&20), None);
        assert_eq!(PaneTree::Pane(10).pane_index(&10), Some(0));
    }
    #[test]
    fn equalize_preserves_nested_topology_and_leaf_order() {
        use super::{Axis, PaneTree};
        let mut tree = PaneTree::Split {
            axis: Axis::Columns,
            weights: vec![2., 7.],
            children: vec![
                PaneTree::Pane(1),
                PaneTree::Split {
                    axis: Axis::Rows,
                    weights: vec![9., 3.],
                    children: vec![PaneTree::Pane(2), PaneTree::Pane(3)],
                },
            ],
        };
        tree.equalize();
        assert_eq!(
            tree,
            PaneTree::Split {
                axis: Axis::Columns,
                weights: vec![1., 1.],
                children: vec![
                    PaneTree::Pane(1),
                    PaneTree::Split {
                        axis: Axis::Rows,
                        weights: vec![1., 1.],
                        children: vec![PaneTree::Pane(2), PaneTree::Pane(3)],
                    }
                ],
            }
        );
    }
    use super::*;
    #[test]
    fn swap_preserves_nested_geometry_and_rejects_missing_targets() {
        let mut tree = PaneTree::Pane(1);
        tree.split(&1, 2, Axis::Columns);
        tree.split(&2, 3, Axis::Rows);
        if let PaneTree::Split { weights, .. } = &mut tree {
            *weights = vec![2., 5.];
        }
        assert!(tree.swap(&1, &3));
        assert_eq!(
            tree,
            PaneTree::Split {
                axis: Axis::Columns,
                weights: vec![2., 5.],
                children: vec![
                    PaneTree::Pane(3),
                    PaneTree::Split {
                        axis: Axis::Rows,
                        weights: vec![1., 1.],
                        children: vec![PaneTree::Pane(2), PaneTree::Pane(1)]
                    }
                ]
            }
        );
        assert!(!tree.swap(&3, &99));
        assert!(!tree.swap(&3, &3));
        assert!(tree.swap(&3, &1));
        let PaneTree::Split { children, .. } = tree else {
            panic!("lost split")
        };
        assert_eq!(children[0], PaneTree::Pane(1));
    }
    #[test]
    fn directional_focus_follows_resized_nested_geometry() {
        let panes = [
            [0., 0., 300., 600.],
            [305., 0., 500., 200.],
            [305., 205., 500., 395.],
        ];
        assert_eq!(focus_neighbor(&panes, 0, Direction::Right), Some(2));
        assert_eq!(focus_neighbor(&panes, 2, Direction::Up), Some(1));
        assert_eq!(focus_neighbor(&panes, 1, Direction::Down), Some(2));
        assert_eq!(focus_neighbor(&panes, 2, Direction::Left), Some(0));
        assert_eq!(focus_neighbor(&panes, 0, Direction::Left), None);
        assert_eq!(focus_neighbor(&panes, 1, Direction::Right), None);
        assert_eq!(focus_neighbor(&[[0.; 4]], 0, Direction::Right), None);
    }
    #[test]
    fn nested_splits_and_close_preserve_surviving_panes() {
        let mut tree = PaneTree::Pane(1);
        assert!(tree.split(&1, 2, Axis::Columns));
        assert!(tree.split(&1, 3, Axis::Rows));
        assert!(!tree.split(&9, 4, Axis::Columns));
        let tree = tree.remove(&1).unwrap();
        assert_eq!(
            tree,
            PaneTree::Split {
                axis: Axis::Columns,
                children: vec![PaneTree::Pane(3), PaneTree::Pane(2)],
                weights: vec![1., 1.]
            }
        );
        let tree = tree.remove(&3).unwrap();
        assert_eq!(tree, PaneTree::Pane(2));
        assert!(tree.remove(&2).is_none());
    }

    #[test]
    fn resize_clamps_and_close_preserves_sibling_proportions() {
        assert_eq!(resized_pair(1., 1., 0.5, 0.2), (1.5, 0.5));
        assert_eq!(resized_pair(1., 1., -9., 0.25), (0.25, 1.75));
        assert_eq!(resized_pair(1., 1., 9., 8.), (1., 1.));
        let tree = PaneTree::Split {
            axis: Axis::Columns,
            children: vec![PaneTree::Pane(1), PaneTree::Pane(2), PaneTree::Pane(3)],
            weights: vec![1., 2., 3.],
        }
        .remove(&2)
        .unwrap();
        assert_eq!(
            tree,
            PaneTree::Split {
                axis: Axis::Columns,
                children: vec![PaneTree::Pane(1), PaneTree::Pane(3)],
                weights: vec![1., 3.],
            }
        );
    }
}
