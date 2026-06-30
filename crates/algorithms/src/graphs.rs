use std::collections::{BinaryHeap, HashMap, HashSet, VecDeque};
use std::cmp::Reverse;

#[derive(Debug, Clone, Copy, PartialEq)]
struct TotalF64(f64);

impl Eq for TotalF64 {}

impl PartialOrd for TotalF64 {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for TotalF64 {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.0.total_cmp(&other.0)
    }
}

/// A directed or undirected graph using adjacency lists.
#[derive(Clone, Debug)]
pub struct Graph {
    adjacency: HashMap<usize, Vec<(usize, f64)>>,
    node_count: usize,
    directed: bool,
}

impl Graph {
    pub fn new(directed: bool) -> Self {
        Graph {
            adjacency: HashMap::new(),
            node_count: 0,
            directed,
        }
    }

    pub fn add_edge(&mut self, from: usize, to: usize, weight: f64) {
        self.node_count = self.node_count.max(from + 1).max(to + 1);
        self.adjacency.entry(from).or_default().push((to, weight));
        if !self.directed {
            self.adjacency.entry(to).or_default().push((from, weight));
        }
    }

    pub fn add_node(&mut self, node: usize) {
        self.node_count = self.node_count.max(node + 1);
        self.adjacency.entry(node).or_default();
    }

    pub fn neighbors(&self, node: usize) -> Vec<(usize, f64)> {
        self.adjacency.get(&node).cloned().unwrap_or_default()
    }

    pub fn nodes(&self) -> Vec<usize> {
        let mut nodes: Vec<usize> = self.adjacency.keys().copied().collect();
        nodes.sort();
        nodes
    }

    pub fn node_count(&self) -> usize {
        self.node_count.max(
            self.adjacency
                .values()
                .flat_map(|edges| edges.iter().map(|(n, _)| *n))
                .chain(self.adjacency.keys().copied())
                .max()
                .map_or(0, |m| m + 1),
        )
    }

    pub fn edge_count(&self) -> usize {
        self.adjacency.values().map(|v| v.len()).sum::<usize>()
            / if self.directed { 1 } else { 2 }
    }

    pub fn is_directed(&self) -> bool {
        self.directed
    }

    fn transpose(&self) -> Graph {
        let mut transposed = Graph::new(true);
        for (&node, edges) in &self.adjacency {
            transposed.add_node(node);
            for &(neighbor, weight) in edges {
                transposed.add_edge(neighbor, node, weight);
            }
        }
        transposed
    }
}

/// Breadth-First Search traversal.
pub fn bfs(graph: &Graph, start: usize) -> Vec<usize> {
    let mut visited = HashSet::new();
    let mut order = Vec::new();
    let mut queue = VecDeque::new();

    visited.insert(start);
    queue.push_back(start);

    while let Some(node) = queue.pop_front() {
        order.push(node);
        for (neighbor, _) in graph.neighbors(node) {
            if visited.insert(neighbor) {
                queue.push_back(neighbor);
            }
        }
    }
    order
}

/// Depth-First Search traversal.
pub fn dfs(graph: &Graph, start: usize) -> Vec<usize> {
    let mut visited = HashSet::new();
    let mut order = Vec::new();
    dfs_recursive(graph, start, &mut visited, &mut order);
    order
}

fn dfs_recursive(
    graph: &Graph,
    node: usize,
    visited: &mut HashSet<usize>,
    order: &mut Vec<usize>,
) {
    if !visited.insert(node) {
        return;
    }
    order.push(node);
    for (neighbor, _) in graph.neighbors(node) {
        dfs_recursive(graph, neighbor, visited, order);
    }
}

/// Dijkstra's shortest path algorithm.
pub fn dijkstra(graph: &Graph, start: usize) -> HashMap<usize, (f64, Option<usize>)> {
    let mut distances: HashMap<usize, (f64, Option<usize>)> = HashMap::new();
    let mut heap = BinaryHeap::new();

    distances.insert(start, (0.0, None));
    heap.push(Reverse((TotalF64(0.0), start)));

    while let Some(Reverse((dist, node))) = heap.pop() {
        if let Some(&(best, _)) = distances.get(&node) {
            if dist > TotalF64(best) {
                continue;
            }
        }
        for (neighbor, weight) in graph.neighbors(node) {
                let new_dist = dist.0 + weight;
                let is_shorter = distances
                .get(&neighbor)
                .map_or(true, |&(d, _)| new_dist < d);
            if is_shorter {
                distances.insert(neighbor, (new_dist, Some(node)));
                heap.push(Reverse((TotalF64(new_dist), neighbor)));
            }
        }
    }
    distances
}

