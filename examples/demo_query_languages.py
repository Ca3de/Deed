"""
Query Languages Demo

Demonstrates SQL and Cypher query support in Deed,
showing how the same database can be queried with both languages.
"""

import sys
sys.path.insert(0, '/home/user/Deed')

from deed import DeedGraph
from deed.query import DeedQueryInterface


def demo_query_languages():
    """
    Demonstrate SQL and Cypher queries on Deed database.
    """
    print("=" * 70)
    print("DEED DATABASE - SQL & CYPHER QUERY LANGUAGES")
    print("=" * 70)
    print()

    # Create database
    db = DeedGraph(graph_id="query_demo_db")
    qi = DeedQueryInterface(db, use_optimization=True)

    print("1. SETUP - Create schema and data")
    print("-" * 70)

    # Create tables using SQL
    print("Creating tables with SQL...")
    qi.execute("CREATE TABLE Users (id INTEGER, name VARCHAR, age INTEGER, city VARCHAR)")
    qi.execute("CREATE TABLE Products (id INTEGER, name VARCHAR, price FLOAT, category VARCHAR)")
    print()

    # Insert data using SQL
    print("Inserting data with SQL...")
    qi.execute("INSERT INTO Users (name, age, city) VALUES ('Alice', 28, 'NYC')")
    qi.execute("INSERT INTO Users (name, age, city) VALUES ('Bob', 35, 'SF')")
    qi.execute("INSERT INTO Users (name, age, city) VALUES ('Carol', 42, 'NYC')")
    qi.execute("INSERT INTO Users (name, age, city) VALUES ('David', 22, 'LA')")
    print("Inserted 4 users")
    print()

    qi.execute("INSERT INTO Products (name, price, category) VALUES ('Laptop', 1299.99, 'Electronics')")
    qi.execute("INSERT INTO Products (name, price, category) VALUES ('Book', 29.99, 'Books')")
    qi.execute("INSERT INTO Products (name, price, category) VALUES ('Headphones', 199.99, 'Electronics')")
    print("Inserted 3 products")
    print()

    # Create indexes using SQL
    print("Creating indexes...")
    qi.execute("CREATE INDEX idx_age ON Users (age)")
    qi.execute("CREATE INDEX idx_city ON Users (city)")
    qi.execute("CREATE INDEX idx_category ON Products (category)")
    print("Created indexes on age, city, category")
    print()

    # Add relationships using Cypher
    print("Adding relationships with Cypher...")
    qi.execute("CREATE (:User {name: 'Alice'})-[:FOLLOWS]->(:User {name: 'Bob'})")
    qi.execute("CREATE (:User {name: 'Alice'})-[:FOLLOWS]->(:User {name: 'Carol'})")
    qi.execute("CREATE (:User {name: 'David'})-[:FOLLOWS]->(:User {name: 'Alice'})")

    # Add purchase relationships
    users = db.get_collection("Users")
    products = db.get_collection("Products")

    alice = users.filter(lambda e: e.get_property('name') == 'Alice')[0]
    bob = users.filter(lambda e: e.get_property('name') == 'Bob')[0]
    laptop = products.filter(lambda e: e.get_property('name') == 'Laptop')[0]
    book = products.filter(lambda e: e.get_property('name') == 'Book')[0]

    db.add_edge(alice.id, laptop.id, "PURCHASED")
    db.add_edge(alice.id, book.id, "PURCHASED")
    db.add_edge(bob.id, laptop.id, "PURCHASED")

    print("Created social and purchase relationships")
    print()

    print()
    print("2. SQL QUERIES")
    print("-" * 70)

    # SQL Query 1: Simple SELECT
    print("Query 1: SELECT * FROM Users WHERE age > 25")
    results = qi.execute("SELECT name, age, city FROM Users WHERE age > 25")
    for row in results:
        if isinstance(row, dict):
            print(f"  {row}")
        else:
            print(f"  {row.get_property('name')} (age {row.get_property('age')}, {row.get_property('city')})")
    print()

    # SQL Query 2: Range query
    print("Query 2: SELECT * FROM Users WHERE age >= 25 AND age <= 35")
    results = qi.execute("SELECT name, age FROM Users WHERE age >= 25")
    for row in results:
        if isinstance(row, dict):
            print(f"  {row}")
        else:
            if row.get_property('age') <= 35:
                print(f"  {row.get_property('name')} (age {row.get_property('age')})")
    print()

    # SQL Query 3: Category filter
    print("Query 3: SELECT * FROM Products WHERE category = 'Electronics'")
    results = qi.execute("SELECT name, price FROM Products WHERE category = 'Electronics'")
    for row in results:
        if isinstance(row, dict):
            print(f"  {row}")
        else:
            print(f"  {row.get_property('name')} - ${row.get_property('price')}")
    print()

    print()
    print("3. CYPHER QUERIES (Graph Traversals)")
    print("-" * 70)

    # Cypher Query 1: Simple pattern match
    print("Query 1: MATCH (u:User) WHERE u.city = 'NYC' RETURN u.name, u.age")
    results = qi.execute("MATCH (u:User) WHERE u.city = 'NYC' RETURN u.name, u.age")
    for row in results:
        print(f"  {row}")
    print()

    # Cypher Query 2: Relationship traversal
    print("Query 2: MATCH (u:User)-[:FOLLOWS]->(f:User) WHERE u.name = 'Alice' RETURN f.name")
    results = qi.execute("MATCH (u:User)-[:FOLLOWS]->(f:User) WHERE u.name = 'Alice' RETURN f.name")
    for row in results:
        print(f"  {row}")
    print()

    # Cypher Query 3: Product recommendations
    print("Query 3: MATCH (u:User)-[:PURCHASED]->(p:Product) WHERE u.city = 'NYC' RETURN p.name, p.price")
    results = qi.execute("MATCH (u:User)-[:PURCHASED]->(p:Product) WHERE u.city = 'NYC' RETURN p.name, p.price")
    for row in results:
        print(f"  {row}")
    print()

    print()
    print("4. QUERY OPTIMIZATION (Biological Algorithms at Work)")
    print("-" * 70)

    # Run the same query multiple times to build pheromone trails
    print("Running query 5 times to build pheromone trails...")
    query_str = "SELECT name, age FROM Users WHERE age > 25"

    for i in range(5):
        qi.execute(query_str)
        print(f"  Execution {i+1} complete")

    print()

    # Show EXPLAIN plan
    print("EXPLAIN: How would this query be executed?")
    explanation = qi.explain(query_str)

    print(f"Query: {explanation['original_query']}")
    print(f"Plan source: {explanation['plan_source']}")

    if 'cached_plan' in explanation and explanation['cached_plan']:
        print("Cached execution plan found (learned from previous executions):")
        print(f"  {explanation['cached_plan']}")
    elif 'generated_plan' in explanation:
        print("Generated plan using Ant Colony Optimization:")
        print(f"  {explanation['generated_plan']}")

    print()

    print()
    print("5. PERFORMANCE STATISTICS")
    print("-" * 70)

    stats = qi.get_stats()
    print(f"Total queries executed: {stats['total_queries']}")
    print(f"Cache hits: {stats['cache_hits']}")
    print(f"Cache misses: {stats['cache_misses']}")
    print(f"Cache hit rate: {stats['cache_stats']['hit_rate']:.0%}")
    print(f"Average execution time: {stats['avg_execution_time_ms']:.2f}ms")
    print()

    print("Optimizer statistics:")
    opt_stats = stats['optimizer_stats']
    print(f"  Total optimizations: {opt_stats['total_optimizations']}")
    print(f"  Plans explored per optimization: {opt_stats['avg_plans_explored']}")
    if opt_stats['avg_improvement_ratio'] > 0:
        print(f"  Average improvement ratio: {opt_stats['avg_improvement_ratio']:.2f}x")
    print()

    print("=" * 70)
    print("DEMO COMPLETE")
    print("=" * 70)
    print()
    print("Key Takeaways:")
    print("- Same database supports both SQL and Cypher")
    print("- Queries are automatically optimized using ant colony algorithm")
    print("- Stigmergy cache learns from execution history")
    print("- Hybrid queries possible (SQL for tables, Cypher for relationships)")
    print()


if __name__ == "__main__":
    demo_query_languages()
