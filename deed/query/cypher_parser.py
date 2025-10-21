"""
Cypher Parser for Deed Database

Translates Cypher (Neo4j graph query language) into Deed's
unified query representation.

Supports basic Cypher:
- MATCH patterns
- WHERE filters
- RETURN projections
- CREATE nodes and relationships
"""

from typing import Dict, List, Any, Optional
import re


class CypherParser:
    """
    Simplified Cypher parser for graph queries.

    Cypher is Neo4j's graph query language. Examples:
    - MATCH (u:User)-[:FOLLOWS]->(f) RETURN f.name
    - MATCH (u:User {city: 'NYC'})-[:PURCHASED]->(p:Product) RETURN p
    """

    def parse(self, cypher: str) -> Dict[str, Any]:
        """
        Parse Cypher query into Deed's internal query representation.

        Args:
            cypher: Cypher query string

        Returns:
            Query dictionary for graph traversal

        Example:
            cypher = "MATCH (u:User)-[:FOLLOWS]->(f:User) WHERE u.name = 'Alice' RETURN f.name"
            query = parser.parse(cypher)
            # {
            #   'operation': 'match',
            #   'pattern': {...},
            #   'filters': {...},
            #   'return': ['f.name']
            # }
        """
        cypher = cypher.strip()
        cypher_upper = cypher.upper()

        if cypher_upper.startswith('MATCH'):
            return self._parse_match(cypher)
        elif cypher_upper.startswith('CREATE'):
            return self._parse_create(cypher)
        else:
            raise ValueError(f"Unsupported Cypher query: {cypher}")

    def _parse_match(self, cypher: str) -> Dict[str, Any]:
        """
        Parse MATCH query.

        Pattern syntax:
        - (n) - any node, variable n
        - (n:Label) - node with label, variable n
        - (n:Label {prop: value}) - node with label and properties
        - -[:TYPE]-> - directed relationship with type
        - -[:TYPE]- - undirected relationship
        - -[r:TYPE]-> - relationship with variable r
        """
        query = {
            'operation': 'match',
            'pattern': {},
            'filters': {},
            'return': [],
            'limit': None,
        }

        # Extract MATCH pattern
        match_pattern = re.search(
            r'MATCH\s+(.*?)(?:WHERE|RETURN|$)',
            cypher,
            re.IGNORECASE
        )

        if match_pattern:
            pattern_str = match_pattern.group(1).strip()
            query['pattern'] = self._parse_pattern(pattern_str)

        # Extract WHERE clause
        where_match = re.search(
            r'WHERE\s+(.*?)(?:RETURN|$)',
            cypher,
            re.IGNORECASE
        )
        if where_match:
            where_clause = where_match.group(1).strip()
            query['filters'] = self._parse_where_cypher(where_clause)

        # Extract RETURN clause
        return_match = re.search(
            r'RETURN\s+(.*?)(?:LIMIT|$)',
            cypher,
            re.IGNORECASE
        )
        if return_match:
            return_clause = return_match.group(1).strip()
            query['return'] = [r.strip() for r in return_clause.split(',')]

        # Extract LIMIT
        limit_match = re.search(r'LIMIT\s+(\d+)', cypher, re.IGNORECASE)
        if limit_match:
            query['limit'] = int(limit_match.group(1))

        return query

    def _parse_pattern(self, pattern: str) -> Dict[str, Any]:
        """
        Parse Cypher pattern into structured representation.

        Examples:
        - (u:User) -> {'nodes': [{'var': 'u', 'label': 'User'}], 'edges': []}
        - (u:User)-[:FOLLOWS]->(f:User) ->
            {
              'nodes': [
                {'var': 'u', 'label': 'User'},
                {'var': 'f', 'label': 'User'}
              ],
              'edges': [
                {'type': 'FOLLOWS', 'direction': 'out', 'from': 'u', 'to': 'f'}
              ]
            }
        """
        result = {
            'nodes': [],
            'edges': [],
        }

        # Extract nodes: (variable:Label {prop: value})
        node_pattern = r'\((\w+)(?::(\w+))?(?:\s*\{([^}]+)\})?\)'
        node_matches = re.finditer(node_pattern, pattern)

        for match in node_matches:
            node = {
                'var': match.group(1),
                'label': match.group(2),
                'properties': {},
            }

            # Parse inline properties
            if match.group(3):
                props_str = match.group(3)
                # Simple property parsing: key: value, key: value
                prop_pairs = props_str.split(',')
                for pair in prop_pairs:
                    if ':' in pair:
                        key, val = pair.split(':', 1)
                        node['properties'][key.strip()] = val.strip().strip("'\"")

            result['nodes'].append(node)

        # Extract edges: -[:TYPE]-> or -[var:TYPE]-> or <-[:TYPE]-
        edge_pattern = r'(<)?-\[(?:(\w+):)?(\w+)\]->(>)?'
        edge_matches = re.finditer(edge_pattern, pattern)

        # Find which nodes the edges connect
        # This is simplified - a real parser would track position
        nodes = result['nodes']

        for i, match in enumerate(edge_matches):
            is_incoming = match.group(1) == '<'
            edge_var = match.group(2)
            edge_type = match.group(3)

            edge = {
                'var': edge_var,
                'type': edge_type,
                'direction': 'in' if is_incoming else 'out',
            }

            # Connect to adjacent nodes (simplified)
            if i < len(nodes) - 1:
                edge['from'] = nodes[i]['var']
                edge['to'] = nodes[i + 1]['var']

            result['edges'].append(edge)

        return result

    def _parse_where_cypher(self, where_clause: str) -> Dict[str, Any]:
        """Parse Cypher WHERE clause."""
        filters = {}

        # Split by AND
        conditions = re.split(r'\s+AND\s+', where_clause, flags=re.IGNORECASE)

        for condition in conditions:
            condition = condition.strip()

            # Match: variable.property operator value
            match = re.match(r'(\w+)\.(\w+)\s*([><=!]+)\s*(.+)', condition)
            if match:
                var = match.group(1)
                prop = match.group(2)
                operator = match.group(3)
                value = match.group(4).strip().strip("'\"")

                # Try to convert to appropriate type
                try:
                    if value.isdigit():
                        value = int(value)
                    elif value.replace('.', '').isdigit():
                        value = float(value)
                except:
                    pass

                if var not in filters:
                    filters[var] = {}
                filters[var][prop] = (operator, value)

        return filters

    def _parse_create(self, cypher: str) -> Dict[str, Any]:
        """Parse CREATE query."""
        query = {
            'operation': 'create',
            'pattern': {},
        }

        # Extract CREATE pattern (same as MATCH pattern)
        create_pattern = re.search(
            r'CREATE\s+(.*?)$',
            cypher,
            re.IGNORECASE
        )

        if create_pattern:
            pattern_str = create_pattern.group(1).strip()
            query['pattern'] = self._parse_pattern(pattern_str)

        return query


