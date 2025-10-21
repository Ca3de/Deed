# DQL: Deed Query Language - Truly Unified

## The Problem with SQL + GQL

Using two separate query languages (SQL for tables, GQL for graphs) is just **syntactic sugar over a wrapper**. It doesn't provide real unification.

**Example of the problem**:
```sql
-- Hybrid query attempt with SQL + GQL
-- Step 1: SQL
SELECT user_id FROM Users WHERE city = 'NYC';

-- Step 2: GQL (separate query!)
MATCH (u:User)-[:PURCHASED]->(p:Product)
WHERE u.user_id IN [previous_results]
RETURN p;
```

This requires **two round trips**, context switching between languages, and manual result passing. Not truly unified.

---

## DQL: Single Language for Both Paradigms

### Core Principle

**Everything is a graph, but we provide relational syntax sugar when convenient.**

In DQL:
- Tables are just **typed node collections** with homogeneous properties
- Rows are nodes
- Foreign keys are edges
- Indexes are edge shortcuts
- Joins are graph traversals with specific edge types

### Syntax Design

```dql
-- Relational-style (familiar SQL syntax)
FROM Users
WHERE age > 25 AND city = 'NYC'
SELECT name, age;

-- Graph-style (natural traversal syntax)
FROM Users WHERE name = 'Alice'
TRAVERSE -[:FOLLOWS]-> AS follower
SELECT follower.name;

-- TRUE HYBRID (single query!)
FROM Users WHERE city = 'NYC'
TRAVERSE -[:PURCHASED]-> Product
WHERE Product.price > 100
SELECT User.name, Product.name, Product.price;
```

**Key insight**: The `FROM` clause always starts with entities. `TRAVERSE` naturally extends to relationships. Single unified execution plan.

---

## DQL Complete Syntax

### 1. Basic Entity Selection (SQL-like)

```dql
-- Simple scan
FROM Users;

-- With filter
FROM Users WHERE age > 25;

-- With projection
FROM Users WHERE city = 'NYC' SELECT name, email;

-- With aggregation
FROM Orders
GROUP BY user_id
SELECT user_id, COUNT(*) as order_count;

-- With ordering and limit
FROM Products
WHERE category = 'Electronics'
ORDER BY price DESC
LIMIT 10;
```

### 2. Graph Traversal (Native)

```dql
-- Single hop
FROM Users WHERE name = 'Alice'
TRAVERSE -[:FOLLOWS]-> AS friend
SELECT friend.name;

-- Multi-hop with depth
FROM Users WHERE name = 'Alice'
TRAVERSE -[:FOLLOWS*1..3]-> AS connection
SELECT connection.name, DEPTH(connection) as degree;

-- Bidirectional
FROM Users WHERE name = 'Alice'
TRAVERSE <-[:FOLLOWS]-> AS mutual_follower
SELECT mutual_follower.name;

-- Path finding
FROM Users WHERE name = 'Alice'
TRAVERSE -[:KNOWS*]-> Users WHERE name = 'Bob'
SHORTEST PATH
SELECT PATH;
```

### 3. TRUE Hybrid Queries (The Innovation!)

```dql
-- Example 1: E-commerce recommendation
FROM Users WHERE city = 'NYC' AND age > 25
TRAVERSE -[:PURCHASED]-> Product
WHERE Product.category = 'Electronics' AND Product.rating > 4.0
GROUP BY Product.id
SELECT Product.name, COUNT(*) as purchase_count
ORDER BY purchase_count DESC
LIMIT 10;

-- Example 2: Social network analysis
FROM Users WHERE occupation = 'Engineer'
TRAVERSE -[:FOLLOWS]-> Person
TRAVERSE -[:WORKS_AT]-> Company
WHERE Company.industry = 'Tech'
SELECT Company.name, COUNT(DISTINCT Person) as engineer_followers
GROUP BY Company.name;

-- Example 3: Complex multi-hop with aggregation
FROM Users AS u
WHERE u.registration_date > '2024-01-01'
TRAVERSE -[:PURCHASED]-> Product AS p
TRAVERSE <-[:PURCHASED]- User AS other
WHERE other.id != u.id  -- Exclude original user
SELECT other.name,
       COUNT(DISTINCT p) as common_products,
       AVG(p.rating) as avg_rating
GROUP BY other.id
HAVING common_products > 3
ORDER BY common_products DESC;
```

