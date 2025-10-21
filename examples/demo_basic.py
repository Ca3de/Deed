"""
Basic Deed Database Demo

Demonstrates the hybrid relational/graph capabilities of Deed database,
showing how the same data structure supports both table-like operations
and graph traversals seamlessly.
"""

import sys
sys.path.insert(0, '/home/user/Deed')

from deed import DeedGraph, Entity
from datetime import datetime


def demo_hybrid_database():
    """
    Demonstrate Deed's ability to handle both relational and graph operations
    on the same data structure.
    """
    print("=" * 70)
    print("DEED DATABASE - HYBRID RELATIONAL & GRAPH DEMO")
    print("=" * 70)
    print()

    # Create a Deed database instance
    db = DeedGraph(graph_id="social_commerce_db")

    print("1. RELATIONAL OPERATIONS (Table-like)")
    print("-" * 70)

    # Create collections (like tables in RDBMS)
    users = db.create_collection("Users", schema={
        'name': str,
        'email': str,
        'age': int,
        'city': str
    })

    products = db.create_collection("Products", schema={
        'name': str,
        'price': float,
        'category': str,
        'rating': float
    })

    # Add entities (like inserting rows)
    alice = db.add_entity(
        collection_name='Users',
        properties={'name': 'Alice', 'email': 'alice@example.com', 'age': 28, 'city': 'NYC'}
    )

    bob = db.add_entity(
        collection_name='Users',
        properties={'name': 'Bob', 'email': 'bob@example.com', 'age': 35, 'city': 'NYC'}
    )

    carol = db.add_entity(
        collection_name='Users',
        properties={'name': 'Carol', 'email': 'carol@example.com', 'age': 42, 'city': 'SF'}
    )

    david = db.add_entity(
        collection_name='Users',
        properties={'name': 'David', 'email': 'david@example.com', 'age': 22, 'city': 'NYC'}
    )

    print(f"Created {users.count()} users")
    print()

    # Add products
    laptop = db.add_entity(
        collection_name='Products',
        properties={'name': 'Laptop Pro', 'price': 1299.99, 'category': 'Electronics', 'rating': 4.5}
    )

    book = db.add_entity(
        collection_name='Products',
        properties={'name': 'Database Design Book', 'price': 49.99, 'category': 'Books', 'rating': 4.8}
    )

    headphones = db.add_entity(
        collection_name='Products',
        properties={'name': 'Wireless Headphones', 'price': 199.99, 'category': 'Electronics', 'rating': 4.3}
    )

    print(f"Created {products.count()} products")
    print()

    # Create index for fast lookups (like SQL indexes)
    users.create_index('city')
    users.create_index('age')
    products.create_index('category')

    print("Created indexes on: city, age, category")
    print()

    # SQL-style queries
    print("Query: SELECT * FROM Users WHERE city = 'NYC'")
    nyc_users = users.lookup('city', value='NYC')
    for user in nyc_users:
        print(f"  - {user.get_property('name')} (age {user.get_property('age')})")
    print()

    print("Query: SELECT * FROM Users WHERE age >= 30 AND age <= 40")
    mature_users = users.lookup('age', min_value=30, max_value=40)
    for user in mature_users:
        print(f"  - {user.get_property('name')} (age {user.get_property('age')})")
    print()

    print("Query: SELECT * FROM Products WHERE category = 'Electronics'")
    electronics = products.lookup('category', value='Electronics')
    for product in electronics:
        print(f"  - {product.get_property('name')} (${product.get_property('price')})")
    print()

    print()
    print("2. GRAPH OPERATIONS (Relationship traversals)")
    print("-" * 70)

    # Add graph relationships (edges)
    # Social network
    db.add_edge(alice.id, bob.id, "FOLLOWS")
    db.add_edge(alice.id, carol.id, "FOLLOWS")
    db.add_edge(bob.id, carol.id, "FOLLOWS")
    db.add_edge(david.id, alice.id, "FOLLOWS")

    # Friendships (bidirectional)
    db.add_edge(alice.id, bob.id, "FRIENDS")
    db.add_edge(bob.id, alice.id, "FRIENDS")

    # Purchase relationships
    db.add_edge(alice.id, laptop.id, "PURCHASED", properties={'date': '2025-10-15', 'quantity': 1})
    db.add_edge(alice.id, book.id, "PURCHASED", properties={'date': '2025-10-18', 'quantity': 2})
    db.add_edge(bob.id, laptop.id, "PURCHASED", properties={'date': '2025-10-16', 'quantity': 1})
    db.add_edge(bob.id, headphones.id, "PURCHASED", properties={'date': '2025-10-17', 'quantity': 1})
    db.add_edge(carol.id, book.id, "PURCHASED", properties={'date': '2025-10-19', 'quantity': 1})

    print("Created relationships:")
    print("  - Social: FOLLOWS, FRIENDS")
    print("  - Commerce: PURCHASED")
    print()

    # Graph traversal queries
    print("Graph Query: Who does Alice follow?")
    alice_follows = db.traverse(alice.id, edge_type="FOLLOWS", direction="out", max_depth=1)
    for person in alice_follows:
        print(f"  - {person.get_property('name')}")
    print()

    print("Graph Query: Who follows Alice?")
    alice_followers = db.traverse(alice.id, edge_type="FOLLOWS", direction="in", max_depth=1)
    for person in alice_followers:
        print(f"  - {person.get_property('name')}")
    print()

    print("Graph Query: What did Alice purchase?")
    alice_purchases = db.traverse(alice.id, edge_type="PURCHASED", direction="out", max_depth=1)
    for product in alice_purchases:
        print(f"  - {product.get_property('name')} (${product.get_property('price')})")
    print()

    print()
    print("3. HYBRID QUERIES (Combining relational + graph)")
    print("-" * 70)

    # Complex hybrid query: Find products purchased by users in NYC
    print("Hybrid Query: Products purchased by NYC users")
    nyc_users = users.lookup('city', value='NYC')

    purchased_products = set()
    for user in nyc_users:
        purchases = db.traverse(user.id, edge_type="PURCHASED", direction="out", max_depth=1)
        purchased_products.update(purchases)

    for product in purchased_products:
        print(f"  - {product.get_property('name')} (${product.get_property('price')})")
    print()

    # Another hybrid query: Friends of friends recommendation
    print("Hybrid Query: Alice's friends-of-friends (potential new friends)")
    alice_friends = db.traverse(alice.id, edge_type="FRIENDS", direction="both", max_depth=1)
    alice_friend_ids = {f.id for f in alice_friends}

    # Get friends of friends
    potential_friends = set()
    for friend in alice_friends:
        fof = db.traverse(friend.id, edge_type="FRIENDS", direction="both", max_depth=1)
        potential_friends.update(fof)

    # Filter out Alice and her existing friends
    potential_friends = [
        p for p in potential_friends
        if p.id != alice.id and p.id not in alice_friend_ids
    ]

    for person in potential_friends:
        print(f"  - {person.get_property('name')}")
    print()

    print()
    print("4. BIOLOGICAL FEATURES (Pheromone tracking)")
    print("-" * 70)

    # Demonstrate pheromone tracking
    print("Accessing entities multiple times builds pheromone strength...")

    # Simulate popular products being accessed frequently
    for _ in range(10):
        laptop_entity = db.get_entity(laptop.id)  # Each access increases pheromone

    for _ in range(5):
        book_entity = db.get_entity(book.id)

    print(f"Laptop access count: {db.get_entity(laptop.id).access_count}")
    print(f"Book access count: {db.get_entity(book.id).access_count}")
    print(f"Headphones access count: {db.get_entity(headphones.id).access_count}")
    print()

    print("Pheromone tracking helps optimize future queries by caching")
    print("frequently-accessed data and routing to hot replicas.")
    print()

    # Check edge pheromones
    alice_to_bob_edges = db.get_edges_between(alice.id, bob.id, edge_type="FOLLOWS")
    if alice_to_bob_edges:
        edge = alice_to_bob_edges[0]
        print(f"Edge pheromone (Alice->Bob FOLLOWS): {edge.pheromone:.2f}")

        # Mark it as traversed multiple times
        for _ in range(5):
            edge.mark_traversed(cost_ms=10.0)

        print(f"After 5 traversals: {edge.pheromone:.2f}")
        print("Stronger pheromone = preferred routing path in future queries")
    print()

    print()
    print("5. DATABASE STATISTICS")
    print("-" * 70)

    stats = db.get_stats()
    print(f"Total entities: {stats['total_entities']}")
    print(f"Total edges: {stats['total_edges']}")
    print(f"Total collections: {stats['total_collections']}")
    print(f"Average entity degree: {stats['avg_entity_degree']:.2f}")
    print(f"Average edge pheromone: {stats['avg_pheromone']:.2f}")
    print()

    print("=" * 70)
    print("DEMO COMPLETE")
    print("=" * 70)
    print()
    print("Key Takeaways:")
    print("- Same data structure supports SQL-like AND graph queries")
    print("- No separate databases needed for relational vs graph workloads")
    print("- Pheromone tracking guides optimization automatically")
    print("- Biological principles make the database adaptive and self-improving")
    print()


if __name__ == "__main__":
    demo_hybrid_database()
