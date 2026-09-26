//! Directed-graph algorithms on dense node indices.
//!
//! All algorithms are iterative (no recursion), so deep dependency chains in large
//! repositories cannot overflow the stack, and all of them iterate nodes and edges in index
//! order, so results are deterministic.

use std::collections::VecDeque;

/// A directed graph with nodes `0..len` and sorted, de-duplicated adjacency lists.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Graph {
    adjacency: Vec<Vec<usize>>,
}

impl Graph {
    /// Creates a graph with `len` nodes and no edges.
    pub fn new(len: usize) -> Self {
        Self {
            adjacency: vec![Vec::new(); len],
        }
    }

    /// Adds the edge `from → to` (ignored if either index is out of range).
    pub fn add_edge(&mut self, from: usize, to: usize) {
        if from < self.adjacency.len() && to < self.adjacency.len() {
            self.adjacency[from].push(to);
        }
    }

    /// Sorts and de-duplicates adjacency lists. Call after adding edges.
    pub fn finish(&mut self) {
        for edges in &mut self.adjacency {
            edges.sort_unstable();
            edges.dedup();
        }
    }

    /// Number of nodes.
    pub fn len(&self) -> usize {
        self.adjacency.len()
    }

    /// Returns `true` when the graph has no nodes.
    pub fn is_empty(&self) -> bool {
        self.adjacency.is_empty()
    }

    /// Successors of `node`.
    pub fn successors(&self, node: usize) -> &[usize] {
        &self.adjacency[node]
    }
}

/// Strongly connected components (Tarjan's algorithm, iterative).
///
/// Components are returned with their members sorted, ordered by their smallest member.
pub fn strongly_connected_components(graph: &Graph) -> Vec<Vec<usize>> {
    const UNVISITED: usize = usize::MAX;
    let len = graph.len();
    let mut index = vec![UNVISITED; len];
    let mut low = vec![0usize; len];
    let mut on_stack = vec![false; len];
    let mut stack: Vec<usize> = Vec::new();
    let mut components = Vec::new();
    let mut counter = 0usize;
    // Call stack of (node, next successor position).
    let mut calls: Vec<(usize, usize)> = Vec::new();

    for root in 0..len {
        if index[root] != UNVISITED {
            continue;
        }
        calls.push((root, 0));
        index[root] = counter;
        low[root] = counter;
        counter += 1;
        stack.push(root);
        on_stack[root] = true;

        while let Some(&mut (node, ref mut position)) = calls.last_mut() {
            let successors = graph.successors(node);
            if *position < successors.len() {
                let next = successors[*position];
                *position += 1;
                if index[next] == UNVISITED {
                    index[next] = counter;
                    low[next] = counter;
                    counter += 1;
                    stack.push(next);
                    on_stack[next] = true;
                    calls.push((next, 0));
                } else if on_stack[next] {
                    low[node] = low[node].min(index[next]);
                }
                continue;
            }
            calls.pop();
            if let Some(&(parent, _)) = calls.last() {
                low[parent] = low[parent].min(low[node]);
            }
            if low[node] == index[node] {
                let mut component = Vec::new();
                while let Some(member) = stack.pop() {
                    on_stack[member] = false;
                    component.push(member);
                    if member == node {
                        break;
                    }
                }
                component.sort_unstable();
                components.push(component);
            }
        }
    }
    components.sort_by_key(|component| component[0]);
    components
}

/// Finds a shortest directed cycle through the members of one strongly connected
/// component, using breadth-first search from each of the first `max_starts` members.
/// The returned path repeats its first node at the end.
pub fn shortest_cycle(graph: &Graph, members: &[usize], max_starts: usize) -> Option<Vec<usize>> {
    let in_component = |node: usize| members.binary_search(&node).is_ok();
    let mut best: Option<Vec<usize>> = None;
    for &start in members.iter().take(max_starts.max(1)) {
        let mut parent = vec![usize::MAX; graph.len()];
        let mut queue = VecDeque::from([start]);
        let mut found = None;
        'search: while let Some(node) = queue.pop_front() {
            for &next in graph.successors(node) {
                if !in_component(next) {
                    continue;
                }
                if next == start {
                    found = Some(node);
                    break 'search;
                }
                if parent[next] == usize::MAX && next != start {
                    parent[next] = node;
                    queue.push_back(next);
                }
            }
        }
        if let Some(last) = found {
            let mut path = vec![start];
            let mut node = last;
            let mut reversed = Vec::new();
            while node != start {
                reversed.push(node);
                node = parent[node];
            }
            path.extend(reversed.into_iter().rev());
            path.push(start);
            if best
                .as_ref()
                .is_none_or(|current| path.len() < current.len())
            {
                best = Some(path);
            }
        }
    }
    best
}

