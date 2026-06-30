use serde::{Deserialize, Serialize};

use crate::graph::GraphSnapshot;
use crate::interval::TimeInterval;

/// A trend line describing how a metric changes over time.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrendLine {
    /// Slope (units per second).
    pub slope: f64,
    /// Intercept at the start of the measurement period.
    pub intercept: f64,
    /// Correlation coefficient (how well the linear model fits).
    pub r_squared: f64,
}

/// Summary of graph evolution over a time period.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvolutionSummary {
    /// The period analyzed.
    pub period: TimeInterval,
    /// Node count trend.
    pub node_trend: TrendLine,
    /// Edge count trend.
    pub edge_trend: TrendLine,
    /// Average node growth rate (nodes per second).
    pub node_growth_rate: f64,
    /// Average edge growth rate (edges per second).
    pub edge_growth_rate: f64,
    /// Node churn rate (nodes added + removed / total over period).
    pub node_churn_rate: f64,
    /// Edge churn rate (edges added + removed / total over period).
    pub edge_churn_rate: f64,
    /// How many nodes survived from start to end.
    pub node_survival_rate: f64,
    /// How many edges survived from start to end.
    pub edge_survival_rate: f64,
}

/// Analyze evolution of a graph across an ordered sequence of snapshots.
#[derive(Debug, Default)]
pub struct EvolutionAnalyzer;

impl EvolutionAnalyzer {
    /// Compute an evolution summary from an ordered slice of snapshots.
    #[must_use]
    pub fn analyze(snapshots: &[GraphSnapshot]) -> Option<EvolutionSummary> {
        if snapshots.len() < 2 {
            return None;
        }
        let first = snapshots.first()?;
        let last = snapshots.last()?;

        let period = TimeInterval::bounded(first.timestamp, last.timestamp);

        // Collect time series data
        let times: Vec<f64> = snapshots
            .iter()
            .map(|s| (s.timestamp - first.timestamp).whole_seconds() as f64)
            .collect();
        let node_counts: Vec<f64> = snapshots.iter().map(|s| s.node_count() as f64).collect();
        let edge_counts: Vec<f64> = snapshots.iter().map(|s| s.edge_count() as f64).collect();

        let node_trend = linear_regression(&times, &node_counts);
        let edge_trend = linear_regression(&times, &edge_counts);

        let duration_secs = (last.timestamp - first.timestamp).whole_seconds() as f64;
        let node_growth_rate = if duration_secs > 0.0 {
            (last.node_count() as f64 - first.node_count() as f64) / duration_secs
        } else {
            0.0
        };
        let edge_growth_rate = if duration_secs > 0.0 {
            (last.edge_count() as f64 - first.edge_count() as f64) / duration_secs
        } else {
            0.0
        };

        // Compute churn — total change magnitude across all diffs
        let node_churn: usize = snapshots
            .windows(2)
            .map(|w| {
                let before_ids: std::collections::HashSet<&str> =
                    w[0].nodes.iter().map(|n| n.id.as_str()).collect();
                let after_ids: std::collections::HashSet<&str> =
                    w[1].nodes.iter().map(|n| n.id.as_str()).collect();
                before_ids.symmetric_difference(&after_ids).count()
            })
            .sum();
        let edge_churn: usize = snapshots
            .windows(2)
            .map(|w| {
                let before_ids: std::collections::HashSet<&str> =
                    w[0].edges.iter().map(|e| e.id.as_str()).collect();
                let after_ids: std::collections::HashSet<&str> =
                    w[1].edges.iter().map(|e| e.id.as_str()).collect();
                before_ids.symmetric_difference(&after_ids).count()
            })
            .sum();

        let avg_total_nodes: f64 =
            node_counts.iter().sum::<f64>() / node_counts.len() as f64;
        let avg_total_edges: f64 =
            edge_counts.iter().sum::<f64>() / edge_counts.len() as f64;

        let node_churn_rate = if avg_total_nodes > 0.0 {
            node_churn as f64 / avg_total_nodes / snapshots.len() as f64
        } else {
            0.0
        };
        let edge_churn_rate = if avg_total_edges > 0.0 {
            edge_churn as f64 / avg_total_edges / snapshots.len() as f64
        } else {
            0.0
        };

        // Survival rate: fraction of nodes/edges present at start still present at end
        let start_node_ids: std::collections::HashSet<&str> =
            first.nodes.iter().map(|n| n.id.as_str()).collect();
        let end_node_ids: std::collections::HashSet<&str> =
            last.nodes.iter().map(|n| n.id.as_str()).collect();
        let node_survival_rate = if start_node_ids.is_empty() {
            1.0
        } else {
            let survived = start_node_ids.intersection(&end_node_ids).count();
            survived as f64 / start_node_ids.len() as f64
        };

        let start_edge_ids: std::collections::HashSet<&str> =
            first.edges.iter().map(|e| e.id.as_str()).collect();
        let end_edge_ids: std::collections::HashSet<&str> =
            last.edges.iter().map(|e| e.id.as_str()).collect();
        let edge_survival_rate = if start_edge_ids.is_empty() {
            1.0
        } else {
            let survived = start_edge_ids.intersection(&end_edge_ids).count();
            survived as f64 / start_edge_ids.len() as f64
        };

        Some(EvolutionSummary {
            period,
            node_trend,
            edge_trend,
            node_growth_rate,
            edge_growth_rate,
            node_churn_rate,
            edge_churn_rate,
            node_survival_rate,
            edge_survival_rate,
        })
    }
}

