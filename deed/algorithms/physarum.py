"""
Physarum-inspired network reconfiguration.

Based on the slime mold Physarum polycephalum, which dynamically
reconfigures its nutrient transport network to optimize efficiency
while maintaining fault tolerance through redundant pathways.

Key principles:
- Strengthen frequently-used paths (pheromone reinforcement)
- Prune rarely-used paths (resource efficiency)
- Maintain some redundancy (fault tolerance)
- Continuous adaptation (not one-time optimization)
"""

from typing import Dict, List, Set, Any, Optional, Tuple
from datetime import datetime, timedelta
from dataclasses import dataclass
import math


@dataclass
class NetworkEdgeStats:
    """Statistics for a network edge (connection between nodes/shards)."""

    edge_id: str
    source_node: str
    target_node: str

    # Usage metrics
    flow_volume: float = 0.0  # Data transferred (MB)
    traversal_count: int = 0  # Number of queries using this edge
    avg_latency_ms: float = 0.0  # Average latency

    # Pheromone analog
    strength: float = 1.0  # Edge strength (higher = more important)

    # Temporal
    last_used: Optional[datetime] = None
    created_at: datetime = None

    def __post_init__(self):
        if self.created_at is None:
            self.created_at = datetime.now()
        if self.last_used is None:
            self.last_used = datetime.now()


