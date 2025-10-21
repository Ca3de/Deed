# Aggregations in Deed Database (DQL)

## Overview

Deed now supports full SQL-style aggregation functions with GROUP BY and HAVING clauses! This makes Deed production-ready for analytics and reporting queries.

## Supported Aggregate Functions

- **COUNT(*)** / **COUNT(field)** - Count rows or non-null values
- **SUM(field)** - Sum numeric values
- **AVG(field)** - Average of numeric values
- **MIN(field)** - Minimum value
- **MAX(field)** - Maximum value

## Basic Aggregation Examples

### COUNT - Total number of users

```dql
SELECT COUNT(*) AS total_users
FROM Users
```

**Result:**
```
total_users: 1250
```

### SUM - Total revenue

```dql
SELECT SUM(amount) AS total_revenue
FROM Orders
WHERE status = 'completed'
```

**Result:**
```
total_revenue: 125000.50
```

### AVG - Average order value

```dql
SELECT AVG(amount) AS avg_order_value
FROM Orders
```

**Result:**
```
avg_order_value: 85.75
```

### MIN/MAX - Price range

```dql
SELECT
    MIN(price) AS lowest_price,
    MAX(price) AS highest_price
FROM Products
```

**Result:**
```
lowest_price: 5.99
highest_price: 999.99
```

## GROUP BY Examples

### Users by city

```dql
SELECT
    city,
    COUNT(*) AS user_count
FROM Users
GROUP BY city
```

**Result:**
```
city          | user_count
--------------|------------
New York      | 450
Los Angeles   | 320
Chicago       | 180
Houston       | 120
Phoenix       | 180
```

### Sales by product category

```dql
SELECT
    category,
    COUNT(*) AS num_products,
    AVG(price) AS avg_price,
    SUM(stock) AS total_inventory
FROM Products
GROUP BY category
```

**Result:**
```
category      | num_products | avg_price | total_inventory
--------------|--------------|-----------|----------------
Electronics   | 150          | 299.99    | 5400
Clothing      | 320          | 45.50     | 12800
Home & Garden | 180          | 75.25     | 6200
```

### Orders by customer and status

```dql
SELECT
    customer_id,
    status,
    COUNT(*) AS order_count,
    SUM(amount) AS total_spent
FROM Orders
GROUP BY customer_id, status
ORDER BY total_spent DESC
LIMIT 10
```

**Result:**
```
customer_id | status    | order_count | total_spent
------------|-----------|-------------|-------------
1042        | completed | 45          | 12450.00
1893        | completed | 38          | 9875.50
2341        | completed | 52          | 8920.25
```

## HAVING Examples

The HAVING clause filters **after** aggregation (unlike WHERE which filters before).

### Cities with more than 100 users

```dql
SELECT
    city,
    COUNT(*) AS user_count
FROM Users
GROUP BY city
HAVING COUNT(*) > 100
```

**Result:**
```
city          | user_count
--------------|------------
New York      | 450
Los Angeles   | 320
Chicago       | 180
Houston       | 120
Phoenix       | 180
```

### Product categories with average price over $100

```dql
SELECT
    category,
    COUNT(*) AS num_products,
    AVG(price) AS avg_price
FROM Products
GROUP BY category
HAVING AVG(price) > 100
```

**Result:**
```
category      | num_products | avg_price
--------------|--------------|----------
Electronics   | 150          | 299.99
Appliances    | 45           | 425.50
```

### High-value customers (spent more than $5000)

```dql
SELECT
    customer_id,
    COUNT(*) AS order_count,
    SUM(amount) AS total_spent
FROM Orders
WHERE status = 'completed'
GROUP BY customer_id
HAVING SUM(amount) > 5000
ORDER BY total_spent DESC
```

**Result:**
```
customer_id | order_count | total_spent
------------|-------------|-------------
1042        | 45          | 12450.00
1893        | 38          | 9875.50
2341        | 52          | 8920.25
5621        | 28          | 7250.00
3892        | 41          | 6180.75
```

## Complex Analytics Examples

### Monthly sales report

```dql
SELECT
    MONTH(order_date) AS month,
    COUNT(*) AS total_orders,
    SUM(amount) AS revenue,
    AVG(amount) AS avg_order_value,
    MIN(amount) AS smallest_order,
    MAX(amount) AS largest_order
FROM Orders
WHERE YEAR(order_date) = 2024
GROUP BY MONTH(order_date)
ORDER BY month
```

### Product performance by category

```dql
SELECT
    p.category,
    COUNT(DISTINCT p.id) AS products_sold,
    COUNT(o.id) AS total_orders,
    SUM(o.quantity) AS units_sold,
    SUM(o.amount) AS total_revenue,
    AVG(o.amount) AS avg_sale_price
FROM Products p
TRAVERSE -[:ORDERED_IN]-> Orders o
GROUP BY p.category
HAVING COUNT(o.id) > 50
ORDER BY total_revenue DESC
```

