"""
Deed: Distributed Emergent Evolution Database

A biologically-inspired hybrid database system that unifies relational
and graph data models through swarm intelligence principles.
"""

__version__ = "0.1.0"
__author__ = "Deed Project"

from deed.core.entity import Entity
from deed.core.edge import Edge
from deed.core.graph import DeedGraph
from deed.core.collection import Collection

__all__ = [
    'Entity',
    'Edge',
    'DeedGraph',
    'Collection',
]
