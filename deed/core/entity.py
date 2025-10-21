"""
Entity: The fundamental node in Deed's property graph.

An Entity represents any data item - whether a row in a table,
a vertex in a graph, or a hybrid object with both properties and relationships.
"""

from typing import Dict, Any, Set, Optional
from uuid import uuid4
from datetime import datetime


class Entity:
    """
    Universal node type in Deed database.

    Combines:
    - Properties (key-value pairs like table columns)
    - Type membership (collection/table grouping)
    - Relationships (graph edges)
    - Location metadata (shard assignment)

    This unified representation allows the same entity to participate
    in both SQL-style queries and graph traversals.
    """

    def __init__(
        self,
        entity_id: Optional[str] = None,
        entity_type: Optional[str] = None,
        properties: Optional[Dict[str, Any]] = None,
        shard_id: Optional[str] = None
    ):
        """
        Initialize an Entity.

        Args:
            entity_id: Unique identifier (auto-generated if not provided)
            entity_type: Collection name (e.g., "Users", "Products")
            properties: Key-value properties
            shard_id: Assigned shard location (for distributed storage)
        """
        self.id = entity_id or str(uuid4())
        self.type = entity_type or "Unknown"
        self.properties = properties or {}
        self.shard_id = shard_id

        # Graph relationships (edge_type -> set of target entity IDs)
        self._outgoing_edges: Dict[str, Set[str]] = {}
        self._incoming_edges: Dict[str, Set[str]] = {}

        # Metadata
        self.created_at = datetime.now()
        self.updated_at = datetime.now()

        # Biological metadata
        self.access_count = 0  # For pheromone calculation
        self.last_accessed = None

    def set_property(self, key: str, value: Any) -> None:
        """Set or update a property value."""
        self.properties[key] = value
        self.updated_at = datetime.now()

    def get_property(self, key: str, default: Any = None) -> Any:
        """Get a property value."""
        return self.properties.get(key, default)

    def has_property(self, key: str) -> bool:
        """Check if property exists."""
        return key in self.properties

    def add_outgoing_edge(self, edge_type: str, target_id: str) -> None:
        """Add an outgoing relationship to another entity."""
        if edge_type not in self._outgoing_edges:
            self._outgoing_edges[edge_type] = set()
        self._outgoing_edges[edge_type].add(target_id)

    def add_incoming_edge(self, edge_type: str, source_id: str) -> None:
        """Add an incoming relationship from another entity."""
        if edge_type not in self._incoming_edges:
            self._incoming_edges[edge_type] = set()
        self._incoming_edges[edge_type].add(source_id)

    def get_outgoing_neighbors(self, edge_type: Optional[str] = None) -> Set[str]:
        """
        Get entity IDs this entity points to.

        Args:
            edge_type: Filter by edge type, or None for all outgoing edges
        """
        if edge_type:
            return self._outgoing_edges.get(edge_type, set())

        # Return all outgoing neighbors across all edge types
        all_neighbors = set()
        for neighbors in self._outgoing_edges.values():
            all_neighbors.update(neighbors)
        return all_neighbors

    def get_incoming_neighbors(self, edge_type: Optional[str] = None) -> Set[str]:
        """
        Get entity IDs pointing to this entity.

        Args:
            edge_type: Filter by edge type, or None for all incoming edges
        """
        if edge_type:
            return self._incoming_edges.get(edge_type, set())

        # Return all incoming neighbors across all edge types
        all_neighbors = set()
        for neighbors in self._incoming_edges.values():
            all_neighbors.update(neighbors)
        return all_neighbors

    def degree(self, direction: str = "both") -> int:
        """
        Calculate node degree.

        Args:
            direction: "out", "in", or "both"
        """
        if direction == "out":
            return sum(len(neighbors) for neighbors in self._outgoing_edges.values())
        elif direction == "in":
            return sum(len(neighbors) for neighbors in self._incoming_edges.values())
        else:  # both
            return self.degree("out") + self.degree("in")

    def mark_accessed(self) -> None:
        """
        Mark this entity as accessed (for pheromone tracking).

        This biological-inspired mechanism tracks usage patterns,
        allowing the database to strengthen frequently-used paths.
        """
        self.access_count += 1
        self.last_accessed = datetime.now()

    def to_dict(self) -> Dict[str, Any]:
        """Serialize entity to dictionary."""
        return {
            'id': self.id,
            'type': self.type,
            'properties': self.properties,
            'shard_id': self.shard_id,
            'created_at': self.created_at.isoformat(),
            'updated_at': self.updated_at.isoformat(),
            'access_count': self.access_count,
            'degree': self.degree()
        }

    def __repr__(self) -> str:
        props = ', '.join(f"{k}={v}" for k, v in list(self.properties.items())[:3])
        if len(self.properties) > 3:
            props += '...'
        return f"Entity(id={self.id[:8]}, type={self.type}, {props})"

    def __eq__(self, other) -> bool:
        if not isinstance(other, Entity):
            return False
        return self.id == other.id

    def __hash__(self) -> int:
        return hash(self.id)
