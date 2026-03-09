# SQL Dialect Differences: A Practical Comparison

This document showcases the key syntax and feature differences between four major SQL dialects: **MySQL**, **PostgreSQL**, **Oracle**, and **Microsoft SQL Server (T-SQL)**. Use the appropriate fence tag in your markdown to get dialect-specific syntax highlighting.

---

## 1. String and Identifier Quoting

Different dialects use different characters to quote identifiers (table names, column names, etc.).

### MySQL
```sql.mysql
-- Backticks for identifiers (MySQL-specific)
SELECT `user_id`, `email` FROM `users` WHERE `status` = 'active';
SELECT * FROM `order_detail` WHERE `order_id` = 12345;
```

### PostgreSQL
```sql.postgresql
-- Double quotes for identifiers (SQL standard)
SELECT "user_id", "email" FROM "users" WHERE "status" = 'active';
SELECT * FROM "order_detail" WHERE "order_id" = 12345;
```

### Oracle
```sql.oracle
-- Double quotes for identifiers (SQL standard)
SELECT "user_id", "email" FROM "users" WHERE "status" = 'active';
SELECT * FROM "order_detail" WHERE "order_id" = 12345;
```

### T-SQL (Microsoft SQL Server)
```sql.mssql
-- Square brackets for identifiers (T-SQL idiomatic)
SELECT [user_id], [email] FROM [users] WHERE [status] = 'active';
SELECT * FROM [dbo].[order_detail] WHERE [order_id] = 12345;
```

---

## 2. Row Limiting / Top-N Queries

Each dialect has a different way to limit the number of rows returned.

### MySQL
```sql.mysql
-- LIMIT clause (MySQL-specific)
SELECT * FROM orders WHERE status = 'pending' LIMIT 10;
SELECT * FROM orders WHERE status = 'pending' LIMIT 10 OFFSET 20;
```

### PostgreSQL
```sql.postgresql
-- LIMIT clause (also supports FETCH standard)
SELECT * FROM orders WHERE status = 'pending' LIMIT 10;
SELECT * FROM orders WHERE status = 'pending' LIMIT 10 OFFSET 20;
-- SQL standard syntax (also works in PostgreSQL 12+)
SELECT * FROM orders WHERE status = 'pending' FETCH FIRST 10 ROWS ONLY;
```

### Oracle
```sql.oracle
-- ROWNUM pseudocolumn (older style) or FETCH FIRST (newer, 12c+)
SELECT * FROM orders WHERE ROWNUM <= 10 AND status = 'pending';
-- SQL standard syntax (Oracle 12c+)
SELECT * FROM orders WHERE status = 'pending' FETCH FIRST 10 ROWS ONLY;
```

### T-SQL (Microsoft SQL Server)
```sql.mssql
-- TOP clause (T-SQL-specific)
SELECT TOP 10 * FROM orders WHERE status = 'pending';
SELECT TOP 10 PERCENT * FROM orders WHERE status = 'pending';
SELECT TOP 10 WITH TIES * FROM orders ORDER BY created_at DESC;
```

---

## 3. Null Coalescing Functions

Each dialect has its own function for handling NULL values.

### MySQL
```sql.mysql
-- IFNULL() function
SELECT
    user_id,
    IFNULL(phone_number, 'N/A') AS phone,
    IFNULL(email, 'unknown@example.com') AS email
FROM users;
```

### PostgreSQL
```sql.postgresql
-- COALESCE() function (SQL standard, more flexible)
SELECT
    user_id,
    COALESCE(phone_number, 'N/A') AS phone,
    COALESCE(email, 'unknown@example.com') AS email
FROM users;
```

### Oracle
```sql.oracle
-- NVL() function or NVL2() for conditional logic
SELECT
    user_id,
    NVL(phone_number, 'N/A') AS phone,
    NVL2(email, email, 'unknown@example.com') AS email
FROM users;
-- DECODE() for multi-way case statement (Oracle-specific)
SELECT
    user_id,
    DECODE(status, 1, 'Active', 2, 'Inactive', 3, 'Pending', 'Unknown') AS status_name
FROM users;
```

