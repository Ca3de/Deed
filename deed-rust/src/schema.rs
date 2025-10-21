//! Schema System for Deed Database
//!
//! Provides optional schema enforcement for collections.
//! Collections can be schema-less (default) or schema-enforced.

use crate::types::{PropertyValue, Properties};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

/// Schema definition for a collection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Schema {
    pub collection: String,
    pub fields: Vec<Field>,
    pub allow_extra_properties: bool,
    pub indexes: Vec<String>, // Indexed field names
}

impl Schema {
    pub fn new(collection: String) -> Self {
        Schema {
            collection,
            fields: Vec::new(),
            allow_extra_properties: false,
            indexes: Vec::new(),
        }
    }

    /// Add a field to the schema
    pub fn add_field(&mut self, field: Field) {
        // Auto-add index for PRIMARY KEY and UNIQUE fields
        if field.constraints.contains(&Constraint::PrimaryKey)
            || field.constraints.contains(&Constraint::Unique)
        {
            self.indexes.push(field.name.clone());
        }

        if field.constraints.contains(&Constraint::Index) {
            self.indexes.push(field.name.clone());
        }

        self.fields.push(field);
    }

    /// Get field definition by name
    pub fn get_field(&self, name: &str) -> Option<&Field> {
        self.fields.iter().find(|f| f.name == name)
    }

    /// Check if field has a constraint
    pub fn has_constraint(&self, field_name: &str, constraint: &Constraint) -> bool {
        if let Some(field) = self.get_field(field_name) {
            field.constraints.contains(constraint)
        } else {
            false
        }
    }
}

/// Field definition within a schema
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Field {
    pub name: String,
    pub field_type: FieldType,
    pub constraints: Vec<Constraint>,
    pub check_expression: Option<String>, // For CHECK constraints
}

impl Field {
    pub fn new(name: String, field_type: FieldType) -> Self {
        Field {
            name,
            field_type,
            constraints: Vec::new(),
            check_expression: None,
        }
    }

    pub fn with_constraint(mut self, constraint: Constraint) -> Self {
        self.constraints.push(constraint);
        self
    }

    pub fn with_check(mut self, expression: String) -> Self {
        self.check_expression = Some(expression);
        self.constraints.push(Constraint::Check);
        self
    }

    /// Check if field has a specific constraint
    pub fn has_constraint(&self, constraint: &Constraint) -> bool {
        self.constraints.contains(constraint)
    }

    /// Get default value if specified
    pub fn get_default(&self) -> Option<&PropertyValue> {
        self.constraints.iter().find_map(|c| match c {
            Constraint::Default(value) => Some(value),
            _ => None,
        })
    }
}

/// Supported field types
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum FieldType {
    String,
    Integer,
    Float,
    Boolean,
    Timestamp,
    Bytes,
    Json,
    Array(Box<FieldType>),
}

impl FieldType {
    /// Check if a PropertyValue matches this type
    pub fn matches(&self, value: &PropertyValue) -> bool {
        match (self, value) {
            (FieldType::String, PropertyValue::String(_)) => true,
            (FieldType::Integer, PropertyValue::Int(_)) => true,
            (FieldType::Float, PropertyValue::Float(_)) => true,
            (FieldType::Boolean, PropertyValue::Bool(_)) => true,
            (FieldType::Bytes, PropertyValue::Bytes(_)) => true,
            // Allow int for float (coercion)
            (FieldType::Float, PropertyValue::Int(_)) => true,
            // Null matches any type (unless NOT NULL constraint)
            (_, PropertyValue::Null) => true,
            _ => false,
        }
    }

    /// Get type name for error messages
    pub fn name(&self) -> String {
        match self {
            FieldType::String => "String".to_string(),
            FieldType::Integer => "Integer".to_string(),
            FieldType::Float => "Float".to_string(),
            FieldType::Boolean => "Boolean".to_string(),
            FieldType::Timestamp => "Timestamp".to_string(),
            FieldType::Bytes => "Bytes".to_string(),
            FieldType::Json => "Json".to_string(),
            FieldType::Array(inner) => format!("Array<{}>", inner.name()),
        }
    }
}

/// Field constraints
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Constraint {
    NotNull,
    Unique,
    Default(PropertyValue),
    Check, // Expression stored in Field.check_expression
    PrimaryKey,
    Index,
}

/// Schema validation errors
#[derive(Debug, Clone)]
pub enum ValidationError {
    FieldRequired(String),
    TypeMismatch {
        field: String,
        expected: String,
        got: String,
    },
    UniqueViolation(String),
    CheckFailed {
        field: String,
        expression: String,
    },
    UnknownField(String),
}

