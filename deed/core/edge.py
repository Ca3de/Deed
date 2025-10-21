"""
Edge: Represents relationships between entities.

Edges in Deed are first-class citizens with properties and pheromone weights,
enabling both graph traversals and biological-inspired routing optimization.
"""

from typing import Dict, Any, Optional
from uuid import uuid4
from datetime import datetime
import math


class Edge:
    """
    Directed relationship between two entities.

    Features:
    - Source and target entities
    - Edge type (relationship label)
    - Properties (relationship metadata)
    - Pheromone weight (biological-inspired routing strength)

    The pheromone mechanism allows frequently-traveled edges to become
    "stronger" (higher weight), guiding query optimization like ant trails.
    """

    # Pheromone constants (inspired by ant colony optimization)
    INITIAL_PHEROMONE = 1.0
    MIN_PHEROMONE = 0.1
    MAX_PHEROMONE = 10.0
    EVAPORATION_RATE = 0.95  # 5% decay per period

    def __init__(
        self,
        source_id: str,
        target_id: str,
        edge_type: str,
        edge_id: Optional[str] = None,
        properties: Optional[Dict[str, Any]] = None,
        pheromone: Optional[float] = None
    ):
        """
        Initialize an Edge.

        Args:
            source_id: Source entity ID
            target_id: Target entity ID
            edge_type: Relationship type (e.g., "FOLLOWS", "PURCHASED", "foreign_key")
            edge_id: Unique identifier (auto-generated if not provided)
            properties: Edge properties (relationship metadata)
            pheromone: Initial pheromone strength (defaults to INITIAL_PHEROMONE)
        """
        self.id = edge_id or str(uuid4())
        self.source_id = source_id
        self.target_id = target_id
        self.type = edge_type
        self.properties = properties or {}

        # Biological-inspired routing
        self.pheromone = pheromone if pheromone is not None else self.INITIAL_PHEROMONE

        # Metadata
        self.created_at = datetime.now()
        self.updated_at = datetime.now()

        # Usage statistics
        self.traversal_count = 0
        self.last_traversed = None
        self.avg_traversal_cost = 0.0  # Milliseconds

    def set_property(self, key: str, value: Any) -> None:
        """Set or update an edge property."""
        self.properties[key] = value
        self.updated_at = datetime.now()

    def get_property(self, key: str, default: Any = None) -> Any:
        """Get an edge property value."""
        return self.properties.get(key, default)

    def reinforce_pheromone(self, amount: float = 1.0) -> None:
        """
        Increase pheromone strength (like ants depositing pheromone).

        Called when this edge is part of a successful query path.
        Higher reinforcement for lower-cost (faster) queries.

        Args:
            amount: Pheromone deposit amount (default 1.0)
        """
        self.pheromone = min(self.pheromone + amount, self.MAX_PHEROMONE)
        self.updated_at = datetime.now()

    def evaporate_pheromone(self) -> None:
        """
        Decrease pheromone strength over time.

        This natural decay prevents stale paths from dominating routing
        and allows the system to adapt to changing workload patterns.

        Called periodically by the Physarum network reconfiguration process.
        """
        self.pheromone = max(
            self.pheromone * self.EVAPORATION_RATE,
            self.MIN_PHEROMONE
        )

    def mark_traversed(self, cost_ms: float) -> None:
        """
        Record that this edge was traversed during query execution.

        Args:
            cost_ms: Query execution time in milliseconds
        """
        self.traversal_count += 1
        self.last_traversed = datetime.now()

        # Update exponential moving average of traversal cost
        alpha = 0.3  # Smoothing factor
        if self.avg_traversal_cost == 0:
            self.avg_traversal_cost = cost_ms
        else:
            self.avg_traversal_cost = (
                alpha * cost_ms + (1 - alpha) * self.avg_traversal_cost
            )

        # Reinforce pheromone inversely proportional to cost
        # Fast edges get stronger reinforcement
        if cost_ms > 0:
            reinforcement = 1.0 / math.log(cost_ms + 2)  # Avoid log(0)
            self.reinforce_pheromone(reinforcement)

    def get_weight(self) -> float:
        """
        Calculate edge weight for routing decisions.

        Combines:
        - Pheromone strength (biological feedback)
        - Historical traversal cost
        - Edge type preferences

        Returns:
            Lower weight = more desirable path (like distance in shortest path)
        """
        # Base weight is inverse of pheromone (high pheromone = low weight = preferred)
        base_weight = 1.0 / self.pheromone

        # Adjust by historical cost if available
        if self.avg_traversal_cost > 0:
            cost_factor = math.log(self.avg_traversal_cost + 1) / 10.0
            return base_weight * (1 + cost_factor)

        return base_weight

    def is_stale(self, staleness_threshold: float = 0.5) -> bool:
        """
        Check if this edge has very low pheromone (candidate for pruning).

        Args:
            staleness_threshold: Pheromone level below which edge is stale
        """
        return self.pheromone < staleness_threshold

    def to_dict(self) -> Dict[str, Any]:
        """Serialize edge to dictionary."""
        return {
            'id': self.id,
            'source_id': self.source_id,
            'target_id': self.target_id,
            'type': self.type,
            'properties': self.properties,
            'pheromone': self.pheromone,
            'traversal_count': self.traversal_count,
            'avg_cost_ms': self.avg_traversal_cost,
            'weight': self.get_weight(),
            'created_at': self.created_at.isoformat(),
        }

    def __repr__(self) -> str:
        return (
            f"Edge(id={self.id[:8]}, "
            f"{self.source_id[:8]}-[{self.type}]->{self.target_id[:8]}, "
            f"pheromone={self.pheromone:.2f})"
        )

    def __eq__(self, other) -> bool:
        if not isinstance(other, Edge):
            return False
        return self.id == other.id

    def __hash__(self) -> int:
        return hash(self.id)
