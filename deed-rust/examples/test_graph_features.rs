use deed_rust::*;
use std::sync::{Arc, RwLock};

fn main() {
    println!("🌐 Deed Graph Features Test\n");

    let graph = Arc::new(RwLock::new(Graph::new()));
    let executor = DQLExecutor::new(graph.clone());

    // Test 1: Create a Social Network with Edges
    println!("👥 Test 1: Creating social network...");

    // Create users
    executor.execute(r#"INSERT INTO Users VALUES ({name: "Alice", age: 28, city: "NYC"})"#).ok();
    executor.execute(r#"INSERT INTO Users VALUES ({name: "Bob", age: 32, city: "NYC"})"#).ok();
    executor.execute(r#"INSERT INTO Users VALUES ({name: "Carol", age: 25, city: "SF"})"#).ok();
    executor.execute(r#"INSERT INTO Users VALUES ({name: "Dave", age: 30, city: "NYC"})"#).ok();

    println!("   ✓ Created 4 users\n");

    // Test 2: Create Relationships (Edges)
    println!("🔗 Test 2: Creating relationships...");

    // Get entity IDs to create edges
    let graph_lock = graph.read().unwrap();
    let users: Vec<Entity> = graph_lock.scan_collection("Users");

    let alice_id = users.iter().find(|u| {
        u.get_property("name").and_then(|p| p.as_str()) == Some("Alice")
    }).unwrap().id;

    let bob_id = users.iter().find(|u| {
        u.get_property("name").and_then(|p| p.as_str()) == Some("Bob")
    }).unwrap().id;

    let carol_id = users.iter().find(|u| {
        u.get_property("name").and_then(|p| p.as_str()) == Some("Carol")
    }).unwrap().id;

    let dave_id = users.iter().find(|u| {
        u.get_property("name").and_then(|p| p.as_str()) == Some("Dave")
    }).unwrap().id;

    // Create FOLLOWS edges
    use std::collections::HashMap;
    graph_lock.add_edge(alice_id, bob_id, "FOLLOWS".to_string(), HashMap::new());
    graph_lock.add_edge(alice_id, carol_id, "FOLLOWS".to_string(), HashMap::new());
    graph_lock.add_edge(bob_id, dave_id, "FOLLOWS".to_string(), HashMap::new());
    graph_lock.add_edge(carol_id, alice_id, "FOLLOWS".to_string(), HashMap::new());

    println!("   ✓ Created 4 FOLLOWS relationships:");
    println!("     - Alice → Bob");
    println!("     - Alice → Carol");
    println!("     - Bob → Dave");
    println!("     - Carol → Alice\n");

    drop(graph_lock);

    // Test 3: Graph Traversal
    println!("🚶 Test 3: Testing graph traversal...");

    // Get Alice's followers (who Alice follows)
    let graph_lock = graph.read().unwrap();
    let alice_follows = graph_lock.get_outgoing_neighbors(alice_id, Some("FOLLOWS"));

    println!("   Alice follows {} people:", alice_follows.len());
    for (target_id, edge_id) in &alice_follows {
        if let Some(user) = graph_lock.get_entity(*target_id) {
            let name = user.get_property("name").and_then(|p| p.as_str()).unwrap_or("Unknown");
            println!("     - {} (edge: {:?})", name, edge_id);
        }
    }

    // Get Alice's followers (who follows Alice)
    let alice_followers = graph_lock.get_incoming_neighbors(alice_id, Some("FOLLOWS"));
    println!("\n   Alice has {} followers:", alice_followers.len());
    for (source_id, edge_id) in &alice_followers {
        if let Some(user) = graph_lock.get_entity(*source_id) {
            let name = user.get_property("name").and_then(|p| p.as_str()).unwrap_or("Unknown");
            println!("     - {} (edge: {:?})", name, edge_id);
        }
    }

    drop(graph_lock);

    // Test 4: Pheromone Testing
    println!("\n🐜 Test 4: Testing pheromone reinforcement...");

    let graph_lock = graph.read().unwrap();

    // Get an edge and check its pheromone
    if let Some((_, edge_id)) = alice_follows.first() {
        if let Some(mut edge) = graph_lock.get_edge(*edge_id) {
            let initial_pheromone = edge.pheromone.strength();
            println!("   Initial pheromone strength: {:.2}", initial_pheromone);

            // Simulate traversals with different latencies
            edge.mark_traversed(1_000_000);  // 1ms - fast
            println!("   After fast traversal (1ms): {:.2}", edge.pheromone.strength());

            edge.mark_traversed(100_000);    // 0.1ms - very fast
            println!("   After very fast traversal (0.1ms): {:.2}", edge.pheromone.strength());

            edge.mark_traversed(10_000_000); // 10ms - slow
            println!("   After slow traversal (10ms): {:.2}", edge.pheromone.strength());

            println!("   Traversal count: {}", edge.traversal_count);
            println!("   Average latency: {:.2}ms", edge.avg_latency_ns as f64 / 1_000_000.0);
        }
    }

    drop(graph_lock);

    // Test 5: Statistics
    println!("\n📊 Test 5: Graph statistics");
    let graph_lock = graph.read().unwrap();
    let stats = graph_lock.stats();

    println!("   Total entities: {}", stats.entity_count);
    println!("   Total edges: {}", stats.edge_count);
    println!("   Collections: {}", stats.collection_count);
    println!("   Average pheromone: {:.2}", stats.avg_pheromone);

    drop(graph_lock);

    println!("\n✨ Graph Features Test Complete!");
}
