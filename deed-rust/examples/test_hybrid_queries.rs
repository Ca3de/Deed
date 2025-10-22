use deed_rust::*;
use std::sync::{Arc, RwLock};
use std::collections::HashMap;

fn main() {
    println!("🔀 Deed Hybrid Query Test (Relational + Graph)\n");

    let graph = Arc::new(RwLock::new(Graph::new()));
    let executor = DQLExecutor::new(graph.clone());

    // Setup: Create an E-commerce scenario
    println!("🏪 Setup: Creating e-commerce data...");

    // Create customers
    executor.execute(r#"INSERT INTO Customers VALUES ({name: "Alice", city: "NYC", membership: "Gold"})"#).ok();
    executor.execute(r#"INSERT INTO Customers VALUES ({name: "Bob", city: "NYC", membership: "Silver"})"#).ok();
    executor.execute(r#"INSERT INTO Customers VALUES ({name: "Carol", city: "SF", membership: "Gold"})"#).ok();
    executor.execute(r#"INSERT INTO Customers VALUES ({name: "Dave", city: "LA", membership: "Bronze"})"#).ok();

    // Create products
    executor.execute(r#"INSERT INTO Products VALUES ({name: "Laptop", price: 999.99, category: "Electronics"})"#).ok();
    executor.execute(r#"INSERT INTO Products VALUES ({name: "Mouse", price: 29.99, category: "Electronics"})"#).ok();
    executor.execute(r#"INSERT INTO Products VALUES ({name: "Desk", price: 299.99, category: "Furniture"})"#).ok();
    executor.execute(r#"INSERT INTO Products VALUES ({name: "Chair", price: 199.99, category: "Furniture"})"#).ok();

    println!("   ✓ Created 4 customers");
    println!("   ✓ Created 4 products\n");

    // Create purchase relationships
    println!("🛒 Creating purchase relationships...");

    let graph_lock = graph.read().unwrap();

    // Get all entities
    let customers: Vec<Entity> = graph_lock.scan_collection("Customers");
    let products: Vec<Entity> = graph_lock.scan_collection("Products");

    // Helper to find entity by name
    let find_customer = |name: &str| -> EntityId {
        customers.iter()
            .find(|c| c.get_property("name").and_then(|p| p.as_str()) == Some(name))
            .unwrap().id
    };

    let find_product = |name: &str| -> EntityId {
        products.iter()
            .find(|p| p.get_property("name").and_then(|p| p.as_str()) == Some(name))
            .unwrap().id
    };

    let alice_id = find_customer("Alice");
    let bob_id = find_customer("Bob");
    let carol_id = find_customer("Carol");

    let laptop_id = find_product("Laptop");
    let mouse_id = find_product("Mouse");
    let desk_id = find_product("Desk");
    let chair_id = find_product("Chair");

    // Create PURCHASED edges with metadata
    let mut purchase1 = HashMap::new();
    purchase1.insert("quantity".to_string(), PropertyValue::Int(1));
    purchase1.insert("date".to_string(), PropertyValue::String("2024-01-15".to_string()));
    graph_lock.add_edge(alice_id, laptop_id, "PURCHASED".to_string(), purchase1);

    let mut purchase2 = HashMap::new();
    purchase2.insert("quantity".to_string(), PropertyValue::Int(2));
    purchase2.insert("date".to_string(), PropertyValue::String("2024-01-16".to_string()));
    graph_lock.add_edge(alice_id, mouse_id, "PURCHASED".to_string(), purchase2);

    let mut purchase3 = HashMap::new();
    purchase3.insert("quantity".to_string(), PropertyValue::Int(1));
    purchase3.insert("date".to_string(), PropertyValue::String("2024-01-20".to_string()));
    graph_lock.add_edge(bob_id, desk_id, "PURCHASED".to_string(), purchase3);

    let mut purchase4 = HashMap::new();
    purchase4.insert("quantity".to_string(), PropertyValue::Int(1));
    purchase4.insert("date".to_string(), PropertyValue::String("2024-01-22".to_string()));
    graph_lock.add_edge(carol_id, laptop_id, "PURCHASED".to_string(), purchase4);

    println!("   ✓ Alice purchased: Laptop, Mouse (2)");
    println!("   ✓ Bob purchased: Desk");
    println!("   ✓ Carol purchased: Laptop\n");

    drop(graph_lock);

    // Test 1: Pure Relational Query
    println!("📊 Test 1: Pure Relational Query");
    println!("   Query: SELECT all Gold members");

    match executor.execute(r#"FROM Customers WHERE membership = "Gold" SELECT name, city, membership"#) {
        Ok(result) => {
            println!("   ✓ Found {} Gold members:", result.rows.len());
            for row in &result.rows {
                println!("     {:?}", row);
            }
        }
        Err(e) => println!("   ✗ Error: {}", e),
    }

    // Test 2: Graph Traversal (Manual)
    println!("\n🌐 Test 2: Graph Traversal");
    println!("   Query: What did Alice purchase?");

    let graph_lock = graph.read().unwrap();
    let purchases = graph_lock.get_outgoing_neighbors(alice_id, Some("PURCHASED"));

    println!("   ✓ Alice purchased {} items:", purchases.len());
    for (product_id, edge_id) in &purchases {
        if let Some(product) = graph_lock.get_entity(*product_id) {
            let name = product.get_property("name").and_then(|p| p.as_str()).unwrap_or("Unknown");
            let price = product.get_property("price");

            if let Some(edge) = graph_lock.get_edge(*edge_id) {
                let quantity = edge.properties.get("quantity");
                println!("     - {} (price: {:?}, quantity: {:?})", name, price, quantity);
            }
        }
    }

    drop(graph_lock);

    // Test 3: Hybrid Query Concept (Manual Implementation)
    println!("\n🔀 Test 3: Hybrid Query (Relational + Graph)");
    println!("   Query: Gold members from NYC and their electronics purchases");

    let graph_lock = graph.read().unwrap();

    // Step 1: Relational filter - Gold members from NYC
    let nyc_gold_members: Vec<Entity> = graph_lock.scan_collection("Customers")
        .into_iter()
        .filter(|c| {
            c.get_property("city").and_then(|p| p.as_str()) == Some("NYC") &&
            c.get_property("membership").and_then(|p| p.as_str()) == Some("Gold")
        })
        .collect();

    println!("   Step 1: Found {} Gold members in NYC", nyc_gold_members.len());

    // Step 2: Graph traversal - Get their purchases
    for customer in &nyc_gold_members {
        let name = customer.get_property("name").and_then(|p| p.as_str()).unwrap_or("Unknown");
        println!("\n   {} purchased:", name);

        let purchases = graph_lock.get_outgoing_neighbors(customer.id, Some("PURCHASED"));

        for (product_id, edge_id) in &purchases {
            if let Some(product) = graph_lock.get_entity(*product_id) {
                // Step 3: Another relational filter - Only electronics
                if product.get_property("category").and_then(|p| p.as_str()) == Some("Electronics") {
                    let product_name = product.get_property("name").and_then(|p| p.as_str()).unwrap_or("Unknown");
                    let price = product.get_property("price");

                    if let Some(edge) = graph_lock.get_edge(*edge_id) {
                        let quantity = edge.properties.get("quantity");
                        println!("     - {} (price: {:?}, qty: {:?})", product_name, price, quantity);
                    }
                }
            }
        }
    }

    drop(graph_lock);

    // Test 4: Aggregation on Graph Data
    println!("\n💰 Test 4: Aggregation - Total spending by customer");

    let graph_lock = graph.read().unwrap();
    let all_customers = graph_lock.scan_collection("Customers");

    for customer in &all_customers {
        let name = customer.get_property("name").and_then(|p| p.as_str()).unwrap_or("Unknown");
        let purchases = graph_lock.get_outgoing_neighbors(customer.id, Some("PURCHASED"));

        let mut total_spending = 0.0;
        for (product_id, edge_id) in &purchases {
            if let Some(product) = graph_lock.get_entity(*product_id) {
                if let Some(PropertyValue::Float(price)) = product.get_property("price") {
                    if let Some(edge) = graph_lock.get_edge(*edge_id) {
                        let quantity = edge.properties.get("quantity")
                            .and_then(|q| if let PropertyValue::Int(q) = q { Some(*q) } else { None })
                            .unwrap_or(1);
                        total_spending += price * quantity as f64;
                    }
                }
            }
        }

        println!("   {} spent: ${:.2}", name, total_spending);
    }

    drop(graph_lock);

    // Test 5: Product Recommendations (Graph Pattern)
    println!("\n🎯 Test 5: Product Recommendations");
    println!("   Pattern: Products purchased by customers who bought Laptop");

    let graph_lock = graph.read().unwrap();

    // Find who bought Laptop
    let laptop_buyers = graph_lock.get_incoming_neighbors(laptop_id, Some("PURCHASED"));
    println!("   {} customers bought Laptop", laptop_buyers.len());

    let mut recommended_products: HashMap<String, usize> = HashMap::new();

    // Find what else they bought
    for (buyer_id, _) in &laptop_buyers {
        let their_purchases = graph_lock.get_outgoing_neighbors(*buyer_id, Some("PURCHASED"));

        for (product_id, _) in &their_purchases {
            if *product_id != laptop_id {  // Exclude Laptop itself
                if let Some(product) = graph_lock.get_entity(*product_id) {
                    let name = product.get_property("name")
                        .and_then(|p| p.as_str())
                        .unwrap_or("Unknown")
                        .to_string();

                    *recommended_products.entry(name).or_insert(0) += 1;
                }
            }
        }
    }

    println!("\n   Recommended products (bought by Laptop buyers):");
    for (product, count) in &recommended_products {
        println!("     - {} ({} purchases)", product, count);
    }

    drop(graph_lock);

    println!("\n✨ Hybrid Query Test Complete!");
}
