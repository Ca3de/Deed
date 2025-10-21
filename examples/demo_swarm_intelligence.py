"""
Swarm Intelligence Demo

Demonstrates Deed's biological algorithms in action:
- Ant Colony Optimization for query planning
- Bee Quorum Consensus for distributed decisions
- Stigmergy-based learning from query history
- Physarum network reconfiguration
"""

import sys
sys.path.insert(0, '/home/user/Deed')

from deed.algorithms import (
    StigmergyCache,
    AntColonyOptimizer,
    BeeQuorumConsensus,
    PhysarumReconfiguration
)
import random


def demo_stigmergy():
    """Demonstrate stigmergy-based query optimization caching."""
    print("=" * 70)
    print("1. STIGMERGY - Environmental Learning")
    print("=" * 70)
    print()

    cache = StigmergyCache(max_trails=100)

    # Simulate a common query pattern
    query1 = {
        'operation': 'scan',
        'collection': 'Users',
        'filters': {'age': '>', 'city': '='},
        'joins': ['Orders'],
    }

    print("Simulating repeated executions of a query pattern...")
    print("Query: SELECT Users JOIN Orders WHERE age > ? AND city = ?")
    print()

    # Different execution plans for the same query
    plan_a = {
        'join_order': ['Users', 'Orders'],
        'use_indexes': ['age', 'city'],
        'filter_order': ['city', 'age']  # City first (more selective)
    }

    plan_b = {
        'join_order': ['Orders', 'Users'],
        'use_indexes': ['age'],
        'filter_order': ['age', 'city']  # Age first (less selective)
    }

    # Simulate Plan A being faster
    print("Execution 1: Plan A")
    cache.add_trail(query1, plan_a, execution_time_ms=45.0, success=True)
    print(f"  Execution time: 45ms")
    print()

    print("Execution 2: Plan B")
    cache.add_trail(query1, plan_b, execution_time_ms=120.0, success=True)
    print(f"  Execution time: 120ms")
    print()

    # Execute Plan A multiple times (reinforcing it)
    for i in range(5):
        cache.add_trail(query1, plan_a, execution_time_ms=random.uniform(40, 50), success=True)

    print("Executed Plan A 5 more times (avg ~45ms)")
    print()

    # Look up best plan
    print("Looking up best execution plan from stigmergy cache...")
    trails = cache.lookup(query1)

    if trails:
        best_trail = trails[0]
        print(f"Best plan found: {best_trail.execution_plan}")
        print(f"  Quality score: {best_trail.quality_score():.3f}")
        print(f"  Average time: {best_trail.avg_execution_time_ms:.1f}ms")
        print(f"  Success rate: {best_trail.success_count / (best_trail.success_count + best_trail.failure_count):.0%}")
        print()

    print("The database 'learned' that Plan A is better through stigmergy!")
    print("Future queries will prefer this plan automatically.")
    print()

    # Demonstrate pheromone evaporation
    print("Simulating pheromone evaporation (3 cycles)...")
    for i in range(3):
        cache.evaporate_all()
        trails = cache.lookup(query1)
        if trails:
            print(f"  Cycle {i+1}: Best trail pheromone = {trails[0].pheromone:.3f}")
    print()

    print("Unused trails fade away, allowing adaptation to new patterns.")
    print()

    print(f"Cache stats: {cache.get_stats()}")
    print()


