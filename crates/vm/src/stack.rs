use serde::{Deserialize, Serialize};

use crate::register::RegisterValue;

/// The inference stack — a typed operand stack used during VM execution.
///
/// Supports push, pop, peek, dup, swap, and depth queries.
/// Used primarily for inference chaining and expression evaluation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InferenceStack {
    inner: Vec<RegisterValue>,
    max_depth: usize,
}

impl InferenceStack {
    pub fn new(max_depth: usize) -> Self {
        Self {
            inner: Vec::with_capacity(max_depth.min(256)),
            max_depth,
        }
    }

    pub fn push(&mut self, value: RegisterValue) -> Result<(), StackError> {
        if self.inner.len() >= self.max_depth {
            return Err(StackError::Overflow(self.max_depth));
        }
        self.inner.push(value);
        Ok(())
    }

    pub fn pop(&mut self) -> Result<RegisterValue, StackError> {
        self.inner.pop().ok_or(StackError::Underflow)
    }

    pub fn peek(&self, depth: usize) -> Result<&RegisterValue, StackError> {
        if depth >= self.inner.len() {
            return Err(StackError::Underflow);
        }
        Ok(&self.inner[self.inner.len() - 1 - depth])
    }

    pub fn dup(&mut self) -> Result<(), StackError> {
        let top = self.peek(0)?.clone();
        self.push(top)
    }

    pub fn swap(&mut self) -> Result<(), StackError> {
        let len = self.inner.len();
        if len < 2 {
            return Err(StackError::Underflow);
        }
        self.inner.swap(len - 1, len - 2);
        Ok(())
    }

    pub fn depth(&self) -> usize {
        self.inner.len()
    }

    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    pub fn clear(&mut self) {
        self.inner.clear();
    }

    pub fn drain(&mut self) -> Vec<RegisterValue> {
        self.inner.drain(..).collect()
    }
}

impl Default for InferenceStack {
    fn default() -> Self {
        Self::new(1024)
    }
}

#[derive(Debug, Clone, thiserror::Error)]
pub enum StackError {
    #[error("stack overflow: max depth {0}")]
    Overflow(usize),
    #[error("stack underflow")]
    Underflow,
}
