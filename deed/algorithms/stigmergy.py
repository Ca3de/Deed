"""
Stigmergy: Indirect coordination through environmental modification.

In nature, ants leave pheromone trails that guide other ants.
In Deed, successful queries leave "digital pheromones" that guide
future query optimization.

This is environment-mediated communication - the database learns
from its own execution history without centralized coordination.
"""

from typing import Dict, List, Optional, Any, Tuple
from dataclasses import dataclass, field
from datetime import datetime, timedelta
from collections import defaultdict
import hashlib
import json


@dataclass
class PheromoneTrail:
    """
    A digital pheromone trail marking a successful query path.

    Analogous to ant pheromone trails that mark good routes to food.
    """

    # Trail identification
    query_signature: str  # Hash of query pattern
    path_signature: str   # Hash of execution path

    # Trail strength
    pheromone: float = 1.0  # Current pheromone concentration
    initial_pheromone: float = 1.0

    # Performance metrics
    avg_execution_time_ms: float = 0.0
    success_count: int = 0
    failure_count: int = 0

    # Temporal metadata
    created_at: datetime = field(default_factory=datetime.now)
    last_reinforced: datetime = field(default_factory=datetime.now)
    last_used: datetime = field(default_factory=datetime.now)

    # Path details
    execution_plan: Dict[str, Any] = field(default_factory=dict)
    # Examples: join order, index choices, shard routing

    def reinforce(self, execution_time_ms: float, success: bool = True) -> None:
        """
        Strengthen this trail (called when path is successfully used).

        Args:
            execution_time_ms: Query execution time
            success: Whether query succeeded
        """
        if success:
            self.success_count += 1

            # Update exponential moving average of execution time
            alpha = 0.3  # Smoothing factor
            if self.avg_execution_time_ms == 0:
                self.avg_execution_time_ms = execution_time_ms
            else:
                self.avg_execution_time_ms = (
                    alpha * execution_time_ms +
                    (1 - alpha) * self.avg_execution_time_ms
                )

            # Reinforce pheromone inversely proportional to execution time
            # Faster queries get stronger reinforcement
            reinforcement = 1.0 / (1.0 + execution_time_ms / 100.0)
            self.pheromone = min(self.pheromone + reinforcement, 10.0)

        else:
            self.failure_count += 1
            # Failed queries weaken the trail
            self.pheromone = max(self.pheromone * 0.8, 0.1)

        self.last_reinforced = datetime.now()
        self.last_used = datetime.now()

    def evaporate(self, decay_rate: float = 0.05) -> None:
        """
        Natural pheromone decay over time.

        Args:
            decay_rate: Fraction to decay (0.05 = 5% loss)
        """
        self.pheromone = max(self.pheromone * (1 - decay_rate), 0.1)

    def is_stale(self, max_age_minutes: int = 60) -> bool:
        """
        Check if trail hasn't been used recently.

        Args:
            max_age_minutes: Age threshold for staleness
        """
        age = datetime.now() - self.last_used
        return age > timedelta(minutes=max_age_minutes)

    def quality_score(self) -> float:
        """
        Calculate overall trail quality.

        Combines:
        - Pheromone strength
        - Success rate
        - Performance (low execution time is good)
        """
        if self.success_count + self.failure_count == 0:
            return 0.0

        success_rate = self.success_count / (self.success_count + self.failure_count)

        # Normalize execution time (lower is better)
        time_score = 1.0 / (1.0 + self.avg_execution_time_ms / 100.0)

        return self.pheromone * success_rate * time_score

    def to_dict(self) -> Dict[str, Any]:
        """Serialize trail to dictionary."""
        return {
            'query_signature': self.query_signature,
            'path_signature': self.path_signature,
            'pheromone': self.pheromone,
            'avg_time_ms': self.avg_execution_time_ms,
            'success_count': self.success_count,
            'failure_count': self.failure_count,
            'quality_score': self.quality_score(),
            'execution_plan': self.execution_plan,
            'created_at': self.created_at.isoformat(),
            'last_used': self.last_used.isoformat(),
        }


