# Deed Quick Start Guide

Get up and running with Deed in 5 minutes!

## Installation

```bash
git clone https://github.com/yourusername/Deed.git
cd Deed
```

That's it! Deed has **zero external dependencies** for the core prototype.

## Your First Deed Database

```python
from deed import DeedGraph

# 1. Create a database
db = DeedGraph(graph_id="my_first_db")

# 2. Add some data (relational style)
users = db.create_collection("Users")

alice = db.add_entity(
    collection_name="Users",
    properties={
        'name': 'Alice',
        'email': 'alice@example.com',
        'age': 28,
        'city': 'NYC'
    }
)

bob = db.add_entity(
    collection_name="Users",
    properties={
        'name': 'Bob',
        'email': 'bob@example.com',
        'age': 35,
        'city': 'SF'
    }
)

# 3. Create relationships (graph style)
db.add_edge(alice.id, bob.id, "FOLLOWS")

# 4. Query relationally (SQL-like)
users.create_index('city')
nyc_users = users.lookup('city', value='NYC')

print(f"Users in NYC: {[u.get_property('name') for u in nyc_users]}")
# Output: Users in NYC: ['Alice']

# 5. Query with graphs (Cypher-like)
alice_follows = db.traverse(
    alice.id,
    edge_type="FOLLOWS",
    direction="out",
    max_depth=1
)

print(f"Alice follows: {[u.get_property('name') for u in alice_follows]}")
# Output: Alice follows: ['Bob']

# 6. Check statistics
print(db.get_stats())
# {
#   'total_entities': 2,
#   'total_edges': 1,
#   'total_collections': 1,
#   'avg_entity_degree': 1.0,
#   'avg_pheromone': 1.0
# }
```

## Try the Demos

```bash
# Basic hybrid database demo
python examples/demo_basic.py

# Swarm intelligence algorithms demo
python examples/demo_swarm_intelligence.py
```

## Core Concepts

### 1. Entity (Universal Node)

Combines table rows and graph vertices:

```python
user = db.add_entity(
    collection_name="Users",  # Like a table
    properties={'name': 'Alice'}  # Like columns
)
```

### 2. Edge (Relationship)

Directed relationships with properties and pheromones:

```python
edge = db.add_edge(
    source_id=alice.id,
    target_id=bob.id,
    edge_type="FOLLOWS",
    properties={'since': '2025-01-01'}
)

# Edges track usage via pheromones
print(edge.pheromone)  # 1.0 initially
edge.mark_traversed(cost_ms=10.0)
print(edge.pheromone)  # Increased!
```

### 3. Collection (Table)

Groups entities of the same type:

```python
users = db.create_collection("Users")
users.create_index('age')  # B+ tree index

# Fast lookups
young_users = users.lookup('age', min_value=18, max_value=25)

# Scans
all_users = users.scan()

# Filters
active = users.filter(lambda e: e.get_property('active') == True)
```

### 4. Graph Traversal

Navigate relationships:

```python
# Follow edges outward
friends = db.traverse(
    start_id=alice.id,
    edge_type="FRIENDS",
    direction="out",
    max_depth=2  # Friends of friends
)

# Find incoming edges
followers = db.traverse(
    start_id=alice.id,
    edge_type="FOLLOWS",
    direction="in",
    max_depth=1
)

# Both directions
connections = db.traverse(
    start_id=alice.id,
    edge_type="KNOWS",
    direction="both",
    max_depth=1
)

# With filtering
local_friends = db.traverse(
    start_id=alice.id,
    edge_type="FRIENDS",
    direction="out",
    max_depth=1,
    filter_fn=lambda e: e.get_property('city') == 'NYC'
)
```

## Biological Algorithms

### Stigmergy Cache

Learn from query execution history:

```python
from deed.algorithms import StigmergyCache

cache = StigmergyCache()

# Record successful query execution
cache.add_trail(
    query={'operation': 'scan', 'collection': 'Users'},
    execution_plan={'use_indexes': ['age'], 'filter_order': ['age', 'city']},
    execution_time_ms=45.0,
    success=True
)

# Later queries benefit from history
best_plan = cache.get_best_plan(query)
```

### Ant Colony Optimizer

Parallel query plan exploration:

```python
from deed.algorithms import AntColonyOptimizer

optimizer = AntColonyOptimizer(num_ants=20, num_iterations=5)

best_plan = optimizer.optimize(
    query={'joins': ['Users', 'Orders'], 'filters': {...}},
    graph_stats={'avg_scan_cost': 100.0}
)
```

