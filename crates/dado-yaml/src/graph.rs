//! Dependency graph: insertion order, topological sort, cycle detection.

use std::collections::VecDeque;

/// A directed graph over named nodes.
#[derive(Debug, Default, Clone)]
pub struct Graph {
    names: Vec<String>,
    edges: Vec<Vec<usize>>,
}

/// Returned when the graph contains a cycle. Carries the cycle for reporting.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cycle {
    /// Node names in cycle order; the first name repeats implicitly.
    pub nodes: Vec<String>,
}

impl core::fmt::Display for Cycle {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "dependency cycle: {}", self.nodes.join(" -> "))
    }
}

impl core::error::Error for Cycle {}

impl Graph {
    /// An empty graph.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a node, returning its index. Names are not deduplicated.
    pub fn add_node(&mut self, name: impl Into<String>) -> usize {
        self.names.push(name.into());
        self.edges.push(Vec::new());
        self.names.len() - 1
    }

    /// Add an edge `from -> to` (both indices from [`Graph::add_node`]).
    pub fn add_edge(&mut self, from: usize, to: usize) {
        if from < self.edges.len() && to < self.edges.len() {
            self.edges[from].push(to);
        }
    }

    /// Node name by index.
    pub fn name(&self, index: usize) -> Option<&str> {
        self.names.get(index).map(String::as_str)
    }

    /// Number of nodes.
    pub fn len(&self) -> usize {
        self.names.len()
    }

    /// True when the graph has no nodes.
    pub fn is_empty(&self) -> bool {
        self.names.is_empty()
    }

    /// Kahn's algorithm. Returns indices in dependency order (dependencies
    /// before dependents), or the first cycle found.
    pub fn topo_sort(&self) -> Result<Vec<usize>, Cycle> {
        let n = self.names.len();
        let mut indegree = vec![0usize; n];
        for targets in &self.edges {
            for &t in targets {
                indegree[t] += 1;
            }
        }

        let mut queue: VecDeque<usize> = (0..n).filter(|&i| indegree[i] == 0).collect();
        let mut order = Vec::with_capacity(n);

        while let Some(node) = queue.pop_front() {
            order.push(node);
            for &next in &self.edges[node] {
                indegree[next] -= 1;
                if indegree[next] == 0 {
                    queue.push_back(next);
                }
            }
        }

        if order.len() == n {
            return Ok(order);
        }

        let stuck: Vec<usize> = (0..n).filter(|&i| indegree[i] > 0).collect();
        Err(Cycle {
            nodes: stuck
                .into_iter()
                .filter_map(|i| self.name(i).map(str::to_owned))
                .collect(),
        })
    }
}
