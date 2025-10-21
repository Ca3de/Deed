"""
Unified Query Interface for Deed Database

Provides a single entry point for both SQL and Cypher queries.
Automatically detects query language and routes to appropriate parser.
"""

from deed.core.graph import DeedGraph
from deed.query.sql_parser import SQLParser
from deed.query.cypher_parser import CypherParser
from deed.query.executor import QueryExecutor
from typing import List, Any, Dict


class DeedQueryInterface:
    """
    Main query interface for Deed database.

    Supports both SQL and Cypher through a unified execute() method.

    Example:
        db = DeedGraph("my_db")
        query_interface = DeedQueryInterface(db)

        # SQL
        results = query_interface.execute(
            "SELECT name, age FROM Users WHERE age > 25"
        )

        # Cypher
        results = query_interface.execute(
            "MATCH (u:User)-[:FOLLOWS]->(f) RETURN f.name"
        )
    """

    def __init__(self, database: DeedGraph, use_optimization: bool = True):
        """
        Initialize query interface.

        Args:
            database: Deed database instance
            use_optimization: Enable biological optimization (recommended)
        """
        self.database = database
        self.sql_parser = SQLParser()
        self.cypher_parser = CypherParser()
        self.executor = QueryExecutor(database, use_optimization=use_optimization)

    def execute(self, query_string: str) -> List[Any]:
        """
        Execute a query (auto-detects SQL vs Cypher).

        Args:
            query_string: SQL or Cypher query string

        Returns:
            Query results

        Examples:
            # SQL SELECT
            results = qi.execute("SELECT * FROM Users WHERE age > 25")

            # SQL with JOIN
            results = qi.execute('''
                SELECT u.name, o.total
                FROM Users u
                JOIN Orders o ON u.id = o.user_id
            ''')

            # Cypher MATCH
            results = qi.execute("MATCH (u:User)-[:FOLLOWS]->(f) RETURN f.name")

            # Cypher with WHERE
            results = qi.execute('''
                MATCH (u:User)-[:PURCHASED]->(p:Product)
                WHERE u.city = 'NYC' AND p.price > 100
                RETURN p.name, p.price
            ''')
        """
        query_string = query_string.strip()

        # Detect query language
        query_upper = query_string.upper()

        # Cypher keywords
        if query_upper.startswith(('MATCH', 'CREATE', 'MERGE', 'DELETE')):
            parsed_query = self.cypher_parser.parse(query_string)
        # SQL keywords
        else:
            parsed_query = self.sql_parser.parse(query_string)

        # Execute with biological optimization
        results = self.executor.execute(parsed_query)

        return results

    def execute_sql(self, sql: str) -> List[Any]:
        """Execute SQL query explicitly."""
        parsed = self.sql_parser.parse(sql)
        return self.executor.execute(parsed)

    def execute_cypher(self, cypher: str) -> List[Any]:
        """Execute Cypher query explicitly."""
        parsed = self.cypher_parser.parse(cypher)
        return self.executor.execute(parsed)

    def get_stats(self) -> Dict[str, Any]:
        """Get query execution statistics."""
        return self.executor.get_stats()

    def explain(self, query_string: str) -> Dict[str, Any]:
        """
        Explain query execution plan (like SQL EXPLAIN).

        Shows:
        - Parsed query structure
        - Optimized execution plan (from stigmergy cache or ant colony)
        - Estimated cost

        Args:
            query_string: Query to explain

        Returns:
            Explanation dictionary
        """
        query_upper = query_string.upper()

        # Parse query
        if query_upper.startswith(('MATCH', 'CREATE')):
            parsed_query = self.cypher_parser.parse(query_string)
        else:
            parsed_query = self.sql_parser.parse(query_string)

        # Get execution plan from stigmergy cache
        cached_plan = self.executor.stigmergy_cache.get_best_plan(parsed_query)

        explanation = {
            'original_query': query_string,
            'parsed_query': parsed_query,
            'cached_plan': cached_plan,
            'plan_source': 'stigmergy_cache' if cached_plan else 'will_use_ant_colony',
        }

        # If no cached plan, generate one with ant colony
        if not cached_plan and self.executor.use_optimization:
            graph_stats = {
                'avg_scan_cost': 100.0,
                'avg_lookup_cost': 10.0,
                'avg_traverse_cost': 50.0,
            }
            best_plan = self.executor.ant_optimizer.optimize(parsed_query, graph_stats)
            explanation['generated_plan'] = best_plan
            explanation['plan_source'] = 'ant_colony_optimizer'

        return explanation


# Convenience function for one-off queries
def query(database: DeedGraph, query_string: str) -> List[Any]:
    """
    Execute a one-off query without creating interface instance.

    Args:
        database: Deed database
        query_string: SQL or Cypher query

    Returns:
        Query results

    Example:
        from deed import DeedGraph
        from deed.query import query

        db = DeedGraph("my_db")
        results = query(db, "SELECT * FROM Users WHERE age > 25")
    """
    qi = DeedQueryInterface(database)
    return qi.execute(query_string)