class PhysarumReconfiguration:
    """
    Adaptive network topology management using Physarum algorithm.

    Continuously monitors and reconfigures the network of database
    nodes/shards to optimize for:
    - Fast access to frequently-used data
    - Redundant paths for fault tolerance
    - Efficient resource usage (don't maintain unused connections)

    Biological analogy: Slime mold forms efficient transport networks
    by thickening tubes that carry nutrients and thinning unused tubes.
    """

    def __init__(
        self,
        reinforcement_rate: float = 0.1,
        decay_rate: float = 0.05,
        min_strength: float = 0.1,
        max_strength: float = 10.0,
        redundancy_factor: int = 2  # Maintain this many redundant paths
    ):
        """
        Initialize Physarum reconfiguration.

        Args:
            reinforcement_rate: How much to strengthen used edges
            decay_rate: How much to weaken unused edges
            min_strength: Minimum edge strength before pruning
            max_strength: Maximum edge strength (cap)
            redundancy_factor: Number of redundant paths to maintain
        """
        self.reinforcement_rate = reinforcement_rate
        self.decay_rate = decay_rate
        self.min_strength = min_strength
        self.max_strength = max_strength
        self.redundancy_factor = redundancy_factor

        # Network state
        self._edges: Dict[str, NetworkEdgeStats] = {}

        # Node connections: node_id -> set of connected node_ids
        self._adjacency: Dict[str, Set[str]] = {}

        self.stats = {
            'total_reconfigurations': 0,
            'edges_strengthened': 0,
            'edges_weakened': 0,
            'edges_pruned': 0,
            'redundant_paths_added': 0,
        }

    def add_edge(
        self,
        edge_id: str,
        source_node: str,
        target_node: str,
        initial_strength: float = 1.0
    ) -> NetworkEdgeStats:
        """
        Add a network edge.

        Args:
            edge_id: Unique edge identifier
            source_node: Source node/shard ID
            target_node: Target node/shard ID
            initial_strength: Initial edge strength

        Returns:
            Created NetworkEdgeStats
        """
        edge = NetworkEdgeStats(
            edge_id=edge_id,
            source_node=source_node,
            target_node=target_node,
            strength=initial_strength
        )

        self._edges[edge_id] = edge

        # Update adjacency
        if source_node not in self._adjacency:
            self._adjacency[source_node] = set()
        if target_node not in self._adjacency:
            self._adjacency[target_node] = set()

        self._adjacency[source_node].add(target_node)

        return edge

    def record_usage(
        self,
        edge_id: str,
        flow_mb: float,
        latency_ms: float
    ) -> None:
        """
        Record that an edge was used for data transfer.

        Args:
            edge_id: Edge that was used
            flow_mb: Amount of data transferred (MB)
            latency_ms: Transfer latency (milliseconds)
        """
        if edge_id not in self._edges:
            return

        edge = self._edges[edge_id]

        # Update statistics
        edge.traversal_count += 1
        edge.flow_volume += flow_mb
        edge.last_used = datetime.now()

        # Update exponential moving average of latency
        alpha = 0.3
        if edge.avg_latency_ms == 0:
            edge.avg_latency_ms = latency_ms
        else:
            edge.avg_latency_ms = (
                alpha * latency_ms + (1 - alpha) * edge.avg_latency_ms
            )

    def reconfigure(self) -> Dict[str, Any]:
        """
        Perform one reconfiguration cycle.

        This is the core Physarum algorithm:
        1. Reinforce (strengthen) frequently-used edges
        2. Decay (weaken) unused edges
        3. Prune very weak edges
        4. Add redundant paths where needed

        Returns:
            Summary of changes made

        Should be called periodically (e.g., every minute).
        """
        changes = {
            'strengthened': [],
            'weakened': [],
            'pruned': [],
            'added': [],
        }

        current_time = datetime.now()

        # Phase 1: Reinforce and decay
        for edge_id, edge in list(self._edges.items()):
            # Calculate time since last use
            time_since_use = (current_time - edge.last_used).total_seconds()

            if time_since_use < 60:  # Used within last minute
                # Reinforce based on usage
                # More flow and lower latency = stronger reinforcement
                flow_factor = math.log(edge.flow_volume + 1)
                latency_factor = 1.0 / (1.0 + edge.avg_latency_ms / 100.0)

                reinforcement = self.reinforcement_rate * flow_factor * latency_factor
                old_strength = edge.strength
                edge.strength = min(edge.strength + reinforcement, self.max_strength)

                if edge.strength > old_strength:
                    changes['strengthened'].append(edge_id)
                    self.stats['edges_strengthened'] += 1

                # Reset flow volume for next cycle
                edge.flow_volume = 0

            else:
                # Decay unused edge
                old_strength = edge.strength
                edge.strength = max(edge.strength * (1 - self.decay_rate), self.min_strength)

                if edge.strength < old_strength:
                    changes['weakened'].append(edge_id)
                    self.stats['edges_weakened'] += 1

        # Phase 2: Prune very weak, redundant edges
        pruned = self._prune_weak_edges()
        changes['pruned'] = pruned
        self.stats['edges_pruned'] += len(pruned)

        # Phase 3: Add redundant paths where needed
        added = self._ensure_redundancy()
        changes['added'] = added
        self.stats['redundant_paths_added'] += len(added)

        self.stats['total_reconfigurations'] += 1

        return changes

    def _prune_weak_edges(self) -> List[str]:
        """
        Remove edges that are very weak AND redundant.

        Won't prune if it would disconnect the network or
        reduce redundancy below threshold.

        Returns:
            List of pruned edge IDs
        """
        pruned = []

        for edge_id, edge in list(self._edges.items()):
            if edge.strength > self.min_strength:
                continue

            # Check if this edge is redundant (alternate path exists)
            if self._has_alternate_path(edge.source_node, edge.target_node, exclude_edge=edge_id):
                # Safe to prune
                self._remove_edge(edge_id)
                pruned.append(edge_id)

        return pruned

    def _ensure_redundancy(self) -> List[str]:
        """
        Add redundant paths between heavily-used nodes.

        Ensures fault tolerance by maintaining multiple routes
        between important nodes.

        Returns:
            List of added edge IDs
        """
        added = []

        # Find high-strength edges (important connections)
        important_edges = [
            e for e in self._edges.values()
            if e.strength > self.max_strength * 0.7
        ]

        for edge in important_edges:
            # Count existing paths between these nodes
            path_count = self._count_paths(edge.source_node, edge.target_node)

            if path_count < self.redundancy_factor:
                # Need more redundancy - add indirect path
                # Find intermediate node
                intermediate = self._find_intermediate_node(
                    edge.source_node,
                    edge.target_node
                )

                if intermediate:
                    # Add edges to create alternate path
                    # source -> intermediate -> target
                    edge_id_1 = f"{edge.source_node}_{intermediate}_redundant"
                    edge_id_2 = f"{intermediate}_{edge.target_node}_redundant"

                    if edge_id_1 not in self._edges:
                        self.add_edge(
                            edge_id_1,
                            edge.source_node,
                            intermediate,
                            initial_strength=self.min_strength * 2
                        )
                        added.append(edge_id_1)

                    if edge_id_2 not in self._edges:
                        self.add_edge(
                            edge_id_2,
                            intermediate,
                            edge.target_node,
                            initial_strength=self.min_strength * 2
                        )
                        added.append(edge_id_2)

        return added

    def _has_alternate_path(
        self,
        source: str,
        target: str,
        exclude_edge: Optional[str] = None
    ) -> bool:
        """
        Check if path exists between source and target (BFS).

        Args:
            source: Source node
            target: Target node
            exclude_edge: Edge ID to exclude from search

        Returns:
            True if alternate path exists
        """
        if source not in self._adjacency or target not in self._adjacency:
            return False

        visited = set()
        queue = [source]

        while queue:
            current = queue.pop(0)

            if current == target:
                return True

            if current in visited:
                continue

            visited.add(current)

            # Explore neighbors
            if current in self._adjacency:
                for neighbor in self._adjacency[current]:
                    # Find edge to this neighbor
                    edge_id = self._find_edge_between(current, neighbor)

                    # Skip excluded edge
                    if edge_id == exclude_edge:
                        continue

                    if neighbor not in visited:
                        queue.append(neighbor)

        return False

    def _count_paths(
        self,
        source: str,
        target: str,
        max_depth: int = 3
    ) -> int:
        """
        Count number of distinct paths between source and target.

        Args:
            source: Source node
            target: Target node
            max_depth: Maximum path length to consider

        Returns:
            Number of paths found
        """
        # DFS with path tracking
        paths = []

        def dfs(current: str, path: List[str], depth: int):
            if depth > max_depth:
                return

            if current == target:
                paths.append(path[:])
                return

            if current in self._adjacency:
                for neighbor in self._adjacency[current]:
                    if neighbor not in path:  # Avoid cycles
                        path.append(neighbor)
                        dfs(neighbor, path, depth + 1)
                        path.pop()

        dfs(source, [source], 0)
        return len(paths)

    def _find_intermediate_node(
        self,
        source: str,
        target: str
    ) -> Optional[str]:
        """
        Find a good intermediate node for creating redundant path.

        Args:
            source: Source node
            target: Target node

        Returns:
            Node ID, or None if not found
        """
        # Find nodes connected to both source and target
        source_neighbors = self._adjacency.get(source, set())
        target_neighbors = self._adjacency.get(target, set())

        # Prefer nodes that have connections to both
        candidates = []
        for node in self._adjacency.keys():
            if node == source or node == target:
                continue

            # Check if connected to source or target
            connected_to_source = node in source_neighbors
            connected_to_target = target in self._adjacency.get(node, set())

            if connected_to_source or connected_to_target:
                candidates.append(node)

        if candidates:
            return candidates[0]

        return None

    def _find_edge_between(self, source: str, target: str) -> Optional[str]:
        """Find edge ID between two nodes."""
        for edge_id, edge in self._edges.items():
            if edge.source_node == source and edge.target_node == target:
                return edge_id
        return None

    def _remove_edge(self, edge_id: str) -> None:
        """Remove an edge from the network."""
        if edge_id not in self._edges:
            return

        edge = self._edges[edge_id]

        # Remove from adjacency
        if edge.source_node in self._adjacency:
            self._adjacency[edge.source_node].discard(edge.target_node)

        del self._edges[edge_id]

    def get_network_health(self) -> Dict[str, Any]:
        """
        Assess overall network health.

        Returns:
            Health metrics
        """
        if not self._edges:
            return {
                'status': 'empty',
                'avg_strength': 0,
                'redundancy_score': 0,
            }

        strengths = [e.strength for e in self._edges.values()]
        avg_strength = sum(strengths) / len(strengths)

        # Count nodes with redundant connections
        redundant_count = 0
        total_connections = 0

        for node in self._adjacency:
            for neighbor in self._adjacency[node]:
                total_connections += 1
                path_count = self._count_paths(node, neighbor)
                if path_count >= self.redundancy_factor:
                    redundant_count += 1

        redundancy_score = (
            redundant_count / total_connections
            if total_connections > 0
            else 0
        )

        return {
            'status': 'healthy' if redundancy_score > 0.5 else 'degraded',
            'total_edges': len(self._edges),
            'total_nodes': len(self._adjacency),
            'avg_strength': avg_strength,
            'redundancy_score': redundancy_score,
            'stats': self.stats,
        }

    def __repr__(self) -> str:
        return (
            f"PhysarumReconfiguration("
            f"edges={len(self._edges)}, "
            f"nodes={len(self._adjacency)}, "
            f"reconfigs={self.stats['total_reconfigurations']})"
        )
