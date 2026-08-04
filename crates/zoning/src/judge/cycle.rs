//! Strongly-connected components, iteratively.
//!
//! Tarjan's algorithm, written with an explicit stack rather than recursion: the
//! natural recursion depth is the length of the longest import chain, which is a
//! property of the package being judged rather than of this tool. A gate that
//! overflows on a deep package is a gate that stops being trusted on exactly the
//! packages that need it.

use std::collections::{HashMap, HashSet};

use crate::pattern::Globs;
use crate::survey::Survey;

/// Every import cycle in `survey` that crosses a directory boundary.
///
/// A property of the graph and nothing else, which is why it is reachable without a
/// contract: the cycle law asks it of a governed package, and `draft` asks it of an
/// ungoverned one to find the tangles no derived contract can honestly declare away.
/// `exempt` is the facade, which sees everything by design and would otherwise appear
/// in a cycle with everything it re-exports.
///
/// Crossing a boundary is a property of the **cycle**, not of the individual imports in
/// it. Withholding same-module edges before the search instead of after severs a real
/// tangle that happens to route through two files in one directory — `a/one.zig ->
/// a/two.zig -> b/three.zig -> a/one.zig` binds `a` and `b` into one indivisible unit
/// exactly as tightly as a two-file cycle does, and reporting nothing there is how a
/// package passes while carrying the thing the law exists to forbid.
#[must_use]
pub fn tangles(survey: &Survey, exempt: &Globs) -> Vec<Vec<String>> {
    let mut adjacency: HashMap<&str, Vec<&str>> =
        survey.files.iter().map(|f| (f.as_str(), Vec::new())).collect();
    for edge in &survey.edges {
        if exempt.matches(&edge.src) {
            continue;
        }
        adjacency.entry(&edge.src).or_default().push(&edge.dst);
    }
    components(&adjacency).into_iter().filter(|c| crosses(c)).collect()
}

/// Whether a component binds more than one module together.
///
/// One unsplittable pair is enough: a cycle wholly inside a directory — or inside a
/// directory and the door file named for it — is a module being internally recursive,
/// which is its own business. Anything else has made two modules into one.
fn crosses(component: &[String]) -> bool {
    component
        .iter()
        .enumerate()
        .any(|(at, one)| component[at + 1..].iter().any(|two| !super::law::one_module(one, two)))
}

/// Every component of two or more nodes — the cycles.
#[must_use]
fn components(adjacency: &HashMap<&str, Vec<&str>>) -> Vec<Vec<String>> {
    condensation(adjacency).into_iter().filter(|c| c.len() > 1).collect()
}

/// Every strongly-connected component, dependencies before dependents.
///
/// Tarjan closes a component only after everything reachable from it, so pop order is
/// already the topological order of the condensation — which is exactly a zone stack,
/// low to high, with each unbreakable tangle appearing as one indivisible height.
/// `draft` reads it that way; the cycle law reads the same answer and keeps only the
/// components larger than one node.
#[must_use]
pub fn condensation<S: std::hash::BuildHasher>(
    adjacency: &HashMap<&str, Vec<&str>, S>,
) -> Vec<Vec<String>> {
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
                component.sort();
                out.push(component);
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

    /// What [`tangles`] keeps, without needing a survey to say it.
    fn spanning(edges: &[(&'static str, &'static str)]) -> Vec<Vec<String>> {
        components(&graph(edges)).into_iter().filter(|c| crosses(c)).collect()
    }

    #[test]
    fn a_cycle_that_detours_through_a_sibling_still_crosses_the_boundary() {
        // The shape a real tangle takes: the trip home goes through a neighbour rather
        // than straight back. `a` and `b` are bound just as tightly as in a two-file
        // cycle, and every file in the loop is part of why.
        let found = spanning(&[
            ("a/one.zig", "a/two.zig"),
            ("a/two.zig", "b/three.zig"),
            ("b/three.zig", "a/one.zig"),
        ]);
        assert_eq!(found.len(), 1, "one tangle, not none: {found:?}");
        assert_eq!(found[0].len(), 3, "and all three files are in it: {found:?}");
    }

    #[test]
    fn a_cycle_inside_one_directory_is_that_directory_s_own_business() {
        assert!(spanning(&[("a/one.zig", "a/two.zig"), ("a/two.zig", "a/one.zig")]).is_empty());
        // A door file named for the directory it fronts counts as part of it.
        assert!(spanning(&[("a/b.zig", "a/b/c.zig"), ("a/b/c.zig", "a/b.zig")]).is_empty());
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