### Bee Quorum Consensus

Fast distributed decisions:

```python
from deed.algorithms import BeeQuorumConsensus

consensus = BeeQuorumConsensus(quorum_threshold=15)

chosen = consensus.reach_consensus(
    options=[
        {'id': 'option_a', 'estimated_quality': 0.9},
        {'id': 'option_b', 'estimated_quality': 0.7},
    ],
    num_scouts=25,
    evaluation_context={}
)
```

### Physarum Network

Self-optimizing topology:

```python
from deed.algorithms import PhysarumReconfiguration

network = PhysarumReconfiguration()

# Add network edges
network.add_edge('e1', 'shard_a', 'shard_b')

# Record usage
network.record_usage('e1', flow_mb=5.0, latency_ms=15.0)

# Reconfigure (strengthen hot paths, prune cold ones)
changes = network.reconfigure()
```

## Next Steps

1. **Read the [Architecture Guide](ARCHITECTURE.md)** - Understand the biological principles
2. **Explore the demos** - See real examples in action
3. **Read the research** - Check out the papers that inspired Deed
4. **Contribute** - Help build the future of databases!

## Common Patterns

### E-commerce Database

```python
db = DeedGraph("ecommerce")

# Collections
users = db.create_collection("Users")
products = db.create_collection("Products")
orders = db.create_collection("Orders")

# Add data
user = db.add_entity("Users", {'name': 'Alice'})
product = db.add_entity("Products", {'name': 'Laptop', 'price': 999})
order = db.add_entity("Orders", {'total': 999, 'status': 'shipped'})

# Relationships
db.add_edge(user.id, order.id, "PLACED")
db.add_edge(order.id, product.id, "CONTAINS")

# Queries
# "What did Alice buy?"
purchases = db.traverse(user.id, "PLACED", "out", max_depth=2)

# "Who bought this product?"
buyers = db.traverse(product.id, "CONTAINS", "in", max_depth=2)
```

### Social Network

```python
db = DeedGraph("social")

users = db.create_collection("Users")
alice = db.add_entity("Users", {'name': 'Alice'})
bob = db.add_entity("Users", {'name': 'Bob'})

# Bidirectional friendship
db.add_edge(alice.id, bob.id, "FRIENDS")
db.add_edge(bob.id, alice.id, "FRIENDS")

# Directed follows
db.add_edge(alice.id, bob.id, "FOLLOWS")

# Find friends
friends = db.traverse(alice.id, "FRIENDS", "both")

# Find followers
followers = db.traverse(alice.id, "FOLLOWS", "in")
```

### Recommendation Engine

```python
# Find similar users (collaborative filtering)
def find_similar_users(user_id):
    # Get items this user liked
    liked = db.traverse(user_id, "LIKED", "out")
    liked_ids = {item.id for item in liked}

    # Find other users who liked the same items
    similar_users = set()
    for item in liked:
        other_users = db.traverse(item.id, "LIKED", "in")
        similar_users.update(other_users)

    # Filter out self
    similar_users = [u for u in similar_users if u.id != user_id]
    return similar_users

# Recommend items
def recommend_items(user_id):
    similar = find_similar_users(user_id)

    # Find items liked by similar users
    recommendations = set()
    for user in similar:
        liked = db.traverse(user.id, "LIKED", "out")
        recommendations.update(liked)

    # Filter out items user already liked
    user_liked = db.traverse(user_id, "LIKED", "out")
    user_liked_ids = {item.id for item in user_liked}

    return [r for r in recommendations if r.id not in user_liked_ids]
```

## Performance Tips

1. **Create indexes** for frequently-queried properties
2. **Use filters** to reduce traversal scope
3. **Limit depth** in traversals to avoid expensive operations
4. **Let pheromones build** - frequently-used paths get faster over time
5. **Use collections** to group similar entities for faster scans

## Troubleshooting

**Q: Queries are slow**
- Create indexes on filter properties
- Reduce traversal depth
- Check if pheromones need time to build (system learns over time)

**Q: High memory usage**
- Limit collection sizes
- Use filters instead of scanning everything
- Implement pagination in application layer

**Q: Can't find entities**
- Verify entity IDs
- Check collection membership
- Ensure indexes are created

## Get Help

- **GitHub Issues**: Bug reports
- **Discussions**: Questions and ideas
- **Documentation**: [ARCHITECTURE.md](ARCHITECTURE.md)

---

**Welcome to Deed - where biology meets databases! 🧬💾**
