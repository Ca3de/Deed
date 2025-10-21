"""
Core data structures for Deed database.

This module implements the unified property graph model that serves
as the foundation for both relational and graph data representations.
"""

from deed.core.entity import Entity
from deed.core.edge import Edge
from deed.core.graph import DeedGraph
from deed.core.collection import Collection

__all__ = ['Entity', 'Edge', 'DeedGraph', 'Collection']
