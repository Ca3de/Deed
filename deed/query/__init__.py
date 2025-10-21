"""
Query Processing Layer for Deed Database

Provides SQL and Cypher query language support on top of the
unified property graph model.

This module translates declarative queries (SQL/Cypher) into
execution plans that leverage biological algorithms for optimization.
"""

from deed.query.sql_parser import SQLParser
from deed.query.cypher_parser import CypherParser
from deed.query.executor import QueryExecutor
from deed.query.query_interface import DeedQueryInterface, query

__all__ = [
    'SQLParser',
    'CypherParser',
    'QueryExecutor',
    'DeedQueryInterface',
    'query',
]
