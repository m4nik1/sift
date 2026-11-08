CREATE TABLE customers (
  id INTEGER PRIMARY KEY,
  name TEXT NOT NULL,
  email TEXT UNIQUE,
  city TEXT,
  joined_at TEXT NOT NULL,
  avatar BLOB
);
CREATE TABLE products (
  id INTEGER PRIMARY KEY,
  sku TEXT NOT NULL UNIQUE,
  title TEXT NOT NULL,
  price REAL NOT NULL,
  stock INTEGER DEFAULT 0
);
CREATE TABLE orders (
  id INTEGER PRIMARY KEY,
  customer_id INTEGER NOT NULL REFERENCES customers(id),
  product_id INTEGER NOT NULL REFERENCES products(id),
  quantity INTEGER NOT NULL,
  total REAL NOT NULL,
  note TEXT,
  placed_at TEXT NOT NULL
);
CREATE INDEX orders_by_customer ON orders(customer_id);
CREATE INDEX orders_by_date ON orders(placed_at);

WITH RECURSIVE n(i) AS (SELECT 1 UNION ALL SELECT i + 1 FROM n WHERE i < 240)
INSERT INTO customers (id, name, email, city, joined_at, avatar)
SELECT i,
  (SELECT value FROM json_each('["Ada","Grace","Linus","Margaret","Dennis","Barbara","Ken","Frances","Edsger","Radia","Alan","Hedy"]') WHERE key = i % 12)
    || ' ' ||
  (SELECT value FROM json_each('["Okafor","Lindqvist","Moreau","Tanaka","Novak","Silva","Haddad","Kowalski","Brennan","Iyer"]') WHERE key = (i * 7) % 10),
  CASE WHEN i % 9 = 0 THEN NULL ELSE 'user' || i || '@example.com' END,
  CASE WHEN i % 7 = 0 THEN NULL ELSE (SELECT value FROM json_each('["Lisbon","Osaka","Toronto","Nairobi","Oslo","Austin","Pune","Lyon"]') WHERE key = i % 8) END,
  date('2023-01-01', '+' || (i * 3) || ' days'),
  CASE WHEN i % 4 = 0 THEN randomblob(64 + i) ELSE NULL END
FROM n;

WITH RECURSIVE n(i) AS (SELECT 1 UNION ALL SELECT i + 1 FROM n WHERE i < 36)
INSERT INTO products (id, sku, title, price, stock)
SELECT i, printf('SKU-%04d', i * 37),
  (SELECT value FROM json_each('["Walnut desk","Linen throw","Brass lamp","Oak stool","Wool rug","Clay vase"]') WHERE key = i % 6)
    || ' ' || (SELECT value FROM json_each('["S","M","L","XL","Mini","Pro"]') WHERE key = (i / 6) % 6),
  round(12.5 + (i * 13.37) % 240, 2),
  (i * 17) % 90
FROM n;

WITH RECURSIVE n(i) AS (SELECT 1 UNION ALL SELECT i + 1 FROM n WHERE i < 4000)
INSERT INTO orders (id, customer_id, product_id, quantity, total, note, placed_at)
SELECT i, 1 + (i * 31) % 240, 1 + (i * 7) % 36, 1 + i % 4,
  round((1 + i % 4) * (12.5 + ((1 + (i * 7) % 36) * 13.37) % 240), 2),
  CASE i % 11 WHEN 0 THEN 'Gift wrap, please' WHEN 3 THEN 'Leave at the side door' || char(10) || 'Ring twice' ELSE NULL END,
  datetime('2024-01-01', '+' || (i * 97) || ' minutes')
FROM n;

CREATE VIEW top_customers AS
SELECT c.id, c.name, c.city, count(o.id) AS orders, round(sum(o.total), 2) AS spent
FROM customers c JOIN orders o ON o.customer_id = c.id
GROUP BY c.id
ORDER BY spent DESC;
