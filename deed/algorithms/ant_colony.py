"""
Ant Colony Optimization for query planning.

Inspired by how ants find optimal foraging paths through pheromone trails,
this module explores multiple query execution strategies in parallel and
converges on the best plan through reinforcement learning.

Key principles:
- Multiple "ants" explore different query plans simultaneously
- Successful plans deposit pheromones
- Pheromones guide future exploration
- Balance exploration (trying new plans) vs exploitation (using known good plans)
"""

from typing import Dict, List, Any, Optional, Tuple
from dataclasses import dataclass
import random
import copy
from deed.algorithms.stigmergy import StigmergyCache


@dataclass
class QueryAnt:
    """
    A lightweight agent that explores one query execution strategy.

    Like a real ant scouting for food, each QueryAnt tests a different
    approach to executing the query and reports back its findings.
    """

    ant_id: int
    query: Dict[str, Any]

    # Exploration path
    execution_plan: Dict[str, Any] = None
    estimated_cost: float = float('inf')

    # Pheromone guidance
    pheromone_sensitivity: float = 0.7  # How much to follow existing trails
    exploration_rate: float = 0.3       # How much to explore randomly

    def explore(
        self,
        graph_stats: Dict[str, Any],
        pheromone_map: StigmergyCache
    ) -> Dict[str, Any]:
        """
        Explore one possible execution plan for the query.

        Args:
            graph_stats: Database statistics for cost estimation
            pheromone_map: Existing pheromone trails to guide exploration

        Returns:
            Execution plan dictionary
        """
        # Check for existing pheromone trails
        trails = pheromone_map.lookup(self.query)

        # Decide: follow pheromones or explore randomly?
        if trails and random.random() < self.pheromone_sensitivity:
            # Follow strongest trail (exploitation)
            base_plan = trails[0].execution_plan

            # Add small random variation (avoid pure copying)
            plan = self._vary_plan(base_plan)
        else:
            # Explore randomly (exploration)
            plan = self._generate_random_plan()

        # Estimate cost of this plan
        self.execution_plan = plan
        self.estimated_cost = self._estimate_cost(plan, graph_stats)

        return plan

    def _generate_random_plan(self) -> Dict[str, Any]:
        """
        Generate a random execution plan.

        This is pure exploration - trying something new.
        """
        plan = {
            'operation': self.query.get('operation'),
            'steps': [],
        }

        # For joins: randomize join order
        if 'joins' in self.query:
            joins = list(self.query['joins'])
            random.shuffle(joins)
            plan['join_order'] = joins

        # For filters: randomize filter application order
        if 'filters' in self.query:
            filters = list(self.query['filters'].items())
            random.shuffle(filters)
            plan['filter_order'] = [f[0] for f in filters]

        # For indexes: randomly choose to use index or scan
        if 'indexed_properties' in self.query:
            plan['use_indexes'] = random.sample(
                self.query['indexed_properties'],
                k=random.randint(0, len(self.query['indexed_properties']))
            )

        # For traversals: randomize breadth-first vs depth-first
        if 'traversals' in self.query:
            plan['traversal_strategy'] = random.choice(['bfs', 'dfs', 'bidirectional'])

        return plan

    def _vary_plan(self, base_plan: Dict[str, Any]) -> Dict[str, Any]:
        """
        Create a small variation of an existing plan.

        This adds exploration while still leveraging known-good plans.
        """
        plan = copy.deepcopy(base_plan)

        # Randomly mutate one aspect
        mutation = random.random()

        if mutation < 0.3 and 'join_order' in plan:
            # Swap two joins
            if len(plan['join_order']) >= 2:
                i, j = random.sample(range(len(plan['join_order'])), 2)
                plan['join_order'][i], plan['join_order'][j] = \
                    plan['join_order'][j], plan['join_order'][i]

        elif mutation < 0.6 and 'use_indexes' in plan:
            # Add or remove an index choice
            if random.random() < 0.5 and 'indexed_properties' in self.query:
                # Add a random index
                available = set(self.query['indexed_properties']) - set(plan['use_indexes'])
                if available:
                    plan['use_indexes'].append(random.choice(list(available)))
            elif plan['use_indexes']:
                # Remove a random index
                plan['use_indexes'].pop(random.randrange(len(plan['use_indexes'])))

        elif 'traversal_strategy' in plan:
            # Change traversal strategy
            strategies = ['bfs', 'dfs', 'bidirectional']
            strategies.remove(plan['traversal_strategy'])
            plan['traversal_strategy'] = random.choice(strategies)

        return plan

    def _estimate_cost(
        self,
        plan: Dict[str, Any],
        graph_stats: Dict[str, Any]
    ) -> float:
        """
        Estimate execution cost of a plan.

        This is a simplified cost model. Real implementation would
        use cardinality estimation, selectivity, etc.

        Args:
            plan: Execution plan to evaluate
            graph_stats: Database statistics

        Returns:
            Estimated cost (lower is better)
        """
        cost = 0.0

        # Base operation cost
        operation = plan.get('operation')
        if operation == 'scan':
            cost += graph_stats.get('avg_scan_cost', 100.0)
        elif operation == 'lookup':
            cost += graph_stats.get('avg_lookup_cost', 10.0)
        elif operation == 'traverse':
            cost += graph_stats.get('avg_traverse_cost', 50.0)

        # Join costs (later joins are more expensive)
        if 'join_order' in plan:
            join_count = len(plan['join_order'])
            # Each join multiplies intermediate result size
            cost += join_count * 50.0 * (1.5 ** join_count)

        # Index usage reduces cost
        if 'use_indexes' in plan:
            index_count = len(plan['use_indexes'])
            cost *= (0.7 ** index_count)  # Each index reduces cost by 30%

        # Traversal strategy affects cost
        if 'traversal_strategy' in plan:
            if plan['traversal_strategy'] == 'bfs':
                cost *= 1.0  # Baseline
            elif plan['traversal_strategy'] == 'dfs':
                cost *= 0.9  # Slightly more efficient
            elif plan['traversal_strategy'] == 'bidirectional':
                cost *= 0.8  # Most efficient for long paths

        # Add some randomness to simulate real-world variability
        cost *= random.uniform(0.9, 1.1)

        return cost