### 4. Pheromone Optimization Hints (Unique to Deed!)

```dql
-- Hint that this query should follow strong pheromone trails
FROM Users WHERE city = 'NYC'
TRAVERSE -[:PURCHASED]-> Product
WITH PHEROMONE STRENGTH > 0.5  -- Only follow well-traveled paths
SELECT Product.name;

-- Exploratory query (encourage new paths)
FROM Users
TRAVERSE -[:MIGHT_LIKE]-> Product
WITH EXPLORE RATE 0.3  -- 30% exploration vs exploitation
SELECT Product.name;

-- Force ant colony optimization
FROM Users WHERE age > 25
TRAVERSE -[:COMPLEX_RELATIONSHIP*1..5]-> Target
OPTIMIZE WITH ANT_COLONY(ants=50, iterations=10)
SELECT Target.name;
```

### 5. Create Operations

```dql
-- Create entity (relational-style)
CREATE Users { name: 'Alice', age: 28, city: 'NYC' };

-- Create with relationship (graph-style)
CREATE (u:User { name: 'Alice', age: 28 })
CREATE (p:Product { name: 'Laptop', price: 1299 })
CREATE (u)-[:PURCHASED { date: '2024-10-21' }]->(p);

-- Bulk create from query result
FROM Users WHERE city = 'NYC'
TRAVERSE -[:PURCHASED]-> Product
WHERE Product.rating > 4.5
CREATE (User)-[:RECOMMENDS]->(Product);
```

### 6. Update Operations

```dql
-- Update entities
FROM Users WHERE name = 'Alice'
UPDATE { city: 'SF', updated_at: NOW() };

-- Update via traversal
FROM Users WHERE name = 'Alice'
TRAVERSE -[:FOLLOWS]-> follower
UPDATE follower { notified: true };
```

---

## Why DQL is Truly Unified

### Comparison

| Approach | Queries Needed | Languages | Execution Plans |
|----------|----------------|-----------|-----------------|
| **PostgreSQL + Neo4j** | 2+ | SQL + Cypher | 2 separate |
| **SQL + GQL (my old approach)** | 2+ | SQL + GQL | 2 separate |
| **DQL (new approach)** | 1 | DQL only | 1 unified |

### Example: Product Recommendation

**Old Approach (SQL + GQL)**:
```sql
-- Query 1: Get NYC users (SQL)
SELECT id FROM Users WHERE city = 'NYC';

-- Query 2: Get their purchases (GQL, separate!)
MATCH (u:User)-[:PURCHASED]->(p:Product)
WHERE u.id IN [results_from_query_1]
RETURN p.name, COUNT(*) as popularity;
```
- 2 queries
- 2 round trips
- Manual result passing
- 2 optimization passes

**New Approach (DQL)**:
```dql
FROM Users WHERE city = 'NYC'
TRAVERSE -[:PURCHASED]-> Product
GROUP BY Product.id
SELECT Product.name, COUNT(*) as popularity
ORDER BY popularity DESC;
```
- 1 query
- 1 round trip
- Automatic result flow
- **1 unified optimization pass** (biological algorithms optimize entire query!)

---

## Implementation Strategy

### Parser
```rust
// Single unified parser
pub struct DQLParser {
    lexer: Lexer,
}

impl DQLParser {
    pub fn parse(&mut self, query: &str) -> Result<QueryPlan, Error> {
        // Parse INTO unified IR (Intermediate Representation)
        let ast = self.parse_ast(query)?;

        // All DQL queries become the SAME IR
        // Whether relational-style or graph-style
        let ir = self.lower_to_ir(ast)?;

        // Single optimization pass
        let optimized = self.optimize_with_ants(ir)?;

        Ok(optimized)
    }
}
```

### Unified IR
```rust
pub enum QueryPlan {
    EntityScan {
        collection: String,
        predicate: Option<Predicate>,
    },
    Traverse {
        start: Box<QueryPlan>,
        edge_type: String,
        direction: Direction,
        depth: Range<usize>,
        filter: Option<Predicate>,
    },
    Aggregate {
        source: Box<QueryPlan>,
        group_by: Vec<String>,
        aggregations: Vec<Aggregation>,
    },
    // ... etc
}
```