def demo_ant_colony():
    """Demonstrate ant colony optimization for query planning."""
    print("=" * 70)
    print("2. ANT COLONY OPTIMIZATION - Query Planning")
    print("=" * 70)
    print()

    optimizer = AntColonyOptimizer(num_ants=20, num_iterations=5)

    # Complex query with many possible execution plans
    complex_query = {
        'operation': 'join',
        'collection': 'Users',
        'joins': ['Orders', 'Products', 'Reviews'],
        'filters': {
            'user_age': '>',
            'product_category': '=',
            'review_rating': '>=',
        },
        'indexed_properties': ['user_age', 'product_category', 'review_rating'],
        'traversals': []
    }

    # Simplified database statistics
    graph_stats = {
        'avg_scan_cost': 100.0,
        'avg_lookup_cost': 10.0,
        'avg_traverse_cost': 50.0,
    }

    print("Complex query to optimize:")
    print("  SELECT * FROM Users")
    print("    JOIN Orders ON Users.id = Orders.user_id")
    print("    JOIN Products ON Orders.product_id = Products.id")
    print("    JOIN Reviews ON Products.id = Reviews.product_id")
    print("  WHERE age > 25 AND category = 'Electronics' AND rating >= 4.0")
    print()

    print(f"Deploying {optimizer.num_ants} ants for {optimizer.num_iterations} iterations...")
    print("Each ant explores a different execution strategy.")
    print()

    # Run optimization
    best_plan = optimizer.optimize(complex_query, graph_stats)

    print("Optimization complete!")
    print()
    print("Best execution plan found:")
    print(f"  Join order: {best_plan.get('join_order', 'N/A')}")
    print(f"  Indexes to use: {best_plan.get('use_indexes', 'N/A')}")
    print(f"  Filter order: {best_plan.get('filter_order', 'N/A')}")
    print()

    # Show optimizer statistics
    print("Ant Colony Statistics:")
    stats = optimizer.get_stats()
    print(f"  Total optimizations: {stats['total_optimizations']}")
    print(f"  Plans explored: {stats['avg_plans_explored']}")
    print(f"  Improvement ratio: {stats['avg_improvement_ratio']:.2f}x")
    print()

    print("Ants explored multiple plans in parallel and converged on the best!")
    print()


def demo_bee_consensus():
    """Demonstrate bee quorum consensus for distributed decisions."""
    print("=" * 70)
    print("3. BEE QUORUM CONSENSUS - Distributed Decisions")
    print("=" * 70)
    print()

    consensus = BeeQuorumConsensus(quorum_threshold=12, max_rounds=10)

    # Scenario: Choose the best replica to read from
    print("Scenario: Multiple database replicas available for read query")
    print("Which replica should we use?")
    print()

    replica_options = [
        {
            'id': 'replica_us_east',
            'estimated_quality': 0.9,  # High quality
            'location': 'US-East',
            'latency_ms': 15,
            'load': 0.3,
        },
        {
            'id': 'replica_us_west',
            'estimated_quality': 0.7,  # Medium quality
            'location': 'US-West',
            'latency_ms': 45,
            'load': 0.2,
        },
        {
            'id': 'replica_eu',
            'estimated_quality': 0.5,  # Lower quality
            'location': 'EU',
            'latency_ms': 120,
            'load': 0.8,  # High load
        },
        {
            'id': 'replica_asia',
            'estimated_quality': 0.8,  # Good quality
            'location': 'Asia',
            'latency_ms': 180,
            'load': 0.1,  # Low load
        },
    ]

    for replica in replica_options:
        print(f"  {replica['id']}:")
        print(f"    Quality: {replica['estimated_quality']:.1f}, Latency: {replica['latency_ms']}ms, Load: {replica['load']:.0%}")

    print()

    # Deploy scout bees
    num_scouts = 25
    print(f"Deploying {num_scouts} scout bees to evaluate replicas...")
    print()

    chosen_replica = consensus.reach_consensus(
        options=replica_options,
        num_scouts=num_scouts,
        evaluation_context={'load': 0.0}  # Simulated context
    )

    print("Consensus reached!")
    print()
    print(f"Chosen replica: {chosen_replica['id']}")
    print(f"  Quality: {chosen_replica['estimated_quality']:.1f}")
    print(f"  Latency: {chosen_replica['latency_ms']}ms")
    print(f"  Location: {chosen_replica['location']}")
    print()

    # Show consensus statistics
    print("Bee Consensus Statistics:")
    stats = consensus.get_stats()
    print(f"  Total decisions: {stats['total_decisions']}")
    print(f"  Avg rounds to consensus: {stats['avg_rounds_to_consensus']:.1f}")
    print(f"  Quorum success rate: {stats['quorum_success_rate']:.0%}")
    print()

    print("Bees quickly converged on the best option through distributed evaluation!")
    print("No centralized voting protocol needed.")
    print()


