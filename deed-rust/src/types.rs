//! Core type definitions for Deed database

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Unique identifier for entities (nodes)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct EntityId(pub u64);

impl EntityId {
    pub fn new(id: u64) -> Self {
        EntityId(id)
    }

    pub fn as_u64(&self) -> u64 {
        self.0
    }
}

/// Unique identifier for edges (relationships)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct EdgeId(pub u64);

impl EdgeId {
    pub fn new(id: u64) -> Self {
        EdgeId(id)
    }

    pub fn as_u64(&self) -> u64 {
        self.0
    }
}

/// Property values (heterogeneous types)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum PropertyValue {
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    String(String),
    Bytes(Vec<u8>),
}

impl PropertyValue {
    pub fn as_i64(&self) -> Option<i64> {
        match self {
            PropertyValue::Int(v) => Some(*v),
            _ => None,
        }
    }

    pub fn as_f64(&self) -> Option<f64> {
        match self {
            PropertyValue::Float(v) => Some(*v),
            PropertyValue::Int(v) => Some(*v as f64),
            _ => None,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            PropertyValue::String(s) => Some(s.as_str()),
            _ => None,
        }
    }
}

/// Properties map (like row columns or node attributes)
pub type Properties = HashMap<String, PropertyValue>;

/// Entity type (collection name, like table name)
pub type EntityType = String;

/// Edge type (relationship label)
pub type EdgeType = String;

/// Pheromone strength for edges (biological optimization)
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Pheromone(pub f32);

impl Pheromone {
    pub const INITIAL: f32 = 1.0;
    pub const MIN: f32 = 0.1;
    pub const MAX: f32 = 10.0;
    pub const EVAPORATION_RATE: f32 = 0.05;

    pub fn new(value: f32) -> Self {
        Pheromone(value.clamp(Self::MIN, Self::MAX))
    }

    pub fn reinforce(&mut self, amount: f32) {
        self.0 = (self.0 + amount).clamp(Self::MIN, Self::MAX);
    }

    pub fn evaporate(&mut self) {
        self.0 = (self.0 * (1.0 - Self::EVAPORATION_RATE)).max(Self::MIN);
    }

    pub fn strength(&self) -> f32 {
        self.0
    }
}

impl Default for Pheromone {
    fn default() -> Self {
        Pheromone(Self::INITIAL)
    }
}