impl std::fmt::Display for ValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ValidationError::FieldRequired(field) => write!(f, "Field '{}' is required", field),
            ValidationError::TypeMismatch {
                field,
                expected,
                got,
            } => write!(
                f,
                "Type mismatch for '{}': expected {}, got {}",
                field, expected, got
            ),
            ValidationError::UniqueViolation(field) => {
                write!(f, "UNIQUE constraint violation on '{}'", field)
            }
            ValidationError::CheckFailed { field, expression } => {
                write!(f, "CHECK constraint failed for '{}': {}", field, expression)
            }
            ValidationError::UnknownField(field) => {
                write!(f, "Unknown field '{}' (schema doesn't allow extra properties)", field)
            }
        }
    }
}

/// Schema validator
pub struct SchemaValidator {
    schemas: HashMap<String, Schema>,
}

impl SchemaValidator {
    pub fn new() -> Self {
        SchemaValidator {
            schemas: HashMap::new(),
        }
    }

    /// Register a schema for a collection
    pub fn register_schema(&mut self, schema: Schema) {
        self.schemas.insert(schema.collection.clone(), schema);
    }

    /// Get schema for a collection
    pub fn get_schema(&self, collection: &str) -> Option<&Schema> {
        self.schemas.get(collection)
    }

    /// Remove schema for a collection (make it schema-less)
    pub fn drop_schema(&mut self, collection: &str) -> Option<Schema> {
        self.schemas.remove(collection)
    }

    /// Check if collection has a schema
    pub fn has_schema(&self, collection: &str) -> bool {
        self.schemas.contains_key(collection)
    }

    /// Validate properties against schema (for INSERT)
    pub fn validate_insert(
        &self,
        collection: &str,
        properties: &Properties,
    ) -> Result<(), ValidationError> {
        if let Some(schema) = self.get_schema(collection) {
            self.validate_against_schema(schema, properties)
        } else {
            // No schema - allow anything
            Ok(())
        }
    }

    /// Validate updates against schema
    pub fn validate_update(
        &self,
        collection: &str,
        updates: &HashMap<String, PropertyValue>,
    ) -> Result<(), ValidationError> {
        if let Some(schema) = self.get_schema(collection) {
            // Check each update field
            for (field_name, value) in updates {
                if let Some(field) = schema.get_field(field_name) {
                    // Check type
                    if !field.field_type.matches(value) {
                        return Err(ValidationError::TypeMismatch {
                            field: field_name.clone(),
                            expected: field.field_type.name(),
                            got: self.value_type_name(value),
                        });
                    }

                    // Check NOT NULL
                    if field.has_constraint(&Constraint::NotNull)
                        && matches!(value, PropertyValue::Null)
                    {
                        return Err(ValidationError::FieldRequired(field_name.clone()));
                    }

                    // Check CHECK constraint (simplified - would need expression evaluator)
                    if field.has_constraint(&Constraint::Check) {
                        if let Some(expr) = &field.check_expression {
                            if !self.evaluate_check(value, expr) {
                                return Err(ValidationError::CheckFailed {
                                    field: field_name.clone(),
                                    expression: expr.clone(),
                                });
                            }
                        }
                    }
                } else if !schema.allow_extra_properties {
                    return Err(ValidationError::UnknownField(field_name.clone()));
                }
            }
            Ok(())
        } else {
            // No schema - allow anything
            Ok(())
        }
    }

    /// Apply default values to properties
    pub fn apply_defaults(&self, collection: &str, properties: &mut Properties) {
        if let Some(schema) = self.get_schema(collection) {
            for field in &schema.fields {
                // If field not present and has default, add it
                if !properties.contains_key(&field.name) {
                    if let Some(default) = field.get_default() {
                        properties.insert(field.name.clone(), default.clone());
                    }
                }
            }
        }
    }

    /// Validate properties against a specific schema
    fn validate_against_schema(
        &self,
        schema: &Schema,
        properties: &Properties,
    ) -> Result<(), ValidationError> {
        // Check all required fields are present
        for field in &schema.fields {
            if field.has_constraint(&Constraint::NotNull)
                || field.has_constraint(&Constraint::PrimaryKey)
            {
                if !properties.contains_key(&field.name) {
                    // Check if has default
                    if field.get_default().is_none() {
                        return Err(ValidationError::FieldRequired(field.name.clone()));
                    }
                }
            }
        }

        // Check all provided properties
        for (prop_name, prop_value) in properties {
            if let Some(field) = schema.get_field(prop_name) {
                // Type check
                if !field.field_type.matches(prop_value) {
                    return Err(ValidationError::TypeMismatch {
                        field: prop_name.clone(),
                        expected: field.field_type.name(),
                        got: self.value_type_name(prop_value),
                    });
                }

                // NOT NULL check
                if field.has_constraint(&Constraint::NotNull)
                    && matches!(prop_value, PropertyValue::Null)
                {
                    return Err(ValidationError::FieldRequired(prop_name.clone()));
                }

                // CHECK constraint (simplified)
                if field.has_constraint(&Constraint::Check) {
                    if let Some(expr) = &field.check_expression {
                        if !self.evaluate_check(prop_value, expr) {
                            return Err(ValidationError::CheckFailed {
                                field: prop_name.clone(),
                                expression: expr.clone(),
                            });
                        }
                    }
                }
            } else if !schema.allow_extra_properties {
                return Err(ValidationError::UnknownField(prop_name.clone()));
            }
        }

        Ok(())
    }

