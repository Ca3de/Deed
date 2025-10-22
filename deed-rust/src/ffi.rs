//! Python FFI bindings using PyO3
//!
//! Exposes Rust core engine to Python for integration with
//! biological optimization algorithms.

use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList};
use crate::graph::Graph;
use crate::types::*;
use std::sync::Arc;
use parking_lot::RwLock;

/// Python-exposed graph database
#[pyclass]
pub struct PyDeedGraph {
    graph: Arc<RwLock<Graph>>,
}

#[pymethods]
impl PyDeedGraph {
    /// Create a new graph database
    #[new]
    fn new() -> Self {
        PyDeedGraph {
            graph: Arc::new(RwLock::new(Graph::new())),
        }
    }

    /// Add an entity
    ///
    /// Args:
    ///     entity_type (str): Collection name (e.g., "Users")
    ///     properties (dict): Property key-value pairs
    ///
    /// Returns:
    ///     int: Entity ID
    fn add_entity(&self, entity_type: String, properties: &PyDict) -> PyResult<u64> {
        let props = py_dict_to_properties(properties)?;
        let graph = self.graph.read();
        let id = graph.add_entity(entity_type, props);
        Ok(id.as_u64())
    }

    /// Get an entity by ID
    ///
    /// Args:
    ///     entity_id (int): Entity ID
    ///
    /// Returns:
    ///     dict or None: Entity properties
    fn get_entity(&self, entity_id: u64) -> PyResult<Option<PyObject>> {
        let graph = self.graph.read();
        match graph.get_entity(EntityId::new(entity_id)) {
            Some(entity) => {
                Python::with_gil(|py| {
                    let dict = PyDict::new(py);
                    dict.set_item("id", entity.id.as_u64())?;
                    dict.set_item("type", entity.entity_type)?;

                    let props_dict = PyDict::new(py);
                    for (k, v) in entity.properties {
                        let py_value = property_value_to_py(py, &v)?;
                        props_dict.set_item(k, py_value)?;
                    }
                    dict.set_item("properties", props_dict)?;

                    Ok(Some(dict.into()))
                })
            }
            None => Ok(None),
        }
    }

    /// Add an edge
    ///
    /// Args:
    ///     source_id (int): Source entity ID
    ///     target_id (int): Target entity ID
    ///     edge_type (str): Edge type (e.g., "FOLLOWS")
    ///     properties (dict): Edge properties
    ///
    /// Returns:
    ///     int or None: Edge ID
    fn add_edge(
        &self,
        source_id: u64,
        target_id: u64,
        edge_type: String,
        properties: &PyDict,
    ) -> PyResult<Option<u64>> {
        let props = py_dict_to_properties(properties)?;
        let graph = self.graph.read();

        match graph.add_edge(
            EntityId::new(source_id),
            EntityId::new(target_id),
            edge_type,
            props,
        ) {
            Some(id) => Ok(Some(id.as_u64())),
            None => Ok(None),
        }
    }

    /// Get outgoing neighbors
    ///
    /// Args:
    ///     entity_id (int): Entity ID
    ///     edge_type (str or None): Optional edge type filter
    ///
    /// Returns:
    ///     list: List of (neighbor_id, edge_id) tuples
    fn get_outgoing_neighbors(
        &self,
        entity_id: u64,
        edge_type: Option<String>,
    ) -> PyResult<Vec<(u64, u64)>> {
        let graph = self.graph.read();
        let neighbors = graph.get_outgoing_neighbors(
            EntityId::new(entity_id),
            edge_type.as_deref(),
        );

        Ok(neighbors
            .into_iter()
            .map(|(entity_id, edge_id)| (entity_id.as_u64(), edge_id.as_u64()))
            .collect())
    }

    /// Scan a collection
    ///
    /// Args:
    ///     entity_type (str): Collection name
    ///
    /// Returns:
    ///     list: List of entity dictionaries
    fn scan_collection(&self, entity_type: String) -> PyResult<Vec<PyObject>> {
        let graph = self.graph.read();
        let entities = graph.scan_collection(&entity_type);

        Python::with_gil(|py| {
            entities
                .into_iter()
                .map(|entity| {
                    let dict = PyDict::new(py);
                    dict.set_item("id", entity.id.as_u64())?;
                    dict.set_item("type", entity.entity_type)?;

                    let props_dict = PyDict::new(py);
                    for (k, v) in entity.properties {
                        let py_value = property_value_to_py(py, &v)?;
                        props_dict.set_item(k, py_value)?;
                    }
                    dict.set_item("properties", props_dict)?;

                    Ok(dict.into())
                })
                .collect()
        })
    }

    /// Evaporate pheromones (called periodically)
    fn evaporate_pheromones(&self) -> PyResult<()> {
        let graph = self.graph.read();
        graph.evaporate_pheromones();
        Ok(())
    }

    /// Get database statistics
    ///
    /// Returns:
    ///     dict: Statistics dictionary
    fn stats(&self) -> PyResult<PyObject> {
        let graph = self.graph.read();
        let stats = graph.stats();

        Python::with_gil(|py| {
            let dict = PyDict::new(py);
            dict.set_item("entity_count", stats.entity_count)?;
            dict.set_item("edge_count", stats.edge_count)?;
            dict.set_item("collection_count", stats.collection_count)?;
            dict.set_item("avg_pheromone", stats.avg_pheromone)?;

            Ok(dict.into())
        })
    }
}

// Helper functions for Python ↔ Rust conversion

fn py_dict_to_properties(dict: &PyDict) -> PyResult<Properties> {
    let mut props = Properties::new();

    for (key, value) in dict.iter() {
        let key_str: String = key.extract()?;

        let prop_value = if let Ok(i) = value.extract::<i64>() {
            PropertyValue::Int(i)
        } else if let Ok(f) = value.extract::<f64>() {
            PropertyValue::Float(f)
        } else if let Ok(s) = value.extract::<String>() {
            PropertyValue::String(s)
        } else if let Ok(b) = value.extract::<bool>() {
            PropertyValue::Bool(b)
        } else {
            PropertyValue::Null
        };

        props.insert(key_str, prop_value);
    }

    Ok(props)
}

fn property_value_to_py<'py>(py: Python<'py>, value: &PropertyValue) -> PyResult<PyObject> {
    let obj = match value {
        PropertyValue::Null => py.None(),
        PropertyValue::Bool(b) => b.into_py(py),
        PropertyValue::Int(i) => i.into_py(py),
        PropertyValue::Float(f) => f.into_py(py),
        PropertyValue::String(s) => s.into_py(py),
        PropertyValue::Bytes(b) => b.clone().into_py(py),
    };

    Ok(obj)
}

/// Python module definition
#[pymodule]
fn deed_core(_py: Python, m: &PyModule) -> PyResult<()> {
    m.add_class::<PyDeedGraph>()?;
    Ok(())
}
