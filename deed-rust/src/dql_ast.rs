//! DQL Abstract Syntax Tree (AST)
//!
//! Represents the parsed structure of a DQL query before optimization.

use serde::{Deserialize, Serialize};
use crate::transaction::IsolationLevel;

/// Top-level query node
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Query {
    Select(SelectQuery),
    Insert(InsertQuery),
    Update(UpdateQuery),
    Delete(DeleteQuery),
    Create(CreateQuery),
    // Transaction commands
    Begin(BeginQuery),
    Commit,
    Rollback,
    // Index commands
    CreateIndex(CreateIndexQuery),
    DropIndex(DropIndexQuery),
}

/// BEGIN TRANSACTION query
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BeginQuery {
    pub isolation_level: Option<IsolationLevel>,
}

/// CREATE INDEX query
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CreateIndexQuery {
    pub index_name: String,
    pub collection: String,
    pub field: String,
    pub unique: bool,
}

/// DROP INDEX query
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DropIndexQuery {
    pub index_name: String,
}

/// SELECT query with optional TRAVERSE
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SelectQuery {
    pub from: FromClause,
    pub traverse: Option<TraverseClause>,
    pub where_clause: Option<WhereClause>,
    pub select: SelectClause,
    pub group_by: Option<GroupByClause>,
    pub having: Option<HavingClause>,
    pub order_by: Option<OrderByClause>,
    pub limit: Option<usize>,
    pub offset: Option<usize>,
}

/// FROM clause (table/collection scan)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FromClause {
    pub collection: String,
    pub alias: Option<String>,
}

/// TRAVERSE clause (graph navigation)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TraverseClause {
    pub patterns: Vec<TraversePattern>,
}

/// Single traverse pattern: -[:TYPE]-> Node
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TraversePattern {
    pub direction: Direction,
    pub edge_type: Option<String>,
    pub target_alias: Option<String>,
    pub min_hops: usize,
    pub max_hops: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Direction {
    Outgoing,  // ->
    Incoming,  // <-
    Both,      // <->
}

/// WHERE clause (filter condition)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WhereClause {
    pub condition: Expression,
}

/// Boolean expressions for WHERE
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Expression {
    // Binary operations
    And(Box<Expression>, Box<Expression>),
    Or(Box<Expression>, Box<Expression>),
    Not(Box<Expression>),

    // Comparisons
    Equal(Box<Expression>, Box<Expression>),
    NotEqual(Box<Expression>, Box<Expression>),
    LessThan(Box<Expression>, Box<Expression>),
    LessThanEq(Box<Expression>, Box<Expression>),
    GreaterThan(Box<Expression>, Box<Expression>),
    GreaterThanEq(Box<Expression>, Box<Expression>),

    // Arithmetic
    Add(Box<Expression>, Box<Expression>),
    Subtract(Box<Expression>, Box<Expression>),
    Multiply(Box<Expression>, Box<Expression>),
    Divide(Box<Expression>, Box<Expression>),

    // Aggregations
    Aggregate(AggregateFunction, Box<Expression>),

    // Values
    Property(PropertyRef),
    Literal(Literal),
}

/// Aggregate functions
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum AggregateFunction {
    Count,      // COUNT(*) or COUNT(field)
    Sum,        // SUM(field)
    Avg,        // AVG(field)
    Min,        // MIN(field)
    Max,        // MAX(field)
}

/// Property reference: Table.column or alias.property
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PropertyRef {
    pub entity: Option<String>, // Optional table/alias
    pub property: String,
}

/// Literal values
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Literal {
    Null,
    Bool(bool),
    Integer(i64),
    Float(f64),
    String(String),
}

/// SELECT clause (projection)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SelectClause {
    pub fields: Vec<SelectField>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SelectField {
    pub expression: Expression,
    pub alias: Option<String>,
}

/// GROUP BY clause
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GroupByClause {
    pub fields: Vec<Expression>,
}

/// HAVING clause (filter after GROUP BY)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HavingClause {
    pub condition: Expression,
}

/// ORDER BY clause
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OrderByClause {
    pub fields: Vec<OrderByField>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OrderByField {
    pub expression: Expression,
    pub ascending: bool,
}

/// INSERT query
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InsertQuery {
    pub collection: String,
    pub properties: Vec<(String, Literal)>,
}

/// UPDATE query
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UpdateQuery {
    pub collection: String,
    pub set: Vec<(String, Expression)>,
    pub where_clause: Option<WhereClause>,
}

/// DELETE query
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DeleteQuery {
    pub collection: String,
    pub where_clause: Option<WhereClause>,
}

/// CREATE query (for edges/relationships)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CreateQuery {
    pub edge_type: String,
    pub source: Expression,
    pub target: Expression,
    pub properties: Vec<(String, Literal)>,
}

impl Expression {
    /// Helper to create property reference
    pub fn property(entity: Option<&str>, property: &str) -> Self {
        Expression::Property(PropertyRef {
            entity: entity.map(|s| s.to_string()),
            property: property.to_string(),
        })
    }

    /// Helper to create literal
    pub fn literal(lit: Literal) -> Self {
        Expression::Literal(lit)
    }

    /// Helper to create string literal
    pub fn string(s: &str) -> Self {
        Expression::Literal(Literal::String(s.to_string()))
    }

    /// Helper to create integer literal
    pub fn integer(n: i64) -> Self {
        Expression::Literal(Literal::Integer(n))
    }

    /// Helper to create boolean literal
    pub fn bool(b: bool) -> Self {
        Expression::Literal(Literal::Bool(b))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_select() {
        let query = SelectQuery {
            from: FromClause {
                collection: "Users".to_string(),
                alias: None,
            },
            traverse: None,
            where_clause: Some(WhereClause {
                condition: Expression::Equal(
                    Box::new(Expression::property(None, "age")),
                    Box::new(Expression::integer(25)),
                ),
            }),
            select: SelectClause {
                fields: vec![SelectField {
                    expression: Expression::property(None, "name"),
                    alias: None,
                }],
            },
            order_by: None,
            limit: None,
            offset: None,
        };

        // Should represent: FROM Users WHERE age = 25 SELECT name
        assert_eq!(query.from.collection, "Users");
    }

    #[test]
    fn test_hybrid_query() {
        let query = SelectQuery {
            from: FromClause {
                collection: "Users".to_string(),
                alias: Some("u".to_string()),
            },
            traverse: Some(TraverseClause {
                patterns: vec![TraversePattern {
                    direction: Direction::Outgoing,
                    edge_type: Some("PURCHASED".to_string()),
                    target_alias: Some("p".to_string()),
                    min_hops: 1,
                    max_hops: 1,
                }],
            }),
            where_clause: Some(WhereClause {
                condition: Expression::GreaterThan(
                    Box::new(Expression::property(Some("p"), "price")),
                    Box::new(Expression::integer(100)),
                ),
            }),
            select: SelectClause {
                fields: vec![
                    SelectField {
                        expression: Expression::property(Some("u"), "name"),
                        alias: None,
                    },
                    SelectField {
                        expression: Expression::property(Some("p"), "name"),
                        alias: Some("product_name".to_string()),
                    },
                ],
            },
            order_by: None,
            limit: Some(10),
            offset: None,
        };

        // Represents: FROM Users u TRAVERSE -[:PURCHASED]-> p WHERE p.price > 100 SELECT u.name, p.name AS product_name LIMIT 10
        assert_eq!(query.from.collection, "Users");
        assert_eq!(query.traverse.as_ref().unwrap().patterns.len(), 1);
        assert_eq!(query.limit, Some(10));
    }
}
