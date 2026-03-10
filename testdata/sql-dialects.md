# SQL Dialect Highlighting Test

## Generic SQL
```sql
SELECT id, name FROM users WHERE active = 1 GROUP BY id ORDER BY name;
```

## MySQL (`sql.mysql`)
```sql.mysql
SELECT * FROM `orders` o
JOIN `customers` c ON o.customer_id = c.id
WHERE o.status = 'pending'
  AND o.total > 100.00
ON DUPLICATE KEY UPDATE updated_at = NOW();

SELECT GROUP_CONCAT(tag SEPARATOR ', ') FROM article_tags GROUP BY article_id;
```

## PostgreSQL (`sql.postgresql`)
```sql.postgresql
SELECT id, data->>'name' AS name, tags @> ARRAY['vip']::text[]
FROM customers
WHERE data @> '{"active": true}'::jsonb
  AND email ILIKE '%@example.com';

INSERT INTO events (payload, created_at)
VALUES ('{"type": "login"}'::jsonb, NOW())
RETURNING id, created_at;

CREATE OR REPLACE FUNCTION get_stats(p_id INT)
RETURNS TABLE(cnt BIGINT, avg_val NUMERIC) AS $$
BEGIN
  RETURN QUERY
    SELECT COUNT(*), AVG(value) FROM measurements WHERE id = p_id;
END;
$$ LANGUAGE plpgsql;
```

## Oracle (`sql.oracle`)
```sql.oracle
SELECT e.name, d.dept_name, LEVEL AS depth
FROM employees e
JOIN departments d ON e.dept_id = d.id
START WITH e.manager_id IS NULL
CONNECT BY PRIOR e.id = e.manager_id;

SELECT NVL(phone, 'N/A') AS phone,
       DECODE(status, 1, 'Active', 2, 'Inactive', 'Unknown') AS status_label
FROM contacts
WHERE ROWNUM <= 20;
```

## Microsoft SQL Server (`sql.mssql`)
```sql.mssql
DECLARE @threshold INT = 100;
DECLARE @affected INT;

SELECT TOP 10 WITH TIES
    [u].[user_id],
    [u].[email],
    @@ROWCOUNT AS system_count
FROM [dbo].[users] AS [u] WITH (NOLOCK)
WHERE @threshold > 0
  AND [u].[status] = 'active';

UPDATE [dbo].[orders]
SET status = 'shipped'
OUTPUT INSERTED.order_id, INSERTED.shipped_at
WHERE order_id IN (SELECT order_id FROM [staging].[ready_orders]);

GO
```

## Aliases also work
```mysql
SELECT 1;
```
```pgsql
SELECT 1;
```
```tsql
SELECT 1;
```
```plsql
SELECT 1 FROM dual;
```