# Example usage and tests
if __name__ == "__main__":
    parser = CypherParser()

    # Test simple MATCH
    cypher1 = "MATCH (u:User) WHERE u.age > 25 RETURN u.name, u.email"
    print("Cypher:", cypher1)
    print("Parsed:", parser.parse(cypher1))
    print()

    # Test MATCH with relationship
    cypher2 = "MATCH (u:User)-[:FOLLOWS]->(f:User) WHERE u.name = 'Alice' RETURN f.name"
    print("Cypher:", cypher2)
    print("Parsed:", parser.parse(cypher2))
    print()

    # Test MATCH with properties
    cypher3 = "MATCH (u:User {city: 'NYC'})-[:PURCHASED]->(p:Product) RETURN p.name, p.price"
    print("Cypher:", cypher3)
    print("Parsed:", parser.parse(cypher3))
    print()

    # Test multi-hop
    cypher4 = "MATCH (a:User)-[:FRIENDS]->(b:User)-[:FRIENDS]->(c:User) WHERE a.name = 'Alice' RETURN c.name LIMIT 10"
    print("Cypher:", cypher4)
    print("Parsed:", parser.parse(cypher4))
    print()

    # Test CREATE
    cypher5 = "CREATE (u:User {name: 'Bob', age: 30})-[:FOLLOWS]->(f:User {name: 'Alice'})"
    print("Cypher:", cypher5)
    print("Parsed:", parser.parse(cypher5))
