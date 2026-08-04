//! Strongly-connected components, iteratively.
//!
//! Tarjan's algorithm, written with an explicit stack rather than recursion: the
//! natural recursion depth is the length of the longest import chain, which is a
//! property of the package being judged rather than of this tool. A gate that
//! overflows on a deep package is a gate that stops being trusted on exactly the
//! packages that need it.

use std::collections::{HashMap, HashSet};

/// Every component of two or more nodes, each sorted, in discovery order.
#[must_use]
pub(super) fn components(adjacency: &HashMap<&str, Vec<&str>>) -> Vec<Vec<String>> {
    let mut order: HashMap<&str, usize> = HashMap::new();
    let mut low: HashMap<&str, usize> = HashMap::new();
    let mut on_stack: HashSet<&str> = HashSet::new();
    let mut stack: Vec<&str> = Vec::new();
    let mut out: Vec<Vec<String>> = Vec::new();
    let mut counter = 0;

    let mut starts: Vec<&&str> = adjacency.keys().collect();
    starts.sort_unstable();
    for &start in starts {
        if order.contains_key(start) {
            continue;
        }
        let mut work: Vec<(&str, usize)> = vec![(start, 0)];
        while let Some(&(node, mut child)) = work.last() {
            let top = work.len() - 1;
            if child == 0 {
                order.insert(node, counter);
                low.insert(node, counter);
                counter += 1;
                stack.push(node);
                on_stack.insert(node);
            }
            let children = adjacency.get(node).map_or(&[][..], Vec::as_slice);
            let mut descended = false;
            while child < children.len() {
                let next = children[child];
                child += 1;
                if !order.contains_key(next) {
                    work[top] = (node, child);
                    work.push((next, 0));
                    descended = true;
                    break;
                }
                if on_stack.contains(next) {
                    let seen = order[next];
                    low.entry(node).and_modify(|l| *l = (*l).min(seen));
                }
            }
            if descended {
                continue;
            }
            work[top] = (node, child);
            work.pop();
            let settled = low[node];
            if let Some(&(parent, _)) = work.last() {
                low.entry(parent).and_modify(|l| *l = (*l).min(settled));
            }
            if settled == order[node] {
                let mut component = Vec::new();
                while let Some(popped) = stack.pop() {
                    on_stack.remove(popped);
                    component.push(popped.to_owned());
                    if popped == node {
                        break;
                    }
                }
                if component.len() > 1 {
                    component.sort();
                    out.push(component);
                }
            }
        }
    }
    out
}

#[cfg(test)]
#[allow(clippy::expect_used, reason = "a test that cannot construct its fixture has failed")]
mod tests {
    use super::*;

    fn graph(edges: &[(&'static str, &'static str)]) -> HashMap<&'static str, Vec<&'static str>> {
        let mut out: HashMap<&str, Vec<&str>> = HashMap::new();
        for &(src, dst) in edges {
            out.entry(src).or_default().push(dst);
            out.entry(dst).or_default();
        }
        out
    }

    #[test]
    fn a_line_has_no_components() {
        assert!(components(&graph(&[("a", "b"), ("b", "c")])).is_empty());
    }

    #[test]
    fn a_two_cycle_and_a_three_cycle_are_found_separately() {
        let found = components(&graph(&[
            ("a", "b"),
            ("b", "a"),
            ("x", "y"),
            ("y", "z"),
            ("z", "x"),
            ("b", "x"),
        ]));
        assert_eq!(found.len(), 2);
        assert!(found.contains(&vec!["a".to_owned(), "b".to_owned()]));
        assert!(found.contains(&vec!["x".to_owned(), "y".to_owned(), "z".to_owned()]));
    }

    #[test]
    fn a_self_loop_is_not_a_component() {
        assert!(components(&graph(&[("a", "a")])).is_empty());
    }

    #[test]
    fn a_deep_chain_does_not_overflow_the_stack() {
        let names: Vec<String> = (0..50_000).map(|i| format!("n{i}")).collect();
        let mut adjacency: HashMap<&str, Vec<&str>> = HashMap::new();
        for pair in names.windows(2) {
            adjacency.entry(&pair[0]).or_default().push(&pair[1]);
        }
        adjacency.entry(names.last().expect("non-empty").as_str()).or_default();
        assert!(components(&adjacency).is_empty());
    }
}