### T-SQL (Microsoft SQL Server)
```sql.mssql
-- ISNULL() function (similar to IFNULL but T-SQL specific)
SELECT
    user_id,
    ISNULL(phone_number, 'N/A') AS phone,
    ISNULL(email, 'unknown@example.com') AS email
FROM users;
-- IIF() for conditional expressions
SELECT
    user_id,
    IIF(status = 1, 'Active', 'Inactive') AS status_name
FROM users;
```

---

## 4. Upsert Operations (Insert or Update)

Each dialect handles INSERT-or-UPDATE differently.

### MySQL
```sql.mysql
-- ON DUPLICATE KEY UPDATE (MySQL-specific)
INSERT INTO users (user_id, email, last_login)
VALUES (123, 'john@example.com', NOW())
ON DUPLICATE KEY UPDATE last_login = NOW();
```

### PostgreSQL
```sql.postgresql
-- INSERT ... ON CONFLICT ... DO UPDATE (PostgreSQL 9.5+)
INSERT INTO users (user_id, email, last_login)
VALUES (123, 'john@example.com', NOW())
ON CONFLICT (user_id) DO UPDATE SET last_login = NOW();
```

### Oracle
```sql.oracle
-- MERGE statement (SQL standard, Oracle-style)
MERGE INTO users u
USING (SELECT 123 AS user_id, 'john@example.com' AS email, SYSDATE AS last_login FROM dual) src
ON (u.user_id = src.user_id)
WHEN MATCHED THEN
  UPDATE SET u.last_login = src.last_login
WHEN NOT MATCHED THEN
  INSERT (user_id, email, last_login) VALUES (src.user_id, src.email, src.last_login);
```

### T-SQL (Microsoft SQL Server)
```sql.mssql
-- MERGE statement (SQL standard style)
MERGE INTO users AS u
USING (SELECT 123 AS user_id, 'john@example.com' AS email, GETDATE() AS last_login) AS src
ON u.user_id = src.user_id
WHEN MATCHED THEN
  UPDATE SET u.last_login = src.last_login
WHEN NOT MATCHED THEN
  INSERT (user_id, email, last_login) VALUES (src.user_id, src.email, src.last_login);
```

---

## 5. Returning Modified Rows

Getting back the rows that were just inserted or updated.

### MySQL
```sql.mysql
-- MySQL has no native RETURNING clause
-- Workaround: use LAST_INSERT_ID() for single inserts
INSERT INTO users (email, status) VALUES ('john@example.com', 'active');
SELECT LAST_INSERT_ID() AS user_id;
```

### PostgreSQL
```sql.postgresql
-- RETURNING clause (PostgreSQL-specific, very powerful)
INSERT INTO users (email, status) VALUES ('john@example.com', 'active')
RETURNING user_id, email, status, created_at;

UPDATE users SET last_login = NOW() WHERE user_id = 123
RETURNING user_id, last_login;

DELETE FROM inactive_users WHERE created_at < '2020-01-01'
RETURNING user_id, email;
```

### Oracle
```sql.oracle
-- RETURNING ... INTO clause (Oracle-specific)
DECLARE
    v_user_id users.user_id%TYPE;
    v_created_at users.created_at%TYPE;
BEGIN
    INSERT INTO users (email, status) VALUES ('john@example.com', 'active')
    RETURNING user_id, created_at INTO v_user_id, v_created_at;
    DBMS_OUTPUT.PUT_LINE('Created user ' || v_user_id);
END;
```

### T-SQL (Microsoft SQL Server)
```sql.mssql
-- OUTPUT clause (T-SQL-specific, returns INSERTED/DELETED virtual tables)
INSERT INTO users (email, status)
OUTPUT INSERTED.user_id, INSERTED.email, INSERTED.status, INSERTED.created_at
VALUES ('john@example.com', 'active');

UPDATE users SET last_login = GETDATE() WHERE user_id = 123
OUTPUT INSERTED.user_id, INSERTED.last_login;

DELETE FROM inactive_users WHERE created_at < '2020-01-01'
OUTPUT DELETED.user_id, DELETED.email;
```

---

## 6. String Aggregation

Concatenating multiple rows into a single string.

