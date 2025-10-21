"""
Collection: Table-like grouping of homogeneous entities.

Collections provide SQL-familiar operations (scans, indexes, filters)
on top of the underlying graph structure. They're not separate from the graph
- they're just typed subsets with optimized access patterns.
"""

from typing import Dict, List, Set, Any, Optional, Callable
from deed.core.entity import Entity
from collections import defaultdict
import bisect


class Index:
    """
    B+ tree-like index for fast property lookups.

    In the graph representation, this is implemented as a special
    edge type connecting indexed values to entities.
    """

    def __init__(self, property_name: str):
        self.property_name = property_name
        # Sorted list of (value, entity_id) tuples for range queries
        self._sorted_entries: List[tuple] = []
        # Map from value to set of entity IDs for exact matches
        self._value_map: Dict[Any, Set[str]] = defaultdict(set)

    def insert(self, entity: Entity) -> None:
        """Add entity to index."""
        if not entity.has_property(self.property_name):
            return

        value = entity.get_property(self.property_name)
        entity_id = entity.id

        # Add to value map
        self._value_map[value].add(entity_id)

        # Add to sorted list (for range queries)
        bisect.insort(self._sorted_entries, (value, entity_id))

    def remove(self, entity: Entity) -> None:
        """Remove entity from index."""
        if not entity.has_property(self.property_name):
            return

        value = entity.get_property(self.property_name)
        entity_id = entity.id

        # Remove from value map
        if value in self._value_map:
            self._value_map[value].discard(entity_id)
            if not self._value_map[value]:
                del self._value_map[value]

        # Remove from sorted list
        self._sorted_entries = [
            (v, eid) for v, eid in self._sorted_entries
            if not (v == value and eid == entity_id)
        ]

    def lookup_exact(self, value: Any) -> Set[str]:
        """Find entity IDs with exact property value."""
        return self._value_map.get(value, set()).copy()

    def lookup_range(self, min_value: Any = None, max_value: Any = None) -> Set[str]:
        """
        Find entity IDs within value range.

        Args:
            min_value: Minimum value (inclusive), None for unbounded
            max_value: Maximum value (inclusive), None for unbounded
        """
        result = set()

        for value, entity_id in self._sorted_entries:
            if min_value is not None and value < min_value:
                continue
            if max_value is not None and value > max_value:
                break
            result.add(entity_id)

        return result


class Collection:
    """
    A typed grouping of entities with table-like semantics.

    Think of this as a "table" in traditional RDBMS, but implemented
    as a subgraph where all nodes share a common type. Provides:

    - Fast scans (iterate all entities of this type)
    - Indexes (B+ tree style lookups on properties)
    - Filters (SQL WHERE clause equivalents)
    - Schema hints (optional, for optimization)

    Biological analogy: A collection is like a pheromone trail network
    specific to one food source - all ants seeking "User" entities
    follow the "Users" collection trail.
    """

    def __init__(self, name: str, schema: Optional[Dict[str, type]] = None):
        """
        Initialize a Collection.

        Args:
            name: Collection name (e.g., "Users", "Products")
            schema: Optional type hints for properties {property: type}
        """
        self.name = name
        self.schema = schema or {}

        # Entity storage (entity_id -> Entity)
        self._entities: Dict[str, Entity] = {}

        # Indexes (property_name -> Index)
        self._indexes: Dict[str, Index] = {}

        # Statistics for query optimization
        self.stats = {
            'count': 0,
            'avg_properties': 0,
            'avg_degree': 0,
        }

    def add_entity(self, entity: Entity) -> None:
        """
        Add an entity to this collection.

        Args:
            entity: Entity to add (will update its type to match collection)
        """
        entity.type = self.name
        self._entities[entity.id] = entity

        # Update all indexes
        for index in self._indexes.values():
            index.insert(entity)

        # Update statistics
        self._update_stats()

    def remove_entity(self, entity_id: str) -> Optional[Entity]:
        """
        Remove an entity from collection.

        Args:
            entity_id: ID of entity to remove

        Returns:
            Removed entity, or None if not found
        """
        entity = self._entities.pop(entity_id, None)
        if entity:
            # Remove from all indexes
            for index in self._indexes.values():
                index.remove(entity)
            self._update_stats()

        return entity

    def get_entity(self, entity_id: str) -> Optional[Entity]:
        """Retrieve entity by ID."""
        return self._entities.get(entity_id)

    def scan(self) -> List[Entity]:
        """
        Full table scan: return all entities.

        Biological analogy: Broadcasting pheromone to all entities of this type.
        """
        return list(self._entities.values())

    def filter(self, predicate: Callable[[Entity], bool]) -> List[Entity]:
        """
        Filter entities by predicate (SQL WHERE clause).

        Args:
            predicate: Function that returns True for entities to include

        Example:
            users.filter(lambda e: e.get_property('age') > 25)
        """
        return [e for e in self._entities.values() if predicate(e)]

    def create_index(self, property_name: str) -> None:
        """
        Create an index on a property for fast lookups.

        Args:
            property_name: Property to index

        Biological note: Like creating a specialized pheromone trail
        that guides directly to entities with specific property values.
        """
        if property_name in self._indexes:
            return  # Already indexed

        index = Index(property_name)
        self._indexes[property_name] = index

        # Populate index with existing entities
        for entity in self._entities.values():
            index.insert(entity)

    def drop_index(self, property_name: str) -> None:
        """Remove an index."""
        self._indexes.pop(property_name, None)

    def lookup(
        self,
        property_name: str,
        value: Any = None,
        min_value: Any = None,
        max_value: Any = None
    ) -> List[Entity]:
        """
        Fast indexed lookup.

        Args:
            property_name: Property to search
            value: Exact value to match (for equality search)
            min_value: Minimum value (for range search)
            max_value: Maximum value (for range search)

        Returns:
            List of matching entities

        Example:
            # Exact match: users.lookup('name', value='Alice')
            # Range: users.lookup('age', min_value=25, max_value=40)
        """
        # Create index if it doesn't exist (lazy indexing)
        if property_name not in self._indexes:
            self.create_index(property_name)

        index = self._indexes[property_name]

        # Exact match
        if value is not None:
            entity_ids = index.lookup_exact(value)
        # Range query
        else:
            entity_ids = index.lookup_range(min_value, max_value)

        return [self._entities[eid] for eid in entity_ids if eid in self._entities]

    def count(self) -> int:
        """Return number of entities in collection."""
        return len(self._entities)

    def _update_stats(self) -> None:
        """Update collection statistics for query optimization."""
        if not self._entities:
            self.stats = {'count': 0, 'avg_properties': 0, 'avg_degree': 0}
            return

        entities = list(self._entities.values())
        self.stats['count'] = len(entities)
        self.stats['avg_properties'] = sum(
            len(e.properties) for e in entities
        ) / len(entities)
        self.stats['avg_degree'] = sum(e.degree() for e in entities) / len(entities)

    def to_dict(self) -> Dict[str, Any]:
        """Serialize collection metadata."""
        return {
            'name': self.name,
            'schema': {k: v.__name__ for k, v in self.schema.items()},
            'entity_count': self.count(),
            'indexes': list(self._indexes.keys()),
            'stats': self.stats,
        }

    def __repr__(self) -> str:
        return f"Collection(name={self.name}, count={self.count()}, indexes={list(self._indexes.keys())})"
