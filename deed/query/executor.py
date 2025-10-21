"""
Query Executor for Deed Database

Executes parsed queries (SQL or Cypher) using biological optimization
algorithms to find the best execution plan.

This ties together:
- Parsed query representation
- Ant colony query plan optimization
- Stigmergy cache for learning
- Actual execution on the graph
"""

from typing import Dict, List, Any, Optional
from deed.core.graph import DeedGraph
from deed.algorithms import AntColonyOptimizer, StigmergyCache
import time


class QueryExecutor:
    """
    Execute queries with biological optimization.

    Process:
    1. Receive parsed query (from SQL or Cypher parser)
    2. Check stigmergy cache for known good plans
    3. If cache miss, use ant colony to find optimal plan
    4. Execute plan on the graph
    5. Record execution time and update cache
    """

    def __init__(
        self,
        database: DeedGraph,
        use_optimization: bool = True,
        num_ants: int = 20,
        num_iterations: int = 3
    ):
        """
        Initialize query executor.

        Args:
            database: Deed database instance
            use_optimization: Whether to use biological optimization
            num_ants: Number of ants for ACO
            num_iterations: Iterations for ACO
        """
        self.database = database
        self.use_optimization = use_optimization

        # Biological optimization components
        self.stigmergy_cache = StigmergyCache()
        self.ant_optimizer = AntColonyOptimizer(
            num_ants=num_ants,
            num_iterations=num_iterations,
            pheromone_cache=self.stigmergy_cache
        )

        # Execution statistics
        self.stats = {
            'total_queries': 0,
            'cache_hits': 0,
            'cache_misses': 0,
            'total_execution_time_ms': 0.0,
        }

    def execute(self, query: Dict[str, Any]) -> List[Any]:
        """
        Execute a parsed query.

        Args:
            query: Parsed query dictionary (from SQL or Cypher parser)

        Returns:
            Query results
        """
        start_time = time.time()

        operation = query.get('operation')

        if operation == 'select' or operation == 'match':
            results = self._execute_select(query)
        elif operation == 'insert' or operation == 'create':
            results = self._execute_insert(query)
        elif operation == 'update':
            results = self._execute_update(query)
        elif operation == 'delete':
            results = self._execute_delete(query)
        elif operation == 'create_table':
            results = self._execute_create_table(query)
        elif operation == 'create_index':
            results = self._execute_create_index(query)
        else:
            raise ValueError(f"Unsupported operation: {operation}")

        # Record execution time
        execution_time_ms = (time.time() - start_time) * 1000

        # Update statistics
        self.stats['total_queries'] += 1
        self.stats['total_execution_time_ms'] += execution_time_ms

        return results

    def _execute_select(self, query: Dict[str, Any]) -> List[Any]:
        """
        Execute SELECT (SQL) or MATCH (Cypher) query.

        This is where biological optimization happens!
        """
        collection_name = query.get('collection')

        if not collection_name:
            # Cypher pattern-based query
            return self._execute_graph_pattern(query)

        collection = self.database.get_collection(collection_name)
        if not collection:
            raise ValueError(f"Collection not found: {collection_name}")

        # Start execution timing
        start_time = time.time()

        # Step 1: Check stigmergy cache for optimal plan
        execution_plan = None
        if self.use_optimization:
            execution_plan = self.stigmergy_cache.get_best_plan(query)
            if execution_plan:
                self.stats['cache_hits'] += 1
            else:
                self.stats['cache_misses'] += 1
                # Step 2: Use ant colony to find optimal plan
                graph_stats = {
                    'avg_scan_cost': 100.0,
                    'avg_lookup_cost': 10.0,
                    'avg_traverse_cost': 50.0,
                }
                execution_plan = self.ant_optimizer.optimize(query, graph_stats)

        # Step 3: Execute query based on plan (or simple execution if no optimization)
        results = []

        # Apply filters
        filters = query.get('filters', {})
        if filters and self.use_optimization and execution_plan:
            # Use optimized filter order
            filter_order = execution_plan.get('filter_order', filters.keys())
        else:
            filter_order = filters.keys()

        # Get entities (using indexes if available)
        if filters:
            # Try to use index for first filter
            first_filter = list(filter_order)[0] if filter_order else None
            if first_filter and first_filter in filters:
                operator, value = filters[first_filter]
                if operator == '=':
                    entities = collection.lookup(first_filter, value=value)
                elif operator == '>':
                    entities = collection.lookup(first_filter, min_value=value)
                elif operator == '<':
                    entities = collection.lookup(first_filter, max_value=value)
                else:
                    entities = collection.scan()
            else:
                entities = collection.scan()
        else:
            entities = collection.scan()

        # Apply remaining filters
        for entity in entities:
            match = True
            for column, (operator, value) in filters.items():
                entity_value = entity.get_property(column)
                if entity_value is None:
                    match = False
                    break

                # Try to coerce types for comparison
                try:
                    if type(entity_value) != type(value):
                        # Try to convert entity_value to match value's type
                        if isinstance(value, int):
                            entity_value = int(entity_value)
                        elif isinstance(value, float):
                            entity_value = float(entity_value)
                        elif isinstance(value, str):
                            entity_value = str(entity_value)

                    if operator == '=':
                        if entity_value != value:
                            match = False
                    elif operator == '>':
                        if not (entity_value > value):
                            match = False
                    elif operator == '<':
                        if not (entity_value < value):
                            match = False
                    elif operator == '>=':
                        if not (entity_value >= value):
                            match = False
                    elif operator == '<=':
                        if not (entity_value <= value):
                            match = False
                    elif operator == '!=':
                        if entity_value == value:
                            match = False
                except (ValueError, TypeError):
                    # Type conversion failed or comparison not supported
                    match = False
                    break

            if match:
                results.append(entity)

        # Apply LIMIT
        limit = query.get('limit')
        if limit:
            results = results[:limit]

        # Project columns
        columns = query.get('columns', ['*'])
        if columns and columns != ['*']:
            projected_results = []
            for entity in results:
                row = {col: entity.get_property(col) for col in columns}
                projected_results.append(row)
            results = projected_results

        # Record execution time and update cache
        execution_time_ms = (time.time() - start_time) * 1000
        if self.use_optimization and execution_plan:
            self.stigmergy_cache.add_trail(
                query=query,
                execution_plan=execution_plan,
                execution_time_ms=execution_time_ms,
                success=True
            )

        return results

    def _execute_graph_pattern(self, query: Dict[str, Any]) -> List[Any]:
        """Execute Cypher graph pattern matching."""
        pattern = query.get('pattern', {})
        nodes = pattern.get('nodes', [])
        edges = pattern.get('edges', [])

        if not nodes:
            return []

        # Start with first node
        start_node_spec = nodes[0]
        start_label = start_node_spec.get('label')
        start_var = start_node_spec.get('var')

        # Get starting entities
        if start_label:
            collection = self.database.get_collection(start_label)
            if not collection:
                return []
            start_entities = collection.scan()
        else:
            # No label specified - scan all entities
            start_entities = list(self.database._entities.values())

        # Apply property filters from pattern
        start_props = start_node_spec.get('properties', {})
        if start_props:
            start_entities = [
                e for e in start_entities
                if all(e.get_property(k) == v for k, v in start_props.items())
            ]

        # Apply WHERE filters
        filters = query.get('filters', {})
        if start_var in filters:
            var_filters = filters[start_var]
            start_entities = [
                e for e in start_entities
                if all(
                    self._check_filter(e.get_property(prop), op, val)
                    for prop, (op, val) in var_filters.items()
                )
            ]

        # If no edges, return start entities
        if not edges:
            return self._project_results(start_entities, query.get('return', []))

        # Traverse edges
        results = []
        for start_entity in start_entities:
            # Build result mapping {var: entity}
            result_map = {start_var: start_entity}

            # Traverse each edge in pattern
            current_entity = start_entity
            for i, edge_spec in enumerate(edges):
                edge_type = edge_spec.get('type')
                direction = edge_spec.get('direction', 'out')
                to_var = edge_spec.get('to')

                # Find connected entities
                traversed = self.database.traverse(
                    start_id=current_entity.id,
                    edge_type=edge_type,
                    direction=direction,
                    max_depth=1
                )

                # Apply filters on target node
                if i + 1 < len(nodes):
                    target_node_spec = nodes[i + 1]
                    target_label = target_node_spec.get('label')
                    target_props = target_node_spec.get('properties', {})

                    if target_label:
                        traversed = [e for e in traversed if e.type == target_label]

                    if target_props:
                        traversed = [
                            e for e in traversed
                            if all(e.get_property(k) == v for k, v in target_props.items())
                        ]

                    # Apply WHERE filters
                    if to_var in filters:
                        var_filters = filters[to_var]
                        traversed = [
                            e for e in traversed
                            if all(
                                self._check_filter(e.get_property(prop), op, val)
                                for prop, (op, val) in var_filters.items()
                            )
                        ]

                # Add to result map
                if traversed and to_var:
                    result_map[to_var] = traversed[0]
                    current_entity = traversed[0]
                else:
                    # No match found - skip this path
                    break

            # If we successfully traversed all edges, add result
            if len(result_map) == len(nodes):
                results.append(result_map)

        # Apply LIMIT
        limit = query.get('limit')
        if limit:
            results = results[:limit]

        # Project return clause
        return_clause = query.get('return', [])
        if return_clause:
            return self._project_cypher_results(results, return_clause)

        return results

    def _check_filter(self, value: Any, operator: str, filter_value: Any) -> bool:
        """Check if value matches filter condition."""
        if value is None:
            return False

        if operator == '=':
            return value == filter_value
        elif operator == '>':
            return value > filter_value
        elif operator == '<':
            return value < filter_value
        elif operator == '>=':
            return value >= filter_value
        elif operator == '<=':
            return value <= filter_value
        elif operator == '!=':
            return value != filter_value
        return False

    def _project_cypher_results(
        self,
        results: List[Dict[str, Any]],
        return_clause: List[str]
    ) -> List[Dict[str, Any]]:
        """Project Cypher RETURN clause."""
        projected = []

        for result_map in results:
            row = {}
            for return_expr in return_clause:
                # Parse var.property
                if '.' in return_expr:
                    var, prop = return_expr.split('.', 1)
                    if var in result_map:
                        entity = result_map[var]
                        row[return_expr] = entity.get_property(prop)
                else:
                    # Return entire entity
                    if return_expr in result_map:
                        row[return_expr] = result_map[return_expr]

            projected.append(row)

        return projected

    def _project_results(self, entities: List[Any], return_clause: List[str]) -> List[Any]:
        """Project simple entity results."""
        if not return_clause:
            return entities

        projected = []
        for entity in entities:
            if return_clause == ['*']:
                projected.append(entity)
            else:
                row = {}
                for expr in return_clause:
                    if '.' in expr:
                        _, prop = expr.split('.', 1)
                        row[expr] = entity.get_property(prop)
                projected.append(row)

        return projected

    def _execute_insert(self, query: Dict[str, Any]) -> List[Any]:
        """Execute INSERT or CREATE query."""
        collection_name = query.get('collection')
        values = query.get('values', {})

        # SQL INSERT
        if collection_name and values:
            entity = self.database.add_entity(
                collection_name=collection_name,
                properties=values
            )
            return [entity]

        # Cypher CREATE
        pattern = query.get('pattern', {})
        if pattern:
            nodes = pattern.get('nodes', [])
            edges = pattern.get('edges', [])

            created_entities = {}

            # Create nodes
            for node_spec in nodes:
                label = node_spec.get('label', 'Unknown')
                props = node_spec.get('properties', {})
                var = node_spec.get('var')

                entity = self.database.add_entity(
                    collection_name=label,
                    properties=props
                )

                if var:
                    created_entities[var] = entity

            # Create edges
            for edge_spec in edges:
                from_var = edge_spec.get('from')
                to_var = edge_spec.get('to')
                edge_type = edge_spec.get('type')

                if from_var in created_entities and to_var in created_entities:
                    self.database.add_edge(
                        source_id=created_entities[from_var].id,
                        target_id=created_entities[to_var].id,
                        edge_type=edge_type
                    )

            return list(created_entities.values())

        return []

    def _execute_update(self, query: Dict[str, Any]) -> List[Any]:
        """Execute UPDATE query."""
        # First find entities matching filters
        select_query = {
            'operation': 'select',
            'collection': query.get('collection'),
            'filters': query.get('filters', {}),
            'columns': ['*'],
        }

        entities = self._execute_select(select_query)

        # Update properties
        updates = query.get('updates', {})
        for entity in entities:
            for key, value in updates.items():
                entity.set_property(key, value)

        return entities

    def _execute_delete(self, query: Dict[str, Any]) -> List[Any]:
        """Execute DELETE query."""
        # First find entities matching filters
        select_query = {
            'operation': 'select',
            'collection': query.get('collection'),
            'filters': query.get('filters', {}),
            'columns': ['*'],
        }

        entities = self._execute_select(select_query)

        # Delete each entity
        for entity in entities:
            self.database.remove_entity(entity.id)

        return entities

    def _execute_create_table(self, query: Dict[str, Any]) -> List[Any]:
        """Execute CREATE TABLE query."""
        collection_name = query.get('collection')
        schema = query.get('schema', {})

        collection = self.database.create_collection(collection_name, schema)
        return [collection]

    def _execute_create_index(self, query: Dict[str, Any]) -> List[Any]:
        """Execute CREATE INDEX query."""
        collection_name = query.get('collection')
        column = query.get('column')

        collection = self.database.get_collection(collection_name)
        if collection and column:
            collection.create_index(column)
            return [{'collection': collection_name, 'column': column}]

        return []

    def get_stats(self) -> Dict[str, Any]:
        """Get executor statistics."""
        avg_time = (
            self.stats['total_execution_time_ms'] / self.stats['total_queries']
            if self.stats['total_queries'] > 0
            else 0
        )

        return {
            **self.stats,
            'avg_execution_time_ms': avg_time,
            'optimizer_stats': self.ant_optimizer.get_stats(),
            'cache_stats': self.stigmergy_cache.get_stats(),
        }
