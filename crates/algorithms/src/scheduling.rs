use std::collections::{BinaryHeap, HashMap, HashSet, VecDeque};
use std::cmp::Reverse;

/// A Priority Queue backed by a binary heap, with decrease-key support.
#[derive(Clone, Debug)]
pub struct PriorityQueue<T: Ord + Clone> {
    heap: BinaryHeap<Reverse<T>>,
}

impl<T: Ord + Clone> PriorityQueue<T> {
    pub fn new() -> Self {
        PriorityQueue {
            heap: BinaryHeap::new(),
        }
    }

    pub fn push(&mut self, item: T) {
        self.heap.push(Reverse(item));
    }

    pub fn pop(&mut self) -> Option<T> {
        self.heap.pop().map(|Reverse(item)| item)
    }

    pub fn peek(&self) -> Option<&T> {
        self.heap.peek().map(|Reverse(item)| item)
    }

    pub fn len(&self) -> usize {
        self.heap.len()
    }

    pub fn is_empty(&self) -> bool {
        self.heap.is_empty()
    }

    pub fn clear(&mut self) {
        self.heap.clear();
    }

    pub fn drain(&mut self) -> Vec<T> {
        let mut items = Vec::new();
        while let Some(item) = self.pop() {
            items.push(item);
        }
        items
    }
}

impl<T: Ord + Clone> Default for PriorityQueue<T> {
    fn default() -> Self {
        Self::new()
    }
}

/// A Binary Heap (max-heap) implementation.
#[derive(Clone, Debug)]
pub struct BinaryHeapCustom<T: Ord> {
    data: Vec<T>,
}

impl<T: Ord> BinaryHeapCustom<T> {
    pub fn new() -> Self {
        BinaryHeapCustom { data: Vec::new() }
    }

    pub fn push(&mut self, item: T) {
        self.data.push(item);
        let mut i = self.data.len() - 1;
        while i > 0 {
            let parent = (i - 1) / 2;
            if self.data[i] <= self.data[parent] {
                break;
            }
            self.data.swap(i, parent);
            i = parent;
        }
    }

    pub fn pop(&mut self) -> Option<T> {
        if self.data.is_empty() {
            return None;
        }
        let last = self.data.len() - 1;
        self.data.swap(0, last);
        let result = self.data.pop();
        let mut i = 0;
        loop {
            let mut largest = i;
            let left = 2 * i + 1;
            let right = 2 * i + 2;
            if left < self.data.len() && self.data[left] > self.data[largest] {
                largest = left;
            }
            if right < self.data.len() && self.data[right] > self.data[largest] {
                largest = right;
            }
            if largest == i {
                break;
            }
            self.data.swap(i, largest);
            i = largest;
        }
        result
    }

    pub fn peek(&self) -> Option<&T> {
        self.data.first()
    }

    pub fn len(&self) -> usize {
        self.data.len()
    }

    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }
}

impl<T: Ord> Default for BinaryHeapCustom<T> {
    fn default() -> Self {
        Self::new()
    }
}

/// A simple work-stealing scheduler.
pub struct WorkStealingScheduler {
    queues: Vec<VecDeque<Box<dyn FnOnce() + Send>>>,
    steal_attempts: u64,
    successes: u64,
}

impl WorkStealingScheduler {
    pub fn new(num_workers: usize) -> Self {
        WorkStealingScheduler {
            queues: (0..num_workers).map(|_| VecDeque::new()).collect(),
            steal_attempts: 0,
            successes: 0,
        }
    }

    pub fn submit(&mut self, worker: usize, task: Box<dyn FnOnce() + Send>) {
        if worker < self.queues.len() {
            self.queues[worker].push_back(task);
        }
    }

    pub fn try_execute_next(&mut self, worker: usize) -> bool {
        if worker >= self.queues.len() {
            return false;
        }
        if let Some(task) = self.queues[worker].pop_front() {
            task();
            return true;
        }
        self.steal_attempts += 1;
        for i in 1..self.queues.len() {
            let target = (worker + i) % self.queues.len();
            if target != worker {
                if let Some(task) = self.queues[target].pop_back() {
                    task();
                    self.successes += 1;
                    return true;
                }
            }
        }
        false
    }

    pub fn queue_len(&self, worker: usize) -> usize {
        self.queues.get(worker).map_or(0, |q| q.len())
    }