### MySQL
```sql.mysql
-- GROUP_CONCAT() function
SELECT
    order_id,
    GROUP_CONCAT(product_name SEPARATOR ', ') AS products,
    GROUP_CONCAT(DISTINCT category SEPARATOR ' | ') AS categories
FROM order_items
GROUP BY order_id;
```

### PostgreSQL
```sql.postgresql
-- STRING_AGG() function (or ARRAY_AGG for arrays)
SELECT
    order_id,
    STRING_AGG(product_name, ', ') AS products,
    STRING_AGG(DISTINCT category, ' | ') AS categories
FROM order_items
GROUP BY order_id;
-- Array aggregation (unique to PostgreSQL)
SELECT order_id, ARRAY_AGG(product_name) AS product_array
FROM order_items
GROUP BY order_id;
```

### Oracle
```sql.oracle
-- LISTAGG() function (Oracle 11g+)
SELECT
    order_id,
    LISTAGG(product_name, ', ') WITHIN GROUP (ORDER BY product_name) AS products
FROM order_items
GROUP BY order_id;
-- WM_CONCAT() (undocumented, older method)
SELECT order_id, WM_CONCAT(product_name) AS products FROM order_items GROUP BY order_id;
```

### T-SQL (Microsoft SQL Server)
```sql.mssql
-- STRING_AGG() function (SQL Server 2017+)
SELECT
    order_id,
    STRING_AGG(product_name, ', ') WITHIN GROUP (ORDER BY product_name) AS products
FROM order_items
GROUP BY order_id;
-- FOR XML PATH (older method, pre-2017)
SELECT
    order_id,
    STUFF((SELECT ',' + product_name FROM order_items AS oi2
           WHERE oi2.order_id = oi.order_id FOR XML PATH('')), 1, 1, '') AS products
FROM order_items AS oi
GROUP BY order_id;
```

---

## 7. Date and Time Functions

Getting current date/time and formatting.

### MySQL
```sql.mysql
-- MySQL date/time functions
SELECT
    NOW() AS current_timestamp,
    CURDATE() AS today,
    CURTIME() AS current_time,
    DATE_FORMAT(NOW(), '%Y-%m-%d %H:%i:%s') AS formatted_date,
    DATE_ADD(NOW(), INTERVAL 7 DAY) AS one_week_from_now,
    YEAR(created_at) AS year_created,
    MONTH(created_at) AS month_created
FROM orders;
```

### PostgreSQL
```sql.postgresql
-- PostgreSQL date/time functions
SELECT
    NOW() AS current_timestamp,
    CURRENT_DATE AS today,
    CURRENT_TIME AS current_time,
    TO_CHAR(NOW(), 'YYYY-MM-DD HH24:MI:SS') AS formatted_date,
    NOW() + INTERVAL '7 days' AS one_week_from_now,
    EXTRACT(YEAR FROM created_at) AS year_created,
    EXTRACT(MONTH FROM created_at) AS month_created,
    DATE_TRUNC('month', created_at) AS first_of_month
FROM orders;
```

### Oracle
```sql.oracle
-- Oracle date/time functions
SELECT
    SYSDATE AS current_timestamp,
    TRUNC(SYSDATE) AS today,
    TO_CHAR(SYSDATE, 'YYYY-MM-DD HH24:MI:SS') AS formatted_date,
    SYSDATE + 7 AS one_week_from_now,
    EXTRACT(YEAR FROM created_at) AS year_created,
    EXTRACT(MONTH FROM created_at) AS month_created,
    TRUNC(created_at, 'MONTH') AS first_of_month
FROM orders;
```

### T-SQL (Microsoft SQL Server)
```sql.mssql
-- T-SQL date/time functions
SELECT
    GETDATE() AS current_timestamp,
    CAST(GETDATE() AS DATE) AS today,
    CAST(GETDATE() AS TIME) AS current_time,
    FORMAT(GETDATE(), 'yyyy-MM-dd HH:mm:ss') AS formatted_date,
    DATEADD(DAY, 7, GETDATE()) AS one_week_from_now,
    YEAR(created_at) AS year_created,
    MONTH(created_at) AS month_created,
    EOMONTH(created_at) AS last_day_of_month
FROM orders;
```

---

## 8. Hierarchical/Recursive Queries

Querying parent-child relationships (organizational hierarchies, category trees, etc.).

