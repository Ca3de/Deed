"""
SQL Parser for Deed Database

Translates SQL queries into Deed's unified query representation,
which can then be optimized by biological algorithms.

Supports a subset of SQL:
- SELECT with WHERE, JOIN, ORDER BY, LIMIT
- INSERT, UPDATE, DELETE
- CREATE TABLE, CREATE INDEX
"""

from typing import Dict, List, Any, Optional
import re


class SQLParser:
    """
    Simplified SQL parser for Deed database.

    Note: This is a proof-of-concept parser. A production implementation
    would use a proper parser generator (e.g., ANTLR, PLY, or pyparsing).
    """

    def __init__(self):
        self.keywords = {
            'SELECT', 'FROM', 'WHERE', 'JOIN', 'ON', 'ORDER', 'BY',
            'LIMIT', 'INSERT', 'INTO', 'VALUES', 'UPDATE', 'SET',
            'DELETE', 'CREATE', 'TABLE', 'INDEX', 'AND', 'OR'
        }

    def parse(self, sql: str) -> Dict[str, Any]:
        """
        Parse SQL query into Deed's internal query representation.

        Args:
            sql: SQL query string

        Returns:
            Query dictionary that can be optimized by ant colony algorithm

        Example:
            sql = "SELECT name, age FROM Users WHERE age > 25 AND city = 'NYC'"
            query = parser.parse(sql)
            # {
            #   'operation': 'select',
            #   'collection': 'Users',
            #   'columns': ['name', 'age'],
            #   'filters': {'age': ('>', 25), 'city': ('=', 'NYC')},
            #   'joins': [],
            # }
        """
        sql = sql.strip()
        sql_upper = sql.upper()

        # Determine query type
        if sql_upper.startswith('SELECT'):
            return self._parse_select(sql)
        elif sql_upper.startswith('INSERT'):
            return self._parse_insert(sql)
        elif sql_upper.startswith('UPDATE'):
            return self._parse_update(sql)
        elif sql_upper.startswith('DELETE'):
            return self._parse_delete(sql)
        elif sql_upper.startswith('CREATE TABLE'):
            return self._parse_create_table(sql)
        elif sql_upper.startswith('CREATE INDEX'):
            return self._parse_create_index(sql)
        else:
            raise ValueError(f"Unsupported SQL query: {sql}")

    def _parse_select(self, sql: str) -> Dict[str, Any]:
        """Parse SELECT query."""
        query = {
            'operation': 'select',
            'columns': [],
            'collection': None,
            'filters': {},
            'joins': [],
            'order_by': [],
            'limit': None,
        }

        # Extract columns
        select_match = re.search(r'SELECT\s+(.*?)\s+FROM', sql, re.IGNORECASE)
        if select_match:
            columns_str = select_match.group(1).strip()
            if columns_str == '*':
                query['columns'] = ['*']
            else:
                query['columns'] = [c.strip() for c in columns_str.split(',')]

        # Extract table/collection
        from_match = re.search(r'FROM\s+(\w+)', sql, re.IGNORECASE)
        if from_match:
            query['collection'] = from_match.group(1)

        # Extract WHERE clause
        where_match = re.search(r'WHERE\s+(.*?)(?:ORDER|LIMIT|$)', sql, re.IGNORECASE)
        if where_match:
            where_clause = where_match.group(1).strip()
            query['filters'] = self._parse_where(where_clause)

        # Extract JOINs
        join_matches = re.finditer(
            r'JOIN\s+(\w+)\s+ON\s+([\w.]+)\s*=\s*([\w.]+)',
            sql,
            re.IGNORECASE
        )
        for match in join_matches:
            query['joins'].append({
                'table': match.group(1),
                'left': match.group(2),
                'right': match.group(3),
            })

        # Extract ORDER BY
        order_match = re.search(r'ORDER BY\s+(.*?)(?:LIMIT|$)', sql, re.IGNORECASE)
        if order_match:
            order_str = order_match.group(1).strip()
            query['order_by'] = [o.strip() for o in order_str.split(',')]

        # Extract LIMIT
        limit_match = re.search(r'LIMIT\s+(\d+)', sql, re.IGNORECASE)
        if limit_match:
            query['limit'] = int(limit_match.group(1))

        return query

    def _parse_where(self, where_clause: str) -> Dict[str, Any]:
        """Parse WHERE clause into filter conditions."""
        filters = {}

        # Split by AND (simplified - doesn't handle OR or parentheses)
        conditions = re.split(r'\s+AND\s+', where_clause, flags=re.IGNORECASE)

        for condition in conditions:
            condition = condition.strip()

            # Match: column operator value
            match = re.match(r'(\w+)\s*([><=!]+)\s*(.+)', condition)
            if match:
                column = match.group(1)
                operator = match.group(2)
                value = match.group(3).strip().strip("'\"")

                # Try to convert to appropriate type
                try:
                    if value.isdigit():
                        value = int(value)
                    elif value.replace('.', '').isdigit():
                        value = float(value)
                except:
                    pass

                filters[column] = (operator, value)

        return filters

    def _parse_insert(self, sql: str) -> Dict[str, Any]:
        """Parse INSERT query."""
        query = {
            'operation': 'insert',
            'collection': None,
            'values': {},
        }

        # INSERT INTO table (col1, col2) VALUES (val1, val2)
        match = re.search(
            r'INSERT INTO\s+(\w+)\s*\((.*?)\)\s*VALUES\s*\((.*?)\)',
            sql,
            re.IGNORECASE
        )

        if match:
            query['collection'] = match.group(1)
            columns = [c.strip() for c in match.group(2).split(',')]
            values_str = [v.strip().strip("'\"") for v in match.group(3).split(',')]

            query['values'] = dict(zip(columns, values_str))

        return query

    def _parse_update(self, sql: str) -> Dict[str, Any]:
        """Parse UPDATE query."""
        query = {
            'operation': 'update',
            'collection': None,
            'updates': {},
            'filters': {},
        }

        # Extract table
        update_match = re.search(r'UPDATE\s+(\w+)', sql, re.IGNORECASE)
        if update_match:
            query['collection'] = update_match.group(1)

        # Extract SET clause
        set_match = re.search(r'SET\s+(.*?)(?:WHERE|$)', sql, re.IGNORECASE)
        if set_match:
            set_clause = set_match.group(1).strip()
            assignments = set_clause.split(',')
            for assignment in assignments:
                if '=' in assignment:
                    col, val = assignment.split('=', 1)
                    query['updates'][col.strip()] = val.strip().strip("'\"")

        # Extract WHERE clause
        where_match = re.search(r'WHERE\s+(.*?)$', sql, re.IGNORECASE)
        if where_match:
            query['filters'] = self._parse_where(where_match.group(1))

        return query

    def _parse_delete(self, sql: str) -> Dict[str, Any]:
        """Parse DELETE query."""
        query = {
            'operation': 'delete',
            'collection': None,
            'filters': {},
        }

        # Extract table
        delete_match = re.search(r'DELETE FROM\s+(\w+)', sql, re.IGNORECASE)
        if delete_match:
            query['collection'] = delete_match.group(1)

        # Extract WHERE clause
        where_match = re.search(r'WHERE\s+(.*?)$', sql, re.IGNORECASE)
        if where_match:
            query['filters'] = self._parse_where(where_match.group(1))

        return query

    def _parse_create_table(self, sql: str) -> Dict[str, Any]:
        """Parse CREATE TABLE query."""
        query = {
            'operation': 'create_table',
            'collection': None,
            'schema': {},
        }

        # CREATE TABLE table_name (col1 type1, col2 type2, ...)
        match = re.search(
            r'CREATE TABLE\s+(\w+)\s*\((.*?)\)',
            sql,
            re.IGNORECASE
        )

        if match:
            query['collection'] = match.group(1)
            columns_str = match.group(2)

            # Parse column definitions
            for col_def in columns_str.split(','):
                parts = col_def.strip().split()
                if len(parts) >= 2:
                    col_name = parts[0]
                    col_type = parts[1].upper()

                    # Map SQL types to Python types
                    type_map = {
                        'INTEGER': int,
                        'INT': int,
                        'FLOAT': float,
                        'REAL': float,
                        'TEXT': str,
                        'VARCHAR': str,
                        'BOOLEAN': bool,
                    }

                    query['schema'][col_name] = type_map.get(col_type, str)

        return query

    def _parse_create_index(self, sql: str) -> Dict[str, Any]:
        """Parse CREATE INDEX query."""
        query = {
            'operation': 'create_index',
            'collection': None,
            'column': None,
        }

        # CREATE INDEX idx_name ON table_name (column)
        match = re.search(
            r'CREATE INDEX\s+\w+\s+ON\s+(\w+)\s*\((\w+)\)',
            sql,
            re.IGNORECASE
        )

        if match:
            query['collection'] = match.group(1)
            query['column'] = match.group(2)

        return query


# Example usage and tests
if __name__ == "__main__":
    parser = SQLParser()

    # Test SELECT
    sql1 = "SELECT name, age FROM Users WHERE age > 25 AND city = 'NYC' LIMIT 10"
    print("SQL:", sql1)
    print("Parsed:", parser.parse(sql1))
    print()

    # Test SELECT with JOIN
    sql2 = """
        SELECT u.name, o.total
        FROM Users u
        JOIN Orders o ON u.id = o.user_id
        WHERE u.age > 30
    """
    print("SQL:", sql2)
    print("Parsed:", parser.parse(sql2))
    print()

    # Test INSERT
    sql3 = "INSERT INTO Users (name, age, city) VALUES ('Alice', 28, 'NYC')"
    print("SQL:", sql3)
    print("Parsed:", parser.parse(sql3))
    print()

    # Test UPDATE
    sql4 = "UPDATE Users SET city = 'SF' WHERE name = 'Bob'"
    print("SQL:", sql4)
    print("Parsed:", parser.parse(sql4))
    print()

    # Test CREATE TABLE
    sql5 = "CREATE TABLE Products (id INTEGER, name VARCHAR, price FLOAT)"
    print("SQL:", sql5)
    print("Parsed:", parser.parse(sql5))