**Note:** This combines **TRAVERSE** (Deed's graph feature) with **aggregations**!

## Comparison: WHERE vs HAVING

### WHERE - Filters **before** grouping

```dql
SELECT
    city,
    COUNT(*) AS active_users
FROM Users
WHERE is_active = true  -- Filter BEFORE grouping
GROUP BY city
```

This first filters to only active users, then groups them by city.

### HAVING - Filters **after** grouping

```dql
SELECT
    city,
    COUNT(*) AS user_count
FROM Users
GROUP BY city
HAVING COUNT(*) > 100  -- Filter AFTER grouping
```

This groups all users by city, then filters to only show cities with >100 users.

### Both Together

```dql
SELECT
    city,
    COUNT(*) AS active_users
FROM Users
WHERE is_active = true      -- Filter BEFORE: only active users
GROUP BY city
HAVING COUNT(*) > 50        -- Filter AFTER: only cities with >50 active users
ORDER BY active_users DESC
```

## Implementation Notes

### Query Execution Order

DQL executes queries in this order (standard SQL):

1. **FROM** - Load data from collection
2. **TRAVERSE** - Graph traversal (Deed-specific)
3. **WHERE** - Filter rows before grouping
4. **GROUP BY** - Group rows by specified fields
5. **Aggregates** - Compute COUNT, SUM, AVG, MIN, MAX
6. **HAVING** - Filter groups after aggregation
7. **SELECT** - Project final columns
8. **ORDER BY** - Sort results
9. **LIMIT/OFFSET** - Paginate results

### Performance Characteristics

- **GROUP BY**: O(N log N) - Uses hash-based grouping
- **COUNT**: O(1) per group
- **SUM/AVG**: O(N) per group
- **MIN/MAX**: O(N) per group

### Biological Optimization

The Ant Colony Optimizer learns optimal execution strategies:
- Which fields to index for faster GROUP BY
- Whether to use hash-based or sort-based grouping
- How to cache frequent aggregation patterns

## Production Use Cases

### 1. Dashboard Analytics

```dql
SELECT
    DATE(created_at) AS date,
    COUNT(*) AS new_users,
    COUNT(DISTINCT country) AS countries
FROM Users
WHERE created_at >= '2024-01-01'
GROUP BY DATE(created_at)
ORDER BY date DESC
LIMIT 30
```

### 2. Sales Reporting

```dql
SELECT
    salesperson_id,
    COUNT(*) AS deals_closed,
    SUM(amount) AS total_sales,
    AVG(amount) AS avg_deal_size
FROM Deals
WHERE status = 'won' AND YEAR(closed_date) = 2024
GROUP BY salesperson_id
HAVING SUM(amount) > 100000
ORDER BY total_sales DESC
```

### 3. Inventory Management

```dql
SELECT
    warehouse_id,
    category,
    COUNT(*) AS product_count,
    SUM(stock) AS total_units,
    SUM(stock * cost) AS inventory_value
FROM Inventory
GROUP BY warehouse_id, category
HAVING SUM(stock) < 100  -- Low stock alert
```

### 4. User Engagement Metrics

```dql
SELECT
    cohort_month,
    COUNT(DISTINCT user_id) AS active_users,
    SUM(sessions) AS total_sessions,
    AVG(sessions) AS avg_sessions_per_user
FROM UserActivity
WHERE activity_date >= '2024-01-01'
GROUP BY cohort_month
ORDER BY cohort_month
```

## Hybrid Relational + Graph Queries

One of Deed's unique features: **Combine aggregations with graph traversal!**

### Example: Social network friend statistics

```dql
SELECT
    u.id,
    u.name,
    COUNT(*) AS friend_count,
    AVG(f.age) AS avg_friend_age
FROM Users u
TRAVERSE -[:FRIEND_OF]-> Users f
GROUP BY u.id, u.name
HAVING COUNT(*) > 10
ORDER BY friend_count DESC
```

### Example: Product recommendation analytics

```dql
SELECT
    p.category,
    COUNT(DISTINCT c.id) AS customer_count,
    SUM(o.amount) AS revenue
FROM Products p
TRAVERSE <-[:PURCHASED]- Customers c
TRAVERSE -[:PLACED]-> Orders o
WHERE o.status = 'completed'
GROUP BY p.category
HAVING COUNT(DISTINCT c.id) > 100
```

## Next Steps

With aggregations complete, Deed now supports:
- ✅ Full CRUD operations
- ✅ Graph traversal (TRAVERSE)
- ✅ Aggregations (COUNT, SUM, AVG, MIN, MAX)
- ✅ GROUP BY and HAVING
- ✅ Biological optimization
- ✅ Optional schemas

**Still needed for production:**
- Transactions (BEGIN/COMMIT/ROLLBACK)
- Security (authentication, permissions)
- Indexes (B-tree for faster lookups)
- Backup/restore

See `FEATURE_AUDIT.md` for the complete roadmap!

---

**Generated:** 2025-10-21
**Deed Version:** 0.1.0 (Pre-production)