class AntColonyOptimizer:
    """
    Query optimization using Ant Colony Optimization algorithm.

    Deploys multiple QueryAnts to explore the space of possible
    execution plans, using pheromone trails to guide convergence
    toward optimal strategies.
    """

    def __init__(
        self,
        num_ants: int = 20,
        num_iterations: int = 3,
        pheromone_cache: Optional[StigmergyCache] = None
    ):
        """
        Initialize ant colony optimizer.

        Args:
            num_ants: Number of ants to deploy per iteration
            num_iterations: Number of exploration rounds
            pheromone_cache: Stigmergy cache for pheromone tracking
        """
        self.num_ants = num_ants
        self.num_iterations = num_iterations
        self.pheromone_cache = pheromone_cache or StigmergyCache()

        self.stats = {
            'total_optimizations': 0,
            'avg_plans_explored': 0,
            'avg_improvement_ratio': 0,
        }

    def optimize(
        self,
        query: Dict[str, Any],
        graph_stats: Dict[str, Any]
    ) -> Dict[str, Any]:
        """
        Find optimal execution plan for a query using ACO.

        Args:
            query: Query to optimize
            graph_stats: Database statistics for cost estimation

        Returns:
            Best execution plan found

        Process:
        1. Deploy ants to explore different plans
        2. Evaluate each plan's cost
        3. Deposit pheromones proportional to plan quality
        4. Repeat, with ants increasingly following good pheromone trails
        5. Return best plan found
        """
        best_plan = None
        best_cost = float('inf')
        all_plans = []

        # Iterative exploration with pheromone reinforcement
        for iteration in range(self.num_iterations):
            iteration_plans = []

            # Deploy ants
            for ant_id in range(self.num_ants):
                ant = QueryAnt(
                    ant_id=ant_id,
                    query=query,
                    pheromone_sensitivity=0.5 + (iteration / self.num_iterations) * 0.3,
                    # Increase pheromone sensitivity over iterations
                    # (shift from exploration to exploitation)
                )

                plan = ant.explore(graph_stats, self.pheromone_cache)
                cost = ant.estimated_cost

                iteration_plans.append((plan, cost))
                all_plans.append((plan, cost))

                # Track best
                if cost < best_cost:
                    best_cost = cost
                    best_plan = plan

            # Reinforce pheromones for good plans in this iteration
            # Sort plans by cost (best first)
            iteration_plans.sort(key=lambda x: x[1])

            # Top 20% of ants deposit pheromones
            top_count = max(1, self.num_ants // 5)
            for plan, cost in iteration_plans[:top_count]:
                # Convert cost to execution time estimate (ms)
                execution_time_ms = cost  # Simplified - cost is time

                # Add trail with strong initial pheromone for good plans
                self.pheromone_cache.add_trail(
                    query=query,
                    execution_plan=plan,
                    execution_time_ms=execution_time_ms,
                    success=True
                )

        # Update statistics
        self.stats['total_optimizations'] += 1
        self.stats['avg_plans_explored'] = len(all_plans)

        # Calculate improvement ratio (best vs worst plan found)
        worst_cost = max(cost for _, cost in all_plans)
        if worst_cost > 0:
            improvement = worst_cost / best_cost
            self.stats['avg_improvement_ratio'] = improvement

        return best_plan

    def get_stats(self) -> Dict[str, Any]:
        """Get optimizer statistics."""
        return {
            **self.stats,
            'pheromone_cache': self.pheromone_cache.get_stats(),
        }

    def __repr__(self) -> str:
        return (
            f"AntColonyOptimizer("
            f"ants={self.num_ants}, "
            f"iterations={self.num_iterations}, "
            f"optimizations={self.stats['total_optimizations']})"
        )