class StigmergyCache:
    """
    Stigmergy-based query optimization cache.

    Maintains a "pheromone map" of successful query execution paths.
    Like a collective memory written in the environment, guiding
    future queries toward proven-effective strategies.

    Biological analogy: The network of ant trails around a nest -
    some trails strong (frequently used, effective), others fading
    (unused, ineffective).
    """

    def __init__(
        self,
        max_trails: int = 10000,
        evaporation_rate: float = 0.05,
        staleness_threshold_minutes: int = 60
    ):
        """
        Initialize stigmergy cache.

        Args:
            max_trails: Maximum number of trails to maintain
            evaporation_rate: Pheromone decay rate per evaporation cycle
            staleness_threshold_minutes: Age after which trails are stale
        """
        self.max_trails = max_trails
        self.evaporation_rate = evaporation_rate
        self.staleness_threshold_minutes = staleness_threshold_minutes

        # Map: query_signature -> list of PheromoneTrails
        self._trails: Dict[str, List[PheromoneTrail]] = defaultdict(list)

        # Statistics
        self.stats = {
            'total_trails': 0,
            'total_reinforcements': 0,
            'cache_hits': 0,
            'cache_misses': 0,
        }

    def get_query_signature(self, query: Dict[str, Any]) -> str:
        """
        Create a signature hash for a query pattern.

        Args:
            query: Query representation (structure, not specific values)

        Returns:
            Hash string identifying this query pattern

        Example:
            SELECT * FROM Users WHERE age > ?
            -> "users_age_gt_scan"
        """
        # Normalize query to remove specific values, keep structure
        normalized = {
            'operation': query.get('operation'),
            'collection': query.get('collection'),
            'filters': sorted(query.get('filters', {}).keys()),
            'joins': query.get('joins', []),
            'traversals': query.get('traversals', []),
        }

        query_str = json.dumps(normalized, sort_keys=True)
        return hashlib.sha256(query_str.encode()).hexdigest()[:16]

    def get_path_signature(self, execution_plan: Dict[str, Any]) -> str:
        """
        Create a signature hash for an execution plan.

        Args:
            execution_plan: How the query was executed

        Returns:
            Hash string identifying this execution path
        """
        plan_str = json.dumps(execution_plan, sort_keys=True)
        return hashlib.sha256(plan_str.encode()).hexdigest()[:16]

    def lookup(self, query: Dict[str, Any]) -> List[PheromoneTrail]:
        """
        Find pheromone trails for a query pattern.

        Args:
            query: Query to find trails for

        Returns:
            List of trails, sorted by quality (best first)
        """
        query_sig = self.get_query_signature(query)

        if query_sig not in self._trails:
            self.stats['cache_misses'] += 1
            return []

        trails = self._trails[query_sig]

        # Filter out stale trails
        trails = [
            t for t in trails
            if not t.is_stale(self.staleness_threshold_minutes)
        ]

        # Sort by quality
        trails.sort(key=lambda t: t.quality_score(), reverse=True)

        self.stats['cache_hits'] += 1
        return trails

    def add_trail(
        self,
        query: Dict[str, Any],
        execution_plan: Dict[str, Any],
        execution_time_ms: float,
        success: bool = True
    ) -> PheromoneTrail:
        """
        Add or reinforce a pheromone trail.

        Args:
            query: Query that was executed
            execution_plan: How it was executed
            execution_time_ms: Execution time
            success: Whether query succeeded

        Returns:
            The trail (new or existing)
        """
        query_sig = self.get_query_signature(query)
        path_sig = self.get_path_signature(execution_plan)

        # Check if this exact path already exists
        existing_trails = self._trails[query_sig]
        for trail in existing_trails:
            if trail.path_signature == path_sig:
                # Reinforce existing trail
                trail.reinforce(execution_time_ms, success)
                self.stats['total_reinforcements'] += 1
                return trail

        # Create new trail
        trail = PheromoneTrail(
            query_signature=query_sig,
            path_signature=path_sig,
            execution_plan=execution_plan,
            avg_execution_time_ms=execution_time_ms
        )
        trail.reinforce(execution_time_ms, success)

        self._trails[query_sig].append(trail)
        self.stats['total_trails'] += 1

        # Enforce max trails limit
        self._enforce_capacity()

        return trail

    def evaporate_all(self) -> None:
        """
        Apply pheromone evaporation to all trails.

        This should be called periodically (e.g., every minute)
        to let unused trails fade away naturally.
        """
        for trails in self._trails.values():
            for trail in trails:
                trail.evaporate(self.evaporation_rate)

        # Remove trails with very low pheromone
        self._prune_weak_trails()

    def _prune_weak_trails(self, min_pheromone: float = 0.2) -> None:
        """Remove trails with pheromone below threshold."""
        for query_sig in list(self._trails.keys()):
            self._trails[query_sig] = [
                t for t in self._trails[query_sig]
                if t.pheromone >= min_pheromone
            ]

            if not self._trails[query_sig]:
                del self._trails[query_sig]

        self.stats['total_trails'] = sum(len(t) for t in self._trails.values())

    def _enforce_capacity(self) -> None:
        """Remove lowest-quality trails if over capacity."""
        total_trails = sum(len(trails) for trails in self._trails.values())

        if total_trails <= self.max_trails:
            return

        # Collect all trails with their quality scores
        all_trails: List[Tuple[str, PheromoneTrail]] = []
        for query_sig, trails in self._trails.items():
            for trail in trails:
                all_trails.append((query_sig, trail))

        # Sort by quality (worst first)
        all_trails.sort(key=lambda x: x[1].quality_score())

        # Remove worst trails until under capacity
        to_remove = total_trails - self.max_trails
        for i in range(to_remove):
            query_sig, trail = all_trails[i]
            self._trails[query_sig].remove(trail)

            if not self._trails[query_sig]:
                del self._trails[query_sig]

        self.stats['total_trails'] = self.max_trails

    def get_best_plan(self, query: Dict[str, Any]) -> Optional[Dict[str, Any]]:
        """
        Get the best known execution plan for a query.

        This is the core of stigmergy-guided optimization:
        "What path did successful queries take before?"

        Args:
            query: Query to optimize

        Returns:
            Best execution plan, or None if no trails exist
        """
        trails = self.lookup(query)

        if not trails:
            return None

        # Return execution plan from strongest trail
        return trails[0].execution_plan

    def get_stats(self) -> Dict[str, Any]:
        """Get cache statistics."""
        return {
            **self.stats,
            'hit_rate': (
                self.stats['cache_hits'] /
                (self.stats['cache_hits'] + self.stats['cache_misses'])
                if (self.stats['cache_hits'] + self.stats['cache_misses']) > 0
                else 0
            ),
        }

    def to_dict(self) -> Dict[str, Any]:
        """Serialize cache state."""
        all_trails = []
        for trails in self._trails.values():
            all_trails.extend([t.to_dict() for t in trails])

        return {
            'config': {
                'max_trails': self.max_trails,
                'evaporation_rate': self.evaporation_rate,
                'staleness_threshold_minutes': self.staleness_threshold_minutes,
            },
            'stats': self.get_stats(),
            'top_trails': sorted(
                all_trails,
                key=lambda t: t['quality_score'],
                reverse=True
            )[:20],  # Top 20 trails
        }

    def __repr__(self) -> str:
        return (
            f"StigmergyCache("
            f"trails={self.stats['total_trails']}, "
            f"hit_rate={self.get_stats()['hit_rate']:.2%})"
        )