def demo_physarum():
    """Demonstrate Physarum network reconfiguration."""
    print("=" * 70)
    print("4. PHYSARUM - Adaptive Network Reconfiguration")
    print("=" * 70)
    print()

    network = PhysarumReconfiguration(
        reinforcement_rate=0.2,
        decay_rate=0.1,
        redundancy_factor=2
    )

    print("Simulating a distributed database network with 5 nodes (shards)")
    print()

    # Create network topology (nodes/shards and connections)
    nodes = ['shard_A', 'shard_B', 'shard_C', 'shard_D', 'shard_E']

    # Initial mesh network
    edges = [
        ('A_B', 'shard_A', 'shard_B'),
        ('A_C', 'shard_A', 'shard_C'),
        ('B_C', 'shard_B', 'shard_C'),
        ('B_D', 'shard_B', 'shard_D'),
        ('C_D', 'shard_C', 'shard_D'),
        ('C_E', 'shard_C', 'shard_E'),
        ('D_E', 'shard_D', 'shard_E'),
    ]

    print("Initial network topology:")
    for edge_id, source, target in edges:
        network.add_edge(edge_id, source, target)
        print(f"  {source} <-> {target}")

    print()

    # Simulate query workload (some paths used heavily, others rarely)
    print("Simulating query workload over time...")
    print()

    # Heavy traffic between A and B
    for _ in range(20):
        network.record_usage('A_B', flow_mb=random.uniform(5, 10), latency_ms=random.uniform(10, 20))

    # Moderate traffic between B and D
    for _ in range(10):
        network.record_usage('B_D', flow_mb=random.uniform(2, 5), latency_ms=random.uniform(15, 25))

    # Heavy traffic between C and E
    for _ in range(18):
        network.record_usage('C_E', flow_mb=random.uniform(4, 8), latency_ms=random.uniform(12, 18))

    # Low traffic on other edges
    network.record_usage('A_C', flow_mb=1.0, latency_ms=30)
    network.record_usage('C_D', flow_mb=0.5, latency_ms=35)

    print("Hottest paths:")
    print("  - shard_A <-> shard_B (20 queries)")
    print("  - shard_C <-> shard_E (18 queries)")
    print("  - shard_B <-> shard_D (10 queries)")
    print()

    # Run reconfiguration
    print("Running Physarum reconfiguration algorithm...")
    print()

    changes = network.reconfigure()

    print("Reconfiguration results:")
    if changes['strengthened']:
        print(f"  Strengthened edges: {changes['strengthened']}")
    if changes['weakened']:
        print(f"  Weakened edges: {changes['weakened']}")
    if changes['pruned']:
        print(f"  Pruned edges: {changes['pruned']}")
    if changes['added']:
        print(f"  Added redundant paths: {changes['added']}")
    print()

    # Show network health
    health = network.get_network_health()
    print("Network health assessment:")
    print(f"  Status: {health['status']}")
    print(f"  Total edges: {health['total_edges']}")
    print(f"  Total nodes: {health['total_nodes']}")
    print(f"  Average edge strength: {health['avg_strength']:.2f}")
    print(f"  Redundancy score: {health['redundancy_score']:.0%}")
    print()

    print("The network self-optimized:")
    print("  - Hot paths strengthened (like slime mold thickening nutrient tubes)")
    print("  - Unused paths weakened (resource efficiency)")
    print("  - Redundant paths maintained (fault tolerance)")
    print()


def demo_all():
    """Run all swarm intelligence demonstrations."""
    demo_stigmergy()
    print("\n" * 2)

    demo_ant_colony()
    print("\n" * 2)

    demo_bee_consensus()
    print("\n" * 2)

    demo_physarum()
    print("\n" * 2)

    print("=" * 70)
    print("SWARM INTELLIGENCE DEMO COMPLETE")
    print("=" * 70)
    print()
    print("Deed database combines multiple biological algorithms:")
    print("  1. Stigmergy - Learn from execution history")
    print("  2. Ant Colony - Parallel query plan exploration")
    print("  3. Bee Quorum - Fast distributed consensus")
    print("  4. Physarum - Self-optimizing network topology")
    print()
    print("Result: A database that adapts, optimizes, and heals itself!")
    print()


if __name__ == "__main__":
    demo_all()