/// Tarjan's algorithm for Strongly Connected Components.
pub fn tarjan_scc(graph: &Graph) -> Vec<Vec<usize>> {
    let nodes = graph.nodes();
    let mut index = 0usize;
    let mut indices = HashMap::new();
    let mut lowlink = HashMap::new();
    let mut on_stack = HashSet::new();
    let mut stack: Vec<usize> = Vec::new();
    let mut sccs = Vec::new();

    fn strongconnect(
        node: usize,
        graph: &Graph,
        index: &mut usize,
        indices: &mut HashMap<usize, usize>,
        lowlink: &mut HashMap<usize, usize>,
        on_stack: &mut HashSet<usize>,
        stack: &mut Vec<usize>,
        sccs: &mut Vec<Vec<usize>>,
    ) {
        indices.insert(node, *index);
        lowlink.insert(node, *index);
        *index += 1;
        stack.push(node);
        on_stack.insert(node);

        for (neighbor, _) in graph.neighbors(node) {
            if !indices.contains_key(&neighbor) {
                strongconnect(neighbor, graph, index, indices, lowlink, on_stack, stack, sccs);
                let neighbor_ll = *lowlink.get(&neighbor).unwrap_or(&usize::MAX);
                let node_ll = lowlink.get(&node).copied().unwrap();
                lowlink.insert(node, node_ll.min(neighbor_ll));
            } else if on_stack.contains(&neighbor) {
                let neighbor_idx = *indices.get(&neighbor).unwrap();
                let node_ll = lowlink.get(&node).copied().unwrap();
                lowlink.insert(node, node_ll.min(neighbor_idx));
            }
        }

        if lowlink.get(&node) == indices.get(&node) {
            let mut scc = Vec::new();
            loop {
                let w = stack.pop().unwrap();
                on_stack.remove(&w);
                scc.push(w);
                if w == node {
                    break;
                }
            }
            scc.sort();
            sccs.push(scc);
        }
    }

    for &node in &nodes {
        if !indices.contains_key(&node) {
            strongconnect(
                node,
                graph,
                &mut index,
                &mut indices,
                &mut lowlink,
                &mut on_stack,
                &mut stack,
                &mut sccs,
            );
        }
    }
    sccs
}

/// Kosaraju's algorithm for Strongly Connected Components.
pub fn kosaraju_scc(graph: &Graph) -> Vec<Vec<usize>> {
    let nodes = graph.nodes();
    let mut visited = HashSet::new();
    let mut order = Vec::new();

    fn dfs1(
        node: usize,
        graph: &Graph,
        visited: &mut HashSet<usize>,
        order: &mut Vec<usize>,
    ) {
        if !visited.insert(node) {
            return;
        }
        for (neighbor, _) in graph.neighbors(node) {
            dfs1(neighbor, graph, visited, order);
        }
        order.push(node);
    }

    for &node in &nodes {
        if !visited.contains(&node) {
            dfs1(node, graph, &mut visited, &mut order);
        }
    }

    let transposed = graph.transpose();
    let mut assigned = HashSet::new();
    let mut sccs = Vec::new();

    fn dfs2(
        node: usize,
        graph: &Graph,
        assigned: &mut HashSet<usize>,
        component: &mut Vec<usize>,
    ) {
        if !assigned.insert(node) {
            return;
        }
        component.push(node);
        for (neighbor, _) in graph.neighbors(node) {
            dfs2(neighbor, graph, assigned, component);
        }
    }

    for &node in order.iter().rev() {
        if !assigned.contains(&node) {
            let mut component = Vec::new();
            dfs2(node, &transposed, &mut assigned, &mut component);
            component.sort();
            sccs.push(component);
        }
    }
    sccs
}

/// PageRank algorithm with damping factor.
pub fn pagerank(graph: &Graph, damping: f64, iterations: usize) -> HashMap<usize, f64> {
    let nodes = graph.nodes();
    let n = nodes.len() as f64;
    if n == 0.0 {
        return HashMap::new();
    }

    let nf = n as f64;
    let mut rank: HashMap<usize, f64> = nodes.iter().map(|&n| (n, 1.0 / nf)).collect();

    for _ in 0..iterations {
        let mut new_rank: HashMap<usize, f64> = HashMap::new();
        let dangling = nodes
            .iter()
            .filter(|&&n| graph.neighbors(n).is_empty())
            .map(|&n| rank[&n])
            .sum::<f64>()
            / n;

        for &node in &nodes {
            let mut sum = 0.0;
            for (&u, edges) in &graph.adjacency {
                let out_degree = edges.len() as f64;
                if out_degree > 0.0 {
                    for &(v, _) in edges {
                        if v == node {
                            sum += rank[&u] / out_degree;
                        }
                    }
                }
            }
            let pr = (1.0 - damping) / nf + damping * (sum + dangling);
            new_rank.insert(node, pr);
        }
        rank = new_rank;
    }
    rank
}