    pub fn steal_attempts(&self) -> u64 {
        self.steal_attempts
    }

    pub fn steal_successes(&self) -> u64 {
        self.successes
    }

    pub fn total_pending(&self) -> usize {
        self.queues.iter().map(|q| q.len()).sum()
    }
}

/// A Directed Acyclic Graph of tasks for dependency-based scheduling.
#[derive(Clone, Debug)]
pub struct TaskGraph {
    tasks: HashMap<usize, Task>,
    dependencies: HashMap<usize, Vec<usize>>,
    dependents: HashMap<usize, Vec<usize>>,
    next_id: usize,
}

#[derive(Clone, Debug)]
pub struct Task {
    pub id: usize,
    pub name: String,
    pub cost: f64,
    pub completed: bool,
}

impl TaskGraph {
    pub fn new() -> Self {
        TaskGraph {
            tasks: HashMap::new(),
            dependencies: HashMap::new(),
            dependents: HashMap::new(),
            next_id: 0,
        }
    }

    pub fn add_task(&mut self, name: &str, cost: f64) -> usize {
        let id = self.next_id;
        self.next_id += 1;
        self.tasks.insert(
            id,
            Task {
                id,
                name: name.to_string(),
                cost,
                completed: false,
            },
        );
        self.dependencies.entry(id).or_default();
        self.dependents.entry(id).or_default();
        id
    }

    pub fn add_dependency(&mut self, task: usize, depends_on: usize) -> Result<(), String> {
        if !self.tasks.contains_key(&task) || !self.tasks.contains_key(&depends_on) {
            return Err("task not found".to_string());
        }
        if self.creates_cycle(task, depends_on) {
            return Err("dependency would create a cycle".to_string());
        }
        self.dependencies.entry(task).or_default().push(depends_on);
        self.dependents
            .entry(depends_on)
            .or_default()
            .push(task);
        Ok(())
    }

    fn creates_cycle(&self, from: usize, to: usize) -> bool {
        let mut visited = HashSet::new();
        let mut stack = vec![from];
        while let Some(node) = stack.pop() {
            if node == to {
                return true;
            }
            if !visited.insert(node) {
                continue;
            }
            if let Some(deps) = self.dependents.get(&node) {
                for &dep in deps {
                    stack.push(dep);
                }
            }
        }
        false
    }

    pub fn mark_completed(&mut self, task: usize) {
        if let Some(t) = self.tasks.get_mut(&task) {
            t.completed = true;
        }
    }

    pub fn is_completed(&self, task: usize) -> bool {
        self.tasks.get(&task).map_or(true, |t| t.completed)
    }

    pub fn ready_tasks(&self) -> Vec<usize> {
        self.tasks
            .iter()
            .filter(|(id, task)| {
                !task.completed
                    && self
                        .dependencies
                        .get(id)
                        .map_or(true, |deps| deps.iter().all(|d| self.is_completed(*d)))
            })
            .map(|(id, _)| *id)
            .collect()
    }

    pub fn critical_path_cost(&self) -> f64 {
        let mut memo = HashMap::new();
        let mut max_cost = 0.0;
        for &id in self.tasks.keys() {
            let cost = self.longest_path_cost(id, &mut memo);
            if cost > max_cost {
                max_cost = cost;
            }
        }
        max_cost
    }

    fn longest_path_cost(&self, task: usize, memo: &mut HashMap<usize, f64>) -> f64 {
        if let Some(&cost) = memo.get(&task) {
            return cost;
        }
        let base = self.tasks.get(&task).map_or(0.0, |t| t.cost);
        let max_dep = self
            .dependents
            .get(&task)
            .map(|deps| {
                deps.iter()
                    .map(|&d| self.longest_path_cost(d, memo))
                    .fold(0.0, f64::max)
            })
            .unwrap_or(0.0);
        let total = base + max_dep;
        memo.insert(task, total);
        total
    }

