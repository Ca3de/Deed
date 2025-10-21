"""
DeedGraph: The main database interface.

This is the unified graph structure that serves as both a property graph
and a relational database. It manages entities, edges, collections, and
coordinates biological-inspired algorithms.
"""

from typing import Dict, List, Set, Optional, Any, Callable
from deed.core.entity import Entity
from deed.core.edge import Edge
from deed.core.collection import Collection
from collections import deque
import time


class DeedGraph:
    """
    The core Deed database structure.

    Implements a unified property graph that supports:
    - Graph operations (add nodes/edges, traverse relationships)
    - Relational operations (tables/collections, indexes, SQL-like queries)
    - Biological algorithms (pheromone tracking, adaptive routing)

    This is a single-node version. Distributed version will use sharding
    with octopus-style peer-to-peer communication between shards.
    """

    def __init__(self, graph_id: str = "deed_db"):
        """
        Initialize a Deed database.

        Args:
            graph_id: Unique identifier for this database instance
        """
        self.graph_id = graph_id

        # Entity storage
        self._entities: Dict[str, Entity] = {}

        # Edge storage (edge_id -> Edge)
        self._edges: Dict[str, Edge] = {}

        # Adjacency lists for fast traversal
        # source_id -> {edge_type -> {target_id -> edge_id}}
        self._outgoing: Dict[str, Dict[str, Dict[str, str]]] = {}
        # target_id -> {edge_type -> {source_id -> edge_id}}
        self._incoming: Dict[str, Dict[str, Dict[str, str]]] = {}

        # Collections (table-like groupings)
        self._collections: Dict[str, Collection] = {}

        # Biological algorithms state
        self._pheromone_decay_active = False

        # Statistics
        self.stats = {
            'total_entities': 0,
            'total_edges': 0,
            'total_collections': 0,
            'queries_executed': 0,
        }

    # ========================================
    # Entity Operations
    # ========================================

    def add_entity(
        self,
        entity_type: Optional[str] = None,
        properties: Optional[Dict[str, Any]] = None,
        entity_id: Optional[str] = None,
        collection_name: Optional[str] = None
    ) -> Entity:
        """
        Create and add a new entity.

        Args:
            entity_type: Entity type (if not using collection)
            properties: Property key-value pairs
            entity_id: Specific ID (auto-generated if None)
            collection_name: Add to this collection (creates if doesn't exist)

        Returns:
            Created Entity

        Example:
            # Create a user entity
            user = graph.add_entity(
                collection_name='Users',
                properties={'name': 'Alice', 'age': 30}
            )
        """
        entity = Entity(
            entity_id=entity_id,
            entity_type=entity_type or collection_name or "Unknown",
            properties=properties
        )

        self._entities[entity.id] = entity

        # Initialize adjacency lists
        self._outgoing[entity.id] = {}
        self._incoming[entity.id] = {}

        # Add to collection if specified
        if collection_name:
            collection = self.get_or_create_collection(collection_name)
            collection.add_entity(entity)

        self.stats['total_entities'] += 1
        return entity

    def get_entity(self, entity_id: str) -> Optional[Entity]:
        """Retrieve entity by ID."""
        entity = self._entities.get(entity_id)
        if entity:
            entity.mark_accessed()  # Pheromone tracking
        return entity

    def remove_entity(self, entity_id: str) -> bool:
        """
        Remove an entity and all its edges.

        Args:
            entity_id: ID of entity to remove

        Returns:
            True if removed, False if not found
        """
        if entity_id not in self._entities:
            return False

        entity = self._entities[entity_id]

        # Remove from collection
        if entity.type in self._collections:
            self._collections[entity.type].remove_entity(entity_id)

        # Remove all connected edges
        # Outgoing edges
        if entity_id in self._outgoing:
            for edge_type, targets in self._outgoing[entity_id].items():
                for target_id, edge_id in targets.items():
                    self._edges.pop(edge_id, None)
                    # Remove from incoming adjacency list of target
                    if target_id in self._incoming:
                        if edge_type in self._incoming[target_id]:
                            self._incoming[target_id][edge_type].pop(entity_id, None)

        # Incoming edges
        if entity_id in self._incoming:
            for edge_type, sources in self._incoming[entity_id].items():
                for source_id, edge_id in sources.items():
                    self._edges.pop(edge_id, None)
                    # Remove from outgoing adjacency list of source
                    if source_id in self._outgoing:
                        if edge_type in self._outgoing[source_id]:
                            self._outgoing[source_id][edge_type].pop(entity_id, None)

        # Remove adjacency lists
        self._outgoing.pop(entity_id, None)
        self._incoming.pop(entity_id, None)

        # Remove entity
        del self._entities[entity_id]
        self.stats['total_entities'] -= 1

        return True

    # ========================================
    # Edge Operations
    # ========================================

    def add_edge(
        self,
        source_id: str,
        target_id: str,
        edge_type: str,
        properties: Optional[Dict[str, Any]] = None
    ) -> Optional[Edge]:
        """
        Create a directed edge between two entities.

        Args:
            source_id: Source entity ID
            target_id: Target entity ID
            edge_type: Relationship type (e.g., "FOLLOWS", "PURCHASED")
            properties: Optional edge properties

        Returns:
            Created Edge, or None if source/target doesn't exist

        Example:
            # Alice follows Bob
            graph.add_edge(alice.id, bob.id, "FOLLOWS")
        """
        if source_id not in self._entities or target_id not in self._entities:
            return None

        edge = Edge(
            source_id=source_id,
            target_id=target_id,
            edge_type=edge_type,
            properties=properties
        )

        self._edges[edge.id] = edge

        # Update adjacency lists
        if edge_type not in self._outgoing[source_id]:
            self._outgoing[source_id][edge_type] = {}
        self._outgoing[source_id][edge_type][target_id] = edge.id

        if edge_type not in self._incoming[target_id]:
            self._incoming[target_id][edge_type] = {}
        self._incoming[target_id][edge_type][source_id] = edge.id

        # Update entity references
        self._entities[source_id].add_outgoing_edge(edge_type, target_id)
        self._entities[target_id].add_incoming_edge(edge_type, source_id)

        self.stats['total_edges'] += 1
        return edge

    def get_edge(self, edge_id: str) -> Optional[Edge]:
        """Retrieve edge by ID."""
        return self._edges.get(edge_id)

    def get_edges_between(
        self,
        source_id: str,
        target_id: str,
        edge_type: Optional[str] = None
    ) -> List[Edge]:
        """
        Find all edges from source to target.

        Args:
            source_id: Source entity ID
            target_id: Target entity ID
            edge_type: Filter by edge type (None for all types)

        Returns:
            List of edges
        """
        if source_id not in self._outgoing:
            return []

        edges = []

        if edge_type:
            # Specific edge type
            if edge_type in self._outgoing[source_id]:
                if target_id in self._outgoing[source_id][edge_type]:
                    edge_id = self._outgoing[source_id][edge_type][target_id]
                    edges.append(self._edges[edge_id])
        else:
            # All edge types
            for edge_type_dict in self._outgoing[source_id].values():
                if target_id in edge_type_dict:
                    edge_id = edge_type_dict[target_id]
                    edges.append(self._edges[edge_id])

        return edges

    # ========================================
    # Collection Operations (Table-like)
    # ========================================

    def create_collection(self, name: str, schema: Optional[Dict[str, type]] = None) -> Collection:
        """
        Create a new collection (table).

        Args:
            name: Collection name
            schema: Optional property type hints

        Returns:
            Created Collection
        """
        if name in self._collections:
            return self._collections[name]

        collection = Collection(name, schema)
        self._collections[name] = collection
        self.stats['total_collections'] += 1
        return collection

    def get_or_create_collection(self, name: str) -> Collection:
        """Get existing collection or create if doesn't exist."""
        if name not in self._collections:
            return self.create_collection(name)
        return self._collections[name]

    def get_collection(self, name: str) -> Optional[Collection]:
        """Retrieve collection by name."""
        return self._collections.get(name)

    def drop_collection(self, name: str) -> bool:
        """
        Remove a collection and all its entities.

        Args:
            name: Collection name

        Returns:
            True if removed, False if not found
        """
        if name not in self._collections:
            return False

        collection = self._collections[name]

        # Remove all entities in collection
        entity_ids = list(collection._entities.keys())
        for entity_id in entity_ids:
            self.remove_entity(entity_id)

        del self._collections[name]
        self.stats['total_collections'] -= 1
        return True

    # ========================================
    # Graph Traversal
    # ========================================

    def traverse(
        self,
        start_id: str,
        edge_type: Optional[str] = None,
        direction: str = "out",
        max_depth: int = 1,
        filter_fn: Optional[Callable[[Entity], bool]] = None
    ) -> List[Entity]:
        """
        Traverse graph from a starting entity.

        Args:
            start_id: Starting entity ID
            edge_type: Filter by edge type (None for all)
            direction: "out" (follow outgoing), "in" (incoming), "both"
            max_depth: Maximum traversal depth
            filter_fn: Optional predicate to filter results

        Returns:
            List of reachable entities

        Example:
            # Find friends of friends (2 hops)
            graph.traverse(alice.id, edge_type="FRIENDS", max_depth=2)
        """
        if start_id not in self._entities:
            return []

        visited: Set[str] = set()
        result: List[Entity] = []
        queue: deque = deque([(start_id, 0)])  # (entity_id, depth)

        while queue:
            current_id, depth = queue.popleft()

            if current_id in visited:
                continue

            visited.add(current_id)
            current_entity = self._entities[current_id]

            # Apply filter
            if filter_fn and not filter_fn(current_entity):
                continue

            # Don't include start node in results
            if current_id != start_id:
                result.append(current_entity)

            # Continue traversal if within depth limit
            if depth < max_depth:
                neighbors = set()

                if direction in ("out", "both"):
                    neighbors.update(current_entity.get_outgoing_neighbors(edge_type))

                if direction in ("in", "both"):
                    neighbors.update(current_entity.get_incoming_neighbors(edge_type))

                for neighbor_id in neighbors:
                    if neighbor_id not in visited:
                        queue.append((neighbor_id, depth + 1))

        return result

    # ========================================
    # Biological Algorithms
    # ========================================

    def evaporate_pheromones(self, decay_rate: Optional[float] = None) -> None:
        """
        Apply pheromone evaporation to all edges.

        This is the "slime mold cleanup" process - edges that aren't
        used frequently gradually lose their pheromone, allowing
        the network to adapt to changing query patterns.

        Called periodically by background process.
        """
        for edge in self._edges.values():
            edge.evaporate_pheromone()

    def get_strongest_path(
        self,
        source_id: str,
        target_id: str,
        edge_type: Optional[str] = None
    ) -> Optional[List[str]]:
        """
        Find path with highest pheromone concentration (ant-inspired).

        This is like following the strongest ant trail - the path
        that has been most successfully used by previous queries.

        Args:
            source_id: Start entity
            target_id: End entity
            edge_type: Filter by edge type

        Returns:
            List of entity IDs forming the path, or None if no path
        """
        if source_id not in self._entities or target_id not in self._entities:
            return None

        # BFS with pheromone-weighted priority
        visited: Set[str] = set()
        # Priority queue: (negative_pheromone_sum, path)
        queue: List[tuple] = [(0.0, [source_id])]

        while queue:
            # Get path with highest pheromone (lowest negative sum)
            queue.sort(key=lambda x: x[0])
            neg_pheromone, path = queue.pop(0)

            current_id = path[-1]

            if current_id == target_id:
                return path

            if current_id in visited:
                continue

            visited.add(current_id)

            # Explore neighbors
            current = self._entities[current_id]
            neighbors = current.get_outgoing_neighbors(edge_type)

            for neighbor_id in neighbors:
                if neighbor_id not in visited:
                    # Get edge pheromone
                    edges = self.get_edges_between(current_id, neighbor_id, edge_type)
                    if edges:
                        pheromone = edges[0].pheromone
                        new_path = path + [neighbor_id]
                        new_pheromone = neg_pheromone - pheromone  # Negative for max heap
                        queue.append((new_pheromone, new_path))

        return None  # No path found

    # ========================================
    # Utility & Statistics
    # ========================================

    def get_stats(self) -> Dict[str, Any]:
        """Get database statistics."""
        return {
            **self.stats,
            'avg_entity_degree': (
                sum(e.degree() for e in self._entities.values()) / len(self._entities)
                if self._entities else 0
            ),
            'avg_pheromone': (
                sum(e.pheromone for e in self._edges.values()) / len(self._edges)
                if self._edges else 0
            ),
        }

    def __repr__(self) -> str:
        return (
            f"DeedGraph(id={self.graph_id}, "
            f"entities={self.stats['total_entities']}, "
            f"edges={self.stats['total_edges']}, "
            f"collections={self.stats['total_collections']})"
        )
