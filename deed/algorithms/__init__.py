"""
Biological algorithms for Deed database.

This module contains swarm intelligence and bio-inspired algorithms that
enable adaptive, self-optimizing database behavior.
"""

from deed.algorithms.stigmergy import StigmergyCache, PheromoneTrail
from deed.algorithms.ant_colony import AntColonyOptimizer, QueryAnt
from deed.algorithms.bee_quorum import BeeQuorumConsensus, ScoutBee
from deed.algorithms.physarum import PhysarumReconfiguration

__all__ = [
    'StigmergyCache',
    'PheromoneTrail',
    'AntColonyOptimizer',
    'QueryAnt',
    'BeeQuorumConsensus',
    'ScoutBee',
    'PhysarumReconfiguration',
]