/// A* pathfinding algorithm.
pub fn a_star(
    graph: &Graph,
    start: usize,
    goal: usize,
    heuristic: impl Fn(usize, usize) -> f64,
) -> Option<(f64, Vec<usize>)> {
    let mut open_set = BinaryHeap::new();
    let mut g_score: HashMap<usize, f64> = HashMap::new();
    let mut came_from: HashMap<usize, usize> = HashMap::new();

    g_score.insert(start, 0.0);
    let f_start = heuristic(start, goal);
    open_set.push(Reverse((TotalF64(f_start), start)));

    while let Some(Reverse((_, current))) = open_set.pop() {
        if current == goal {
            let mut path = vec![goal];
            let mut node = goal;
            while let Some(&prev) = came_from.get(&node) {
                path.push(prev);
                node = prev;
            }
            path.reverse();
            return Some((g_score[&goal], path));
        }

        let current_g = g_score[&current];
        for (neighbor, weight) in graph.neighbors(current) {
            let tentative_g = current_g + weight;
            let is_better = g_score
                .get(&neighbor)
                .map_or(true, |&g| tentative_g < g);
            if is_better {
                came_from.insert(neighbor, current);
                g_score.insert(neighbor, tentative_g);
                let f = tentative_g + heuristic(neighbor, goal);
                open_set.push(Reverse((TotalF64(f), neighbor)));
            }
        }
    }
    None
}

/// Dijkstra's shortest path returning full path and distance.
pub fn shortest_path(
    graph: &Graph,
    start: usize,
    goal: usize,
) -> Option<(f64, Vec<usize>)> {
    let result = dijkstra(graph, start);
    let (dist, _) = result.get(&goal)?;
    let mut path = vec![goal];
    let mut node = goal;
    while let Some(&(_, Some(prev))) = result.get(&node) {
        path.push(prev);
        node = prev;
        if node == start {
            break;
        }
    }
    if *path.last()? != start {
        return None;
    }
    path.reverse();
    Some((*dist, path))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_simple_graph() -> Graph {
        let mut g = Graph::new(false);
        g.add_edge(0, 1, 1.0);
        g.add_edge(0, 2, 4.0);
        g.add_edge(1, 2, 2.0);
        g.add_edge(1, 3, 5.0);
        g.add_edge(2, 3, 1.0);
        g
    }

    fn make_directed_graph() -> Graph {
        let mut g = Graph::new(true);
        g.add_edge(0, 1, 1.0);
        g.add_edge(1, 2, 1.0);
        g.add_edge(2, 0, 1.0);
        g.add_edge(2, 3, 1.0);
        g.add_edge(3, 4, 1.0);
        g.add_edge(4, 3, 1.0);
        g
    }

    #[test]
    fn bfs_traversal() {
        let g = make_simple_graph();
        let order = bfs(&g, 0);
        assert_eq!(order[0], 0);
        assert!(order.len() == 4);
    }

    #[test]
    fn dfs_traversal() {
        let g = make_simple_graph();
        let order = dfs(&g, 0);
        assert_eq!(order[0], 0);
        assert!(order.len() == 4);
    }

    #[test]
    fn dijkstra_shortest_path() {
        let g = make_simple_graph();
        let dists = dijkstra(&g, 0);
        assert!((dists[&3].0 - 4.0).abs() < 0.001);
    }

    #[test]
    fn dijkstra_path_to_self() {
        let g = make_simple_graph();
        let dists = dijkstra(&g, 0);
        assert!((dists[&0].0 - 0.0).abs() < 0.001);
    }

    #[test]
    fn tarjan_scc_directed() {
        let g = make_directed_graph();
        let sccs = tarjan_scc(&g);
        assert_eq!(sccs.len(), 2);
        assert!(sccs.iter().any(|c| c.len() >= 3));
    }

    #[test]
    fn kosaraju_scc_directed() {
        let g = make_directed_graph();
        let sccs = kosaraju_scc(&g);
        assert_eq!(sccs.len(), 2);
    }

    #[test]
    fn tarjan_scc_undirected() {
        let g = make_simple_graph();
        let sccs = tarjan_scc(&g);
        assert_eq!(sccs.len(), 1);
        assert_eq!(sccs[0].len(), 4);
    }

    #[test]
    fn pagerank_basic() {
        let mut g = Graph::new(true);
        g.add_edge(0, 1, 1.0);
        g.add_edge(0, 2, 1.0);
        g.add_edge(1, 2, 1.0);
        g.add_edge(2, 0, 1.0);
        let ranks = pagerank(&g, 0.85, 20);
        assert_eq!(ranks.len(), 3);
        let total: f64 = ranks.values().sum();
        assert!((total - 1.0).abs() < 0.01);
    }

    #[test]
    fn a_star_pathfinding() {
        let g = make_simple_graph();
        let result = a_star(&g, 0, 3, |a, b| {
            ((a as isize - b as isize).abs()) as f64
        });
        assert!(result.is_some());
        let (dist, path) = result.unwrap();
        assert!((dist - 4.0).abs() < 0.001);
        assert_eq!(path, vec![0, 1, 2, 3]);
    }

    #[test]
    fn shortest_path_function() {
        let g = make_simple_graph();
        let result = shortest_path(&g, 0, 3);
        assert!(result.is_some());
        let (dist, path) = result.unwrap();
        assert!((dist - 4.0).abs() < 0.001);
        assert_eq!(path[0], 0);
        assert_eq!(*path.last().unwrap(), 3);
    }

    #[test]
    fn disconnected_node() {
        let mut g = Graph::new(false);
        g.add_edge(0, 1, 1.0);
        g.add_node(5);
        let result = shortest_path(&g, 0, 5);
        assert!(result.is_none());
    }
}