    /// Get type name from PropertyValue
    fn value_type_name(&self, value: &PropertyValue) -> String {
        match value {
            PropertyValue::Null => "Null".to_string(),
            PropertyValue::Bool(_) => "Boolean".to_string(),
            PropertyValue::Int(_) => "Integer".to_string(),
            PropertyValue::Float(_) => "Float".to_string(),
            PropertyValue::String(_) => "String".to_string(),
            PropertyValue::Bytes(_) => "Bytes".to_string(),
        }
    }

    /// Evaluate CHECK constraint (simplified - no full expression parser yet)
    fn evaluate_check(&self, value: &PropertyValue, expression: &str) -> bool {
        // Simplified evaluation for common cases
        // Full implementation would use expression evaluator from dql_ir

        // Example: "age >= 18 AND age <= 120"
        // For now, just return true (constraint checking will be enhanced)
        // In production, integrate with FilterExpr evaluator

        // Simple numeric range check
        if let PropertyValue::Int(n) = value {
            // Check for patterns like "field >= X AND field <= Y"
            if expression.contains(">=") && expression.contains("<=") {
                // Extract numbers (very simplistic)
                // This is a placeholder - real implementation would parse expression
                return *n >= 0; // Placeholder
            }
        }

        true // Default: pass (until full expression evaluator is integrated)
    }
}

impl Default for SchemaValidator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_schema_creation() {
        let mut schema = Schema::new("Users".to_string());

        let id_field = Field::new("id".to_string(), FieldType::Integer)
            .with_constraint(Constraint::PrimaryKey);

        let name_field = Field::new("name".to_string(), FieldType::String)
            .with_constraint(Constraint::NotNull);

        let age_field = Field::new("age".to_string(), FieldType::Integer)
            .with_check("age >= 18 AND age <= 120".to_string());

        schema.add_field(id_field);
        schema.add_field(name_field);
        schema.add_field(age_field);

        assert_eq!(schema.fields.len(), 3);
        assert!(schema.has_constraint("id", &Constraint::PrimaryKey));
        assert!(schema.has_constraint("name", &Constraint::NotNull));
    }

    #[test]
    fn test_field_type_matching() {
        assert!(FieldType::String.matches(&PropertyValue::String("test".to_string())));
        assert!(FieldType::Integer.matches(&PropertyValue::Int(42)));
        assert!(FieldType::Float.matches(&PropertyValue::Float(3.14)));
        assert!(FieldType::Boolean.matches(&PropertyValue::Bool(true)));

        // Float accepts int (coercion)
        assert!(FieldType::Float.matches(&PropertyValue::Int(42)));

        // Null matches any type (unless NOT NULL)
        assert!(FieldType::String.matches(&PropertyValue::Null));
    }

    #[test]
    fn test_schema_validator() {
        let mut validator = SchemaValidator::new();

        let mut schema = Schema::new("Users".to_string());
        schema.add_field(
            Field::new("name".to_string(), FieldType::String)
                .with_constraint(Constraint::NotNull),
        );
        schema.add_field(Field::new("age".to_string(), FieldType::Integer));

        validator.register_schema(schema);

        // Valid insert
        let mut props = Properties::new();
        props.insert("name".to_string(), PropertyValue::String("Alice".to_string()));
        props.insert("age".to_string(), PropertyValue::Int(28));

        assert!(validator.validate_insert("Users", &props).is_ok());

        // Missing required field
        let mut bad_props = Properties::new();
        bad_props.insert("age".to_string(), PropertyValue::Int(28));

        assert!(validator.validate_insert("Users", &bad_props).is_err());
    }

    #[test]
    fn test_default_values() {
        let mut validator = SchemaValidator::new();

        let mut schema = Schema::new("Products".to_string());
        schema.add_field(
            Field::new("name".to_string(), FieldType::String)
                .with_constraint(Constraint::NotNull),
        );
        schema.add_field(
            Field::new("stock".to_string(), FieldType::Integer)
                .with_constraint(Constraint::Default(PropertyValue::Int(0))),
        );

        validator.register_schema(schema);

        let mut props = Properties::new();
        props.insert("name".to_string(), PropertyValue::String("Laptop".to_string()));

        // Apply defaults
        validator.apply_defaults("Products", &mut props);

        // Stock should now have default value
        assert_eq!(props.get("stock"), Some(&PropertyValue::Int(0)));
    }
}
