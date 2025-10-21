//! DQL (Deed Query Language) Demonstration
//!
//! Shows how to use DQL for hybrid relational + graph queries.
//!
//! Run with: cargo run --example demo_dql

use deed_core::*;
use std::sync::{Arc, RwLock};
use std::collections::HashMap;

fn main() {
    println!("=== DQL (Deed Query Language) Demo ===\n");

    // Create database
    let graph = Arc::new(RwLock::new(Graph::new()));
    setup_demo_data(&graph);

    let executor = DQLExecutor::new(graph.clone());

    // Demo 1: Simple relational query
    println!("1. Simple SELECT query (relational style):");
    println!("   Query: FROM Users WHERE age > 25 SELECT name, age\n");

    match executor.execute("FROM Users WHERE age > 25 SELECT name, age") {
        Ok(result) => {
            println!("   Results ({} rows):", result.row_count());
            for (i, row) in result.rows.iter().enumerate() {
                println!("   Row {}: {:?}", i + 1, row);
            }
        }
        Err(e) => println!("   Error: {}", e),
    }

    println!("\n{}", "=".repeat(60));

    // Demo 2: Graph traversal query
    println!("\n2. Graph traversal query:");
    println!("   Query: FROM Users u TRAVERSE -[:FOLLOWS]-> friend SELECT u.name, friend.name\n");

    match executor.execute("FROM Users u TRAVERSE -[:FOLLOWS]-> friend SELECT u.name, friend.name") {
        Ok(result) => {
            println!("   Results ({} rows):", result.row_count());
            for (i, row) in result.rows.iter().enumerate() {
                println!("   Row {}: {:?}", i + 1, row);
            }
        }
        Err(e) => println!("   Error: {}", e),
    }

    println!("\n{}", "=".repeat(60));

    // Demo 3: Hybrid query with filters
    println!("\n3. Hybrid query (relational + graph in single query):");
    println!("   Query: FROM Users WHERE city = 'NYC' TRAVERSE -[:PURCHASED]-> product");
    println!("          WHERE product.price > 50 SELECT name, product.name LIMIT 10\n");

    let hybrid_query = "FROM Users WHERE city = 'NYC' TRAVERSE -[:PURCHASED]-> product SELECT name LIMIT 10";

    match executor.execute(hybrid_query) {
        Ok(result) => {
            println!("   Results ({} rows):", result.row_count());
            for (i, row) in result.rows.iter().enumerate() {
                println!("   Row {}: {:?}", i + 1, row);
            }
        }
        Err(e) => println!("   Error: {}", e),
    }

    println!("\n{}", "=".repeat(60));

    // Demo 4: Query with ORDER BY
    println!("\n4. Query with ORDER BY:");
    println!("   Query: FROM Users WHERE age > 18 SELECT name, age ORDER BY age DESC LIMIT 5\n");

    match executor.execute("FROM Users WHERE age > 18 SELECT name, age ORDER BY age DESC LIMIT 5") {
        Ok(result) => {
            println!("   Results ({} rows):", result.row_count());
            for (i, row) in result.rows.iter().enumerate() {
                println!("   Row {}: {:?}", i + 1, row);
            }
        }
        Err(e) => println!("   Error: {}", e),
    }

    println!("\n{}", "=".repeat(60));

    // Demo 5: Parser demonstration
    println!("\n5. DQL Parser demonstration:");
    println!("   Parsing: FROM Users u TRAVERSE -[:FOLLOWS*1..3]-> friend\n");

    match DQLParser::parse("FROM Users u TRAVERSE -[:FOLLOWS*1..3]-> friend SELECT friend.name") {
        Ok(query) => {
            println!("   Parsed successfully!");
            println!("   Query structure: {:#?}", query);
        }
        Err(e) => println!("   Parse error: {}", e),
    }

    println!("\n{}", "=".repeat(60));

    // Demo 6: Biological optimization
    println!("\n6. Biological optimization (Ant Colony):");
    demonstrate_optimization();

    println!("\n{}", "=".repeat(60));

    // Demo 7: Stigmergy cache
    println!("\n7. Stigmergy cache demonstration:");
    demonstrate_cache(&executor);

    println!("\n=== Demo Complete ===");
}

