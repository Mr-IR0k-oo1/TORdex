/// Trait for memory entries that can be consolidated from STM to LTM.
pub trait ConsolidationCandidate {
    fn importance(&self) -> f64;
    fn access_count(&self) -> u64;
    fn age_seconds(&self) -> i64;
    fn kind(&self) -> &str;
}
