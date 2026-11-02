-- SPDX-License-Identifier: Apache-2.0
CREATE TABLE orc_golden_customers (
    id INTEGER PRIMARY KEY,
    email TEXT NOT NULL UNIQUE,
    nick TEXT DEFAULT 'anon'
);
CREATE TABLE orc_golden_orders (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    customer_id INTEGER NOT NULL REFERENCES orc_golden_customers (id) ON DELETE CASCADE,
    price NUMERIC CONSTRAINT orders_price_positive CHECK (price > 0),
    placed TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    total NUMERIC GENERATED ALWAYS AS (price * 2) STORED
);
CREATE INDEX orc_golden_orders_placed ON orc_golden_orders (placed DESC) WHERE price > 1;
CREATE VIEW orc_golden_order_totals AS
    SELECT customer_id, SUM(price) AS spent FROM orc_golden_orders GROUP BY customer_id;
CREATE TRIGGER orc_golden_orders_touch BEFORE INSERT ON orc_golden_orders
    BEGIN SELECT 1; END;
-- teardown
DROP VIEW IF EXISTS orc_golden_order_totals;
DROP TABLE IF EXISTS orc_golden_orders;
DROP TABLE IF EXISTS orc_golden_customers;