### MySQL
```sql.mysql
-- MySQL has limited hierarchical query support
-- Common approach: self-join with multiple levels
SELECT
    e.employee_id,
    e.name AS employee_name,
    m.name AS manager_name,
    gm.name AS grand_manager_name
FROM employees e
LEFT JOIN employees m ON e.manager_id = m.employee_id
LEFT JOIN employees gm ON m.manager_id = gm.employee_id
WHERE e.department_id = 5;
```

### PostgreSQL
```sql.postgresql
-- Recursive CTE (Common Table Expression)
WITH RECURSIVE employee_hierarchy AS (
    -- Base case: top-level employees (no manager)
    SELECT employee_id, name, manager_id, 1 AS level
    FROM employees
    WHERE manager_id IS NULL

    UNION ALL

    -- Recursive case: employees reporting to previous level
    SELECT e.employee_id, e.name, e.manager_id, eh.level + 1
    FROM employees e
    INNER JOIN employee_hierarchy eh ON e.manager_id = eh.employee_id
)
SELECT * FROM employee_hierarchy ORDER BY level, name;
```

### Oracle
```sql.oracle
-- CONNECT BY clause (Oracle-specific, very powerful)
SELECT
    LPAD(' ', (LEVEL-1)*2) || name AS org_chart,
    employee_id,
    manager_id,
    LEVEL AS hierarchy_level
FROM employees
START WITH manager_id IS NULL  -- Start with top-level manager
CONNECT BY PRIOR employee_id = manager_id
ORDER BY LEVEL, name;

-- Alternative: get all reports under a specific manager
SELECT employee_id, name FROM employees
WHERE ROWNUM <= 100
START WITH manager_id = 1  -- Reports to employee_id = 1
CONNECT BY PRIOR employee_id = manager_id;
```

### T-SQL (Microsoft SQL Server)
```sql.mssql
-- Recursive CTE
WITH employee_hierarchy AS (
    -- Base case: top-level employees (no manager)
    SELECT employee_id, name, manager_id, 1 AS level
    FROM employees
    WHERE manager_id IS NULL

    UNION ALL

    -- Recursive case: employees reporting to previous level
    SELECT e.employee_id, e.name, e.manager_id, eh.level + 1
    FROM employees e
    INNER JOIN employee_hierarchy eh ON e.manager_id = eh.employee_id
    WHERE eh.level < 10  -- Prevent infinite recursion
)
SELECT * FROM employee_hierarchy
ORDER BY level, name
OPTION (MAXRECURSION 0);  -- 0 = unlimited recursion (0-32767)
```

---

## 9. JSON Support

Storing and querying JSON data.

### MySQL
```sql.mysql
-- JSON functions (MySQL 5.7+)
SELECT
    user_id,
    data->'$.name' AS name,               -- Extract as text
    JSON_EXTRACT(data, '$.age') AS age,   -- Explicit extraction
    JSON_UNQUOTE(data->'$.email') AS email
FROM users
WHERE JSON_EXTRACT(data, '$.status') = 'active';

-- Insert JSON
INSERT INTO users (user_id, data)
VALUES (1, JSON_OBJECT('name', 'John', 'age', 30, 'email', 'john@example.com'));
```

### PostgreSQL
```sql.postgresql
-- JSONB type (more efficient than JSON)
SELECT
    user_id,
    data->>'name' AS name,              -- Extract as text
    (data->>'age')::INTEGER AS age,     -- With type casting
    data->>'email' AS email
FROM users
WHERE data @> '{"status": "active"}'::jsonb;  -- Contains operator

-- JSON operators
SELECT
    user_id,
    CASE WHEN data ? 'premium' THEN 'Has premium' ELSE 'Basic' END AS account_type
FROM users
WHERE data ?| ARRAY['premium', 'vip'];  -- Any key exists

-- Insert JSON
INSERT INTO users (user_id, data)
VALUES (1, jsonb_build_object('name', 'John', 'age', 30, 'email', 'john@example.com'));
```

