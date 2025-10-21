//! Integration tests for DQL (Deed Query Language)

use deed_core::*;
use std::sync::{Arc, RwLock};

#[test]
fn test_dql_simple_select() {
    let graph = setup_test_graph();
    let executor = DQLExecutor::new(graph);

    let result = executor.execute("FROM Users WHERE age > 20 SELECT name, age");

    assert!(result.is_ok(), "Query should execute successfully");
    let res = result.unwrap();
    assert!(res.row_count() > 0, "Should return at least one row");
}

#[test]
fn test_dql_hybrid_query() {
    let graph = setup_test_graph_with_edges();
    let executor = DQLExecutor::new(graph);

    let query = "FROM Users u TRAVERSE -[:FOLLOWS]-> friend SELECT u.name, friend.name";
    let result = executor.execute(query);

    assert!(result.is_ok(), "Hybrid query should execute successfully");
}

#[test]
fn test_dql_parser() {
    let query = "FROM Users WHERE city = 'NYC' SELECT name";
    let result = DQLParser::parse(query);

    assert!(result.is_ok(), "Parser should parse valid query");

    if let Ok(dql_ast::Query::Select(select)) = result {
        assert_eq!(select.from.collection, "Users");
        assert!(select.where_clause.is_some());
    } else {
        panic!("Expected SELECT query");
    }
}

#[test]
fn test_dql_lexer() {
    use dql_lexer::{Lexer, Token};

    let mut lexer = Lexer::new("FROM Users WHERE age = 25");
    let tokens = lexer.tokenize().unwrap();

    assert_eq!(tokens[0], Token::From);
    assert_eq!(tokens[1], Token::Identifier("Users".to_string()));
    assert_eq!(tokens[2], Token::Where);
    assert_eq!(tokens[3], Token::Identifier("age".to_string()));
    assert_eq!(tokens[4], Token::Equal);
    assert_eq!(tokens[5], Token::Integer(25));
}

#[test]
fn test_dql_traverse_pattern() {
    let query = "FROM Users TRAVERSE -[:PURCHASED*1..3]-> Product SELECT Product.name";
    let result = DQLParser::parse(query);

    assert!(result.is_ok(), "Variable-length traverse should parse");

    if let Ok(dql_ast::Query::Select(select)) = result {
        let traverse = select.traverse.unwrap();
        assert_eq!(traverse.patterns[0].min_hops, 1);
        assert_eq!(traverse.patterns[0].max_hops, 3);
    }
}

#[test]
fn test_dql_with_order_and_limit() {
    let graph = setup_test_graph();
    let executor = DQLExecutor::new(graph);

    let query = "FROM Users WHERE age > 18 SELECT name, age ORDER BY age DESC LIMIT 5";
    let result = executor.execute(query);

    assert!(result.is_ok());
    let res = result.unwrap();
    assert!(res.row_count() <= 5, "Should respect LIMIT clause");
}

#[test]
fn test_ant_colony_optimizer() {
    use dql_ir::*;

    let stats = GraphStats {
        entity_count: 10000,
        edge_count: 50000,
        collection_count: 10,
        avg_pheromone: 1.0,
    };

    let operations = vec![
        Operation::Scan {
            collection: "Users".to_string(),
            alias: "u".to_string(),
            filter: Some(FilterExpr::Equal(
                Box::new(FilterExpr::Property {
                    binding: "u".to_string(),
                    property: "city".to_string(),
                }),
                Box::new(FilterExpr::Constant(dql_ir::Value::String("NYC".to_string()))),
            )),
        },
        Operation::Project {
            fields: vec![ProjectField {
                expression: FilterExpr::Property {
                    binding: "u".to_string(),
                    property: "name".to_string(),
                },
                alias: "name".to_string(),
            }],
        },
    ];

    let plan = QueryPlan::new(operations);

    let mut optimizer = AntColonyOptimizer::new();
    let optimized = optimizer.optimize(plan.clone(), &stats);

    assert!(optimized.estimated_cost > 0.0);
    println!("Original operations: {}", plan.operations.len());
    println!("Optimized operations: {}", optimized.operations.len());
    println!("Estimated cost: {}", optimized.estimated_cost);
}

#[test]
fn test_stigmergy_cache() {
    use dql_optimizer::StigmergyCache;
    use dql_ir::QueryPlan;

    let mut cache = StigmergyCache::new(10);

    let plan = QueryPlan::new(vec![]);

    // Cache miss
    assert!(cache.get("query1").is_none());

    // Store plan
    cache.put("query1".to_string(), plan.clone());

    // Cache hit
    assert!(cache.get("query1").is_some());

    let stats = cache.stats();
    assert_eq!(stats.size, 1);
    assert_eq!(stats.total_hits, 1);
}

// Helper functions

fn setup_test_graph() -> Arc<RwLock<Graph>> {
    let graph = Arc::new(RwLock::new(Graph::new()));

    {
        let g = graph.read().unwrap();

        // Add test users
        for i in 1..=10 {
            let mut props = std::collections::HashMap::new();
            props.insert(
                "name".to_string(),
                PropertyValue::String(format!("User{}", i)),
            );
            props.insert("age".to_string(), PropertyValue::Int(20 + i));
            props.insert(
                "city".to_string(),
                PropertyValue::String(if i % 2 == 0 {
                    "NYC".to_string()
                } else {
                    "SF".to_string()
                }),
            );

            g.add_entity("Users".to_string(), props);
        }
    }

    graph
}

fn setup_test_graph_with_edges() -> Arc<RwLock<Graph>> {
    let graph = Arc::new(RwLock::new(Graph::new()));

    {
        let g = graph.read().unwrap();

        // Add users
        let mut user_ids = Vec::new();
        for i in 1..=5 {
            let mut props = std::collections::HashMap::new();
            props.insert(
                "name".to_string(),
                PropertyValue::String(format!("User{}", i)),
            );
            props.insert("age".to_string(), PropertyValue::Int(20 + i));

            let id = g.add_entity("Users".to_string(), props);
            user_ids.push(id);
        }

        // Add FOLLOWS edges
        for i in 0..user_ids.len() - 1 {
            g.add_edge(
                user_ids[i],
                user_ids[i + 1],
                "FOLLOWS".to_string(),
                std::collections::HashMap::new(),
            );
        }
    }

    graph
}