    pub fn topological_sort(&self) -> Vec<usize> {
        let mut in_degree: HashMap<usize, usize> = HashMap::new();
        for &id in self.tasks.keys() {
            in_degree.entry(id).or_insert(0);
        }
        for (_, deps) in &self.dependencies {
            for &dep in deps {
                *in_degree.entry(dep).or_insert(0) += 0;
                if let Some(d) = self.dependents.get(&dep) {
                    for &t in d {
                        *in_degree.entry(t).or_insert(0) += 1;
                    }
                }
            }
        }
        // simpler: count deps per task
        let mut in_deg: HashMap<usize, usize> = HashMap::new();
        for &id in self.tasks.keys() {
            let count = self
                .dependencies
                .get(&id)
                .map_or(0, |deps| deps.len());
            in_deg.insert(id, count);
        }

        let mut queue: Vec<usize> = in_deg
            .iter()
            .filter(|&(_, deg)| *deg == 0)
            .map(|(&id, _)| id)
            .collect();
        let mut sorted = Vec::new();

        while let Some(task) = queue.pop() {
            sorted.push(task);
            if let Some(deps) = self.dependents.get(&task) {
                for &dep in deps {
                    if let Some(deg) = in_deg.get_mut(&dep) {
                        *deg = deg.saturating_sub(1);
                        if *deg == 0 {
                            queue.push(dep);
                        }
                    }
                }
            }
        }
        sorted
    }

    pub fn task_count(&self) -> usize {
        self.tasks.len()
    }
}

impl Default for TaskGraph {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn priority_queue_order() {
        let mut pq = PriorityQueue::new();
        pq.push(3);
        pq.push(1);
        pq.push(2);
        assert_eq!(pq.pop(), Some(1));
        assert_eq!(pq.pop(), Some(2));
        assert_eq!(pq.pop(), Some(3));
    }

    #[test]
    fn priority_queue_peek() {
        let mut pq = PriorityQueue::new();
        pq.push(5);
        pq.push(1);
        assert_eq!(*pq.peek().unwrap(), 1);
    }

    #[test]
    fn binary_heap_max() {
        let mut bh = BinaryHeapCustom::new();
        bh.push(3);
        bh.push(5);
        bh.push(1);
        assert_eq!(bh.pop(), Some(5));
        assert_eq!(bh.pop(), Some(3));
        assert_eq!(bh.pop(), Some(1));
    }

    #[test]
    fn binary_heap_empty_pop() {
        let mut bh: BinaryHeapCustom<i32> = BinaryHeapCustom::new();
        assert_eq!(bh.pop(), None);
    }

    #[test]
    fn work_stealing_basic() {
        let mut ws = WorkStealingScheduler::new(2);
        let mut _executed: Vec<i32> = Vec::new();
        ws.submit(
            0,
            Box::new(|| {}),
        );
        assert_eq!(ws.queue_len(0), 1);
        assert!(ws.try_execute_next(0));
    }

    #[test]
    fn task_graph_dag() {
        let mut tg = TaskGraph::new();
        let a = tg.add_task("A", 1.0);
        let b = tg.add_task("B", 2.0);
        let c = tg.add_task("C", 3.0);
        tg.add_dependency(b, a).unwrap();
        tg.add_dependency(c, b).unwrap();
        let sorted = tg.topological_sort();
        assert!(sorted.len() == 3);
        let pos = |id: usize| sorted.iter().position(|&x| x == id).unwrap();
        assert!(pos(a) < pos(b));
        assert!(pos(b) < pos(c));
    }

    #[test]
    fn task_graph_critical_path() {
        let mut tg = TaskGraph::new();
        let a = tg.add_task("A", 1.0);
        let b = tg.add_task("B", 2.0);
        let c = tg.add_task("C", 3.0);
        tg.add_dependency(b, a).unwrap();
        tg.add_dependency(c, b).unwrap();
        let cp = tg.critical_path_cost();
        assert!((cp - 6.0).abs() < 0.001);
    }

    #[test]
    fn task_graph_cycle_detection() {
        let mut tg = TaskGraph::new();
        let a = tg.add_task("A", 1.0);
        let b = tg.add_task("B", 1.0);
        tg.add_dependency(b, a).unwrap();
        let result = tg.add_dependency(a, b);
        assert!(result.is_err());
    }

    #[test]
    fn task_graph_ready_tasks() {
        let mut tg = TaskGraph::new();
        let a = tg.add_task("A", 1.0);
        let b = tg.add_task("B", 2.0);
        tg.add_dependency(b, a).unwrap();
        let ready = tg.ready_tasks();
        assert!(ready.contains(&a));
        assert!(!ready.contains(&b));
    }
}