/// Assigns each node a layer in the condensation DAG: nodes that depend on nothing are in
/// layer 0, and every other node is one layer above its highest dependency. Members of the
/// same strongly connected component share a layer.
pub fn layers(graph: &Graph, components: &[Vec<usize>]) -> Vec<u32> {
    let mut component_of = vec![0usize; graph.len()];
    for (index, component) in components.iter().enumerate() {
        for &node in component {
            component_of[node] = index;
        }
    }
    let count = components.len();
    let mut outgoing: Vec<Vec<usize>> = vec![Vec::new(); count];
    for node in 0..graph.len() {
        for &next in graph.successors(node) {
            let (a, b) = (component_of[node], component_of[next]);
            if a != b && !outgoing[a].contains(&b) {
                outgoing[a].push(b);
            }
        }
    }
    // Process components in reverse topological order: dependencies (sinks) first.
    let mut dependents_left = vec![0usize; count];
    let mut reverse: Vec<Vec<usize>> = vec![Vec::new(); count];
    for (from, targets) in outgoing.iter().enumerate() {
        for &to in targets {
            reverse[to].push(from);
        }
        dependents_left[from] = targets.len();
    }
    let mut layer = vec![0u32; count];
    let mut queue: VecDeque<usize> = (0..count).filter(|&c| dependents_left[c] == 0).collect();
    while let Some(component) = queue.pop_front() {
        for &dependent in &reverse[component] {
            layer[dependent] = layer[dependent].max(layer[component] + 1);
            dependents_left[dependent] -= 1;
            if dependents_left[dependent] == 0 {
                queue.push_back(dependent);
            }
        }
    }
    (0..graph.len())
        .map(|node| layer[component_of[node]])
        .collect()
}

/// Normalized betweenness centrality for an unweighted directed graph (Brandes' algorithm).
///
/// Values are divided by `(n - 1)(n - 2)`, the number of ordered pairs of other nodes, so
/// they fall between 0 and 1.
pub fn betweenness(graph: &Graph) -> Vec<f64> {
    let len = graph.len();
    let mut centrality = vec![0.0f64; len];
    if len < 3 {
        return centrality;
    }
    for source in 0..len {
        let mut stack = Vec::with_capacity(len);
        let mut predecessors: Vec<Vec<usize>> = vec![Vec::new(); len];
        let mut paths = vec![0.0f64; len];
        let mut distance = vec![-1i64; len];
        paths[source] = 1.0;
        distance[source] = 0;
        let mut queue = VecDeque::from([source]);
        while let Some(node) = queue.pop_front() {
            stack.push(node);
            for &next in graph.successors(node) {
                if distance[next] < 0 {
                    distance[next] = distance[node] + 1;
                    queue.push_back(next);
                }
                if distance[next] == distance[node] + 1 {
                    paths[next] += paths[node];
                    predecessors[next].push(node);
                }
            }
        }
        let mut dependency = vec![0.0f64; len];
        while let Some(node) = stack.pop() {
            for &predecessor in &predecessors[node] {
                dependency[predecessor] +=
                    paths[predecessor] / paths[node] * (1.0 + dependency[node]);
            }
            if node != source {
                centrality[node] += dependency[node];
            }
        }
    }
    let scale = ((len - 1) * (len - 2)) as f64;
    centrality.iter().map(|value| value / scale).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn graph(len: usize, edges: &[(usize, usize)]) -> Graph {
        let mut graph = Graph::new(len);
        for &(a, b) in edges {
            graph.add_edge(a, b);
        }
        graph.finish();
        graph
    }

    #[test]
    fn finds_strongly_connected_components() {
        // 0 → 1 → 2 → 0 is a cycle; 3 → 4 is acyclic; 5 has a self-loop-free singleton.
        let g = graph(6, &[(0, 1), (1, 2), (2, 0), (2, 3), (3, 4), (4, 4)]);
        let components = strongly_connected_components(&g);
        assert_eq!(components, vec![vec![0, 1, 2], vec![3], vec![4], vec![5]]);
    }

    #[test]
    fn handles_deep_chains_without_recursion() {
        let len = 200_000;
        let edges: Vec<_> = (0..len - 1).map(|i| (i, i + 1)).collect();
        let g = graph(len, &edges);
        assert_eq!(strongly_connected_components(&g).len(), len);
    }

    #[test]
    fn finds_shortest_cycles() {
        // Two cycles through 0: 0→1→2→3→0 (length 4) and 0→4→0 (length 2).
        let g = graph(5, &[(0, 1), (1, 2), (2, 3), (3, 0), (0, 4), (4, 0)]);
        let components = strongly_connected_components(&g);
        let cycle = shortest_cycle(&g, &components[0], 10).unwrap();
        assert_eq!(cycle, vec![0, 4, 0]);
        let acyclic = graph(2, &[(0, 1)]);
        assert!(shortest_cycle(&acyclic, &[0], 10).is_none());
    }

    #[test]
    fn layers_follow_dependency_depth() {
        // app → service → repo; app → util; cycle a ↔ b depends on repo.
        let g = graph(6, &[(0, 1), (1, 2), (0, 3), (4, 5), (5, 4), (4, 2)]);
        let components = strongly_connected_components(&g);
        let layer = layers(&g, &components);
        assert_eq!(layer[2], 0);
        assert_eq!(layer[3], 0);
        assert_eq!(layer[1], 1);
        assert_eq!(layer[0], 2);
        assert_eq!(layer[4], layer[5]);
        assert_eq!(layer[4], 1);
    }

    #[test]
    fn betweenness_of_a_path_graph() {
        // 0 → 1 → 2: node 1 lies on the only path between 0 and 2.
        let g = graph(3, &[(0, 1), (1, 2)]);
        let values = betweenness(&g);
        assert_eq!(values, vec![0.0, 0.5, 0.0]);
        // Star into a hub that fans out: hub carries every path.
        let star = graph(5, &[(0, 4), (1, 4), (4, 2), (4, 3)]);
        let centrality = betweenness(&star);
        assert!((centrality[4] - 4.0 / 12.0).abs() < 1e-9);
        assert!(centrality[..4].iter().all(|&c| c == 0.0));
    }
}