/// Simple linear regression (y = slope * x + intercept).
fn linear_regression(x: &[f64], y: &[f64]) -> TrendLine {
    let n = x.len() as f64;
    if n < 2.0 {
        return TrendLine {
            slope: 0.0,
            intercept: y.first().copied().unwrap_or(0.0),
            r_squared: 0.0,
        };
    }
    let sum_x: f64 = x.iter().sum();
    let sum_y: f64 = y.iter().sum();
    let sum_xy: f64 = x.iter().zip(y.iter()).map(|(&a, &b)| a * b).sum();
    let sum_xx: f64 = x.iter().map(|&a| a * a).sum();

    let denom = n * sum_xx - sum_x * sum_x;
    if denom.abs() < f64::EPSILON {
        return TrendLine {
            slope: 0.0,
            intercept: sum_y / n,
            r_squared: 0.0,
        };
    }

    let slope = (n * sum_xy - sum_x * sum_y) / denom;
    let intercept = (sum_y - slope * sum_x) / n;

    // Compute R-squared
    let y_mean = sum_y / n;
    let ss_total: f64 = y.iter().map(|&yi| (yi - y_mean).powi(2)).sum();
    let ss_residual: f64 = y
        .iter()
        .enumerate()
        .map(|(i, &yi)| {
            let y_pred = slope * x[i] + intercept;
            (yi - y_pred).powi(2)
        })
        .sum();
    let r_squared = if ss_total > 0.0 {
        1.0 - ss_residual / ss_total
    } else {
        0.0
    };

    TrendLine {
        slope,
        intercept,
        r_squared,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::{GraphSnapshot, TemporalNode};
    use crate::interval::TimeInterval;
    use std::collections::HashMap;
    use time::macros::datetime;
    use time::OffsetDateTime;

    fn snapshot(
        t: OffsetDateTime,
        node_ids: &[&str],
        edge_ids: &[&str],
    ) -> GraphSnapshot {
        let nodes: Vec<_> = node_ids
            .iter()
            .map(|id| TemporalNode {
                id: id.to_string(),
                kind: "entity".to_string(),
                label: id.to_string(),
                interval: TimeInterval::starting_at(t),
                attributes: HashMap::new(),
            })
            .collect();
        let edges = edge_ids
            .iter()
            .map(|id| crate::graph::TemporalEdge {
                id: id.to_string(),
                kind: "related".to_string(),
                source_id: "n1".to_string(),
                target_id: "n2".to_string(),
                interval: TimeInterval::starting_at(t),
                properties: HashMap::new(),
            })
            .collect();
        GraphSnapshot {
            timestamp: t,
            nodes,
            edges,
        }
    }

    #[test]
    fn fewer_than_two_snapshots_returns_none() {
        let snap = snapshot(datetime!(2024-01-01 0:00 UTC), &["n1"], &[]);
        assert!(EvolutionAnalyzer::analyze(&[snap]).is_none());
    }

    #[test]
    fn growth_rate_positive_when_nodes_increase() {
        let s1 = snapshot(datetime!(2024-01-01 0:00 UTC), &["n1"], &[]);
        let s2 = snapshot(datetime!(2024-01-02 0:00 UTC), &["n1", "n2"], &[]);
        let summary = EvolutionAnalyzer::analyze(&[s1, s2]).unwrap();
        assert!(summary.node_growth_rate > 0.0);
        assert!(summary.edge_growth_rate.abs() < f64::EPSILON);
    }

    #[test]
    fn survival_rate_partial() {
        let s1 = snapshot(datetime!(2024-01-01 0:00 UTC), &["n1", "n2"], &[]);
        let s2 = snapshot(datetime!(2024-01-02 0:00 UTC), &["n1", "n3"], &[]);
        let summary = EvolutionAnalyzer::analyze(&[s1, s2]).unwrap();
        assert!((summary.node_survival_rate - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn survival_rate_full() {
        let s1 = snapshot(datetime!(2024-01-01 0:00 UTC), &["n1", "n2"], &["e1"]);
        let s2 = snapshot(datetime!(2024-01-02 0:00 UTC), &["n1", "n2", "n3"], &["e1"]);
        let summary = EvolutionAnalyzer::analyze(&[s1, s2]).unwrap();
        assert!((summary.node_survival_rate - 1.0).abs() < f64::EPSILON);
        assert!((summary.edge_survival_rate - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn trend_line_produced() {
        let s1 = snapshot(datetime!(2024-01-01 0:00 UTC), &["n1"], &[]);
        let s2 = snapshot(datetime!(2024-01-02 0:00 UTC), &["n1", "n2"], &[]);
        let s3 = snapshot(datetime!(2024-01-03 0:00 UTC), &["n1", "n2", "n3"], &[]);
        let summary = EvolutionAnalyzer::analyze(&[s1, s2, s3]).unwrap();
        assert!(summary.node_trend.slope > 0.0);
        // Three points on a line => perfect fit
        assert!((summary.node_trend.r_squared - 1.0).abs() < 0.01);
    }

    #[test]
    fn churn_detected() {
        let s1 = snapshot(datetime!(2024-01-01 0:00 UTC), &["n1", "n2"], &[]);
        let s2 = snapshot(datetime!(2024-01-02 0:00 UTC), &["n3", "n4"], &[]);
        let summary = EvolutionAnalyzer::analyze(&[s1, s2]).unwrap();
        assert!(summary.node_churn_rate > 0.0);
    }

    #[test]
    fn edge_evolution() {
        let s1 = snapshot(datetime!(2024-01-01 0:00 UTC), &["n1"], &["e1"]);
        let s2 = snapshot(datetime!(2024-01-02 0:00 UTC), &["n1", "n2"], &["e1", "e2"]);
        let summary = EvolutionAnalyzer::analyze(&[s1, s2]).unwrap();
        assert!(summary.edge_growth_rate > 0.0);
    }
}