**Key**: Both relational and graph operations compile to the SAME IR. Then biological optimization works on the unified plan.

---

## Competitive Advantage

### vs SQL-only (PostgreSQL)
- ❌ PostgreSQL: Recursive CTEs for graphs (slow, awkward)
- ✅ DQL: Native graph traversal syntax

### vs Graph-only (Neo4j)
- ❌ Neo4j: Awkward for analytical queries, aggregations
- ✅ DQL: Native relational operations

### vs Multi-model (ArangoDB)
- ❌ ArangoDB: Still separate AQL syntax for different models
- ✅ DQL: Single language, seamless mixing

### vs SQL + GQL wrapper
- ❌ Wrapper: Two queries, two optimizations
- ✅ DQL: One query, one biological optimization

---

## The Real Innovation

**With DQL + Biological Optimization**:

1. **Ant Colony explores the ENTIRE hybrid query plan**
   - Should we start with Users scan or Product scan?
   - Should we traverse then filter, or filter then traverse?
   - Which index to use? Which pheromone trail to follow?
   - All decided holistically

2. **Pheromone trails cross relational/graph boundaries**
   - If `Users WHERE city='NYC' → PURCHASED → Product` is frequently executed
   - The pheromone strengthens on BOTH the index scan AND the traversal
   - Future queries automatically benefit

3. **Stigmergy cache learns hybrid patterns**
   - "Oh, queries that filter on city and then traverse purchases do better with index-first strategy"
   - Automatically applies this learning to new queries

**This is NOT possible with SQL + GQL separately!**

---

## Killer Feature: Adaptive Query Rewriting

```dql
-- User writes:
FROM Users WHERE city = 'NYC'
TRAVERSE -[:PURCHASED]-> Product
WHERE Product.price > 100;

-- Ant colony tries multiple execution orders:
-- Plan A: Users → filter → traverse → filter
-- Plan B: Product → filter → reverse traverse → filter
-- Plan C: Use pheromone-strong path User→Product directly
-- Plan D: Use materialized view if available

-- After 20 ants explore, bee quorum picks best
-- Stigmergy cache remembers for next time
-- Next similar query uses learned plan immediately
```

This level of optimization is **impossible** when SQL and GQL are separate languages executed by separate engines.

---

## Migration Path

### Phase 1: DQL Subset (SQL-compatible)
```dql
-- All SQL queries work in DQL
SELECT name, age FROM Users WHERE city = 'NYC';
```

### Phase 2: Add Graph Extensions
```dql
-- Extend with TRAVERSE
SELECT name FROM Users WHERE city = 'NYC'
TRAVERSE -[:PURCHASED]-> SELECT Product.name;
```

### Phase 3: Full DQL
```dql
-- Native DQL syntax
FROM Users WHERE city = 'NYC'
TRAVERSE -[:PURCHASED]-> Product
SELECT User.name, Product.name;
```

---

## DQL Grammar (Simplified BNF)

```bnf
query ::=
    | from_clause traverse_clause* where_clause? select_clause
    | create_statement
    | update_statement

from_clause ::= "FROM" entity_type ("AS" alias)? where_clause?

traverse_clause ::=
    "TRAVERSE" edge_pattern ("AS" alias)? where_clause?

edge_pattern ::=
    | "-[:" edge_type ("*" range)? "]->"  // Outgoing
    | "<-[:" edge_type ("*" range)? "]-"  // Incoming
    | "<-[:" edge_type ("*" range)? "]->" // Both

where_clause ::= "WHERE" predicate

select_clause ::=
    | "SELECT" projection_list
    | "GROUP BY" group_list "SELECT" aggregation_list

predicate ::=
    | property comparison value
    | predicate "AND" predicate
    | predicate "OR" predicate
```

---

## Summary

### Old Approach (Flawed)
- SQL for relational
- GQL for graph
- Two separate query parsers
- Two separate execution plans
- Two separate optimizations
= **Just a wrapper!** ❌

### New Approach (Unified)
- **DQL** for everything
- Single parser
- Single unified IR
- Single execution plan
- **Single biological optimization** (the key!)
= **True innovation!** ✅

**The biological algorithms only provide value when they optimize the ENTIRE hybrid query, not separate SQL/GQL pieces.**