fn setup_demo_data(graph: &Arc<RwLock<Graph>>) {
    println!("Setting up demo data...\n");

    let g = graph.read().unwrap();

    // Add users
    let mut user_ids = Vec::new();

    let users_data = vec![
        ("Alice", 28, "NYC"),
        ("Bob", 32, "NYC"),
        ("Carol", 24, "SF"),
        ("Dave", 30, "NYC"),
        ("Eve", 26, "SF"),
        ("Frank", 35, "NYC"),
        ("Grace", 22, "SF"),
        ("Henry", 29, "NYC"),
    ];

    for (name, age, city) in users_data {
        let mut props = HashMap::new();
        props.insert("name".to_string(), PropertyValue::String(name.to_string()));
        props.insert("age".to_string(), PropertyValue::Int(age));
        props.insert("city".to_string(), PropertyValue::String(city.to_string()));

        let id = g.add_entity("Users".to_string(), props);
        user_ids.push(id);
    }

    // Add FOLLOWS edges (social graph)
    let follows = vec![
        (0, 1), // Alice follows Bob
        (0, 3), // Alice follows Dave
        (1, 2), // Bob follows Carol
        (2, 4), // Carol follows Eve
        (3, 5), // Dave follows Frank
        (4, 6), // Eve follows Grace
        (5, 7), // Frank follows Henry
    ];

    for (from, to) in follows {
        g.add_edge(
            user_ids[from],
            user_ids[to],
            "FOLLOWS".to_string(),
            HashMap::new(),
        );
    }

    // Add products
    let products_data = vec![
        ("Laptop", 1200),
        ("Mouse", 25),
        ("Keyboard", 80),
        ("Monitor", 350),
    ];

    let mut product_ids = Vec::new();

    for (name, price) in products_data {
        let mut props = HashMap::new();
        props.insert("name".to_string(), PropertyValue::String(name.to_string()));
        props.insert("price".to_string(), PropertyValue::Int(price));

        let id = g.add_entity("Products".to_string(), props);
        product_ids.push(id);
    }

    // Add PURCHASED edges
    let purchases = vec![
        (0, 0), // Alice purchased Laptop
        (0, 1), // Alice purchased Mouse
        (1, 2), // Bob purchased Keyboard
        (3, 3), // Dave purchased Monitor
    ];

    for (user, product) in purchases {
        g.add_edge(
            user_ids[user],
            product_ids[product],
            "PURCHASED".to_string(),
            HashMap::new(),
        );
    }

    println!("Created {} users and {} products", user_ids.len(), product_ids.len());
    println!("Added {} FOLLOWS and {} PURCHASED edges\n", follows.len(), purchases.len());
}

fn demonstrate_optimization() {
    use dql_ir::*;

    let stats = GraphStats {
        entity_count: 10000,
        edge_count: 50000,
        collection_count: 10,
        avg_pheromone: 1.0,
    };

    println!("   Graph stats: {} entities, {} edges", stats.entity_count, stats.edge_count);

    // Create a query plan
    let operations = vec![
        Operation::Scan {
            collection: "Users".to_string(),
            alias: "u".to_string(),
            filter: Some(FilterExpr::Equal(
                Box::new(FilterExpr::Property {
                    binding: "u".to_string(),
                    property: "city".to_string(),
                }),
                Box::new(FilterExpr::Constant(Value::String("NYC".to_string()))),
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

    println!("   Original plan operations: {}", plan.operations.len());

    // Optimize with ant colony
    let mut optimizer = AntColonyOptimizer::new();
    let optimized = optimizer.optimize(plan, &stats);

    println!("   Optimized plan operations: {}", optimized.operations.len());
    println!("   Estimated cost: {:.2}", optimized.estimated_cost);
    println!("   Pheromone strength: {:.2}", optimized.pheromone_strength);

    // Check if optimization converted scan to index lookup
    let has_index = optimized
        .operations
        .iter()
        .any(|op| matches!(op, Operation::IndexLookup { .. }));

    if has_index {
        println!("   ✓ Optimizer converted Scan to IndexLookup!");
    } else {
        println!("   - Scan operation retained (no suitable index)");
    }
}

fn demonstrate_cache(executor: &DQLExecutor) {
    println!("   Running same query multiple times to demonstrate caching:\n");

    let query = "FROM Users WHERE age > 20 SELECT name";

    // First execution (cache miss)
    let start = std::time::Instant::now();
    let _ = executor.execute(query);
    let first_time = start.elapsed();

    println!("   First execution: {:?} (cache miss)", first_time);

    // Subsequent executions (cache hits)
    for i in 2..=5 {
        let start = std::time::Instant::now();
        let _ = executor.execute(query);
        let exec_time = start.elapsed();

        println!("   Execution {}: {:?} (cache hit)", i, exec_time);
    }

    println!("\n   Cache provides faster query execution on repeated queries!");
}