### Oracle
```sql.oracle
-- JSON functions (Oracle 12c+, stored as CLOB)
SELECT
    user_id,
    JSON_VALUE(data, '$.name') AS name,
    JSON_VALUE(data, '$.age' RETURNING NUMBER) AS age,
    JSON_VALUE(data, '$.email') AS email
FROM users
WHERE JSON_VALUE(data, '$.status') = 'active';

-- Insert JSON (as string literal)
INSERT INTO users (user_id, data)
VALUES (1, '{"name":"John","age":30,"email":"john@example.com"}');
```

### T-SQL (Microsoft SQL Server)
```sql.mssql
-- JSON functions (SQL Server 2016+)
SELECT
    user_id,
    JSON_VALUE(data, '$.name') AS name,
    JSON_VALUE(data, '$.age') AS age,
    JSON_VALUE(data, '$.email') AS email
FROM users
WHERE JSON_VALUE(data, '$.status') = 'active';

-- JSON_QUERY returns JSON object/array
SELECT
    user_id,
    JSON_QUERY(data, '$.address') AS address_json
FROM users;

-- Insert JSON
INSERT INTO users (user_id, data)
VALUES (1, JSON_OBJECT('name':'John', 'age':30, 'email':'john@example.com'));
```

---

## 10. Variables and Parameters

Storing and using variables (different for T-SQL).

### MySQL
```sql.mysql
-- User variables (prefixed with @)
SET @user_id = 123;
SET @current_date = NOW();

SELECT * FROM orders WHERE user_id = @user_id AND created_at > @current_date;
```

### PostgreSQL
```sql.postgresql
-- Prepared statements with parameters
PREPARE get_user_orders (INTEGER, TIMESTAMP) AS
  SELECT * FROM orders WHERE user_id = $1 AND created_at > $2;

EXECUTE get_user_orders(123, NOW());
```

### Oracle
```sql.oracle
-- Bind variables (prefixed with :)
-- In SQL*Plus or other clients:
VARIABLE user_id NUMBER;
VARIABLE current_date DATE;

BEGIN
    :user_id := 123;
    :current_date := SYSDATE;
END;
/

SELECT * FROM orders WHERE user_id = :user_id AND created_at > :current_date;
```

### T-SQL (Microsoft SQL Server)
```sql.mssql
-- Local variables (prefixed with @)
DECLARE @user_id INT = 123;
DECLARE @current_date DATETIME = GETDATE();

SELECT * FROM orders WHERE user_id = @user_id AND created_at > @current_date;

-- System variables (prefixed with @@)
SELECT @@IDENTITY AS last_inserted_id;
SELECT @@ROWCOUNT AS rows_affected;
SELECT @@VERSION AS sql_server_version;
```

---

## Summary Table

| Feature | MySQL | PostgreSQL | Oracle | T-SQL |
|---------|-------|-----------|--------|-------|
| **Identifier quotes** | Backticks `` ` `` | Double quotes `"` | Double quotes `"` | Square brackets `[]` |
| **Row limiting** | `LIMIT n` | `LIMIT n` / `FETCH FIRST n` | `ROWNUM` / `FETCH FIRST n` | `TOP n` |
| **Null coalescing** | `IFNULL()` | `COALESCE()` | `NVL()` | `ISNULL()` |
| **Upsert** | `ON DUPLICATE KEY` | `ON CONFLICT DO UPDATE` | `MERGE` | `MERGE` |
| **Return updated rows** | `LAST_INSERT_ID()` | `RETURNING` | `RETURNING INTO` | `OUTPUT` |
| **String aggregation** | `GROUP_CONCAT()` | `STRING_AGG()` | `LISTAGG()` | `STRING_AGG()` |
| **Current date/time** | `NOW()` | `NOW()` | `SYSDATE` | `GETDATE()` |
| **Hierarchy queries** | Self-join | Recursive CTE | `CONNECT BY` | Recursive CTE |
| **JSON type** | `JSON` | `JSONB`/`JSON` | `CLOB` | `NVARCHAR` |
| **Variables** | `@var` (user variables) | `$1, $2` (params) | `:var` (bind vars) | `@var` (local) / `@@var` (system) |

---

**Use the markdown fence tags to see dialect-specific syntax highlighting:**
- ` ```sql.mysql ` for MySQL
- ` ```sql.postgresql ` for PostgreSQL
- ` ```sql.oracle ` for Oracle
- ` ```sql.mssql ` for T-SQL
