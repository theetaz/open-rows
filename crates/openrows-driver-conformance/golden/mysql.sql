-- SPDX-License-Identifier: Apache-2.0
-- The collation is explicit because each server version defaults to its own.
CREATE TABLE orc_golden_customers (
    id INT NOT NULL AUTO_INCREMENT,
    email VARCHAR(80) NOT NULL,
    nick VARCHAR(20) DEFAULT 'anon' COMMENT 'Display name',
    PRIMARY KEY (id),
    UNIQUE KEY customers_email (email)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_bin COMMENT='Who buys';
CREATE TABLE orc_golden_orders (
    id BIGINT NOT NULL AUTO_INCREMENT PRIMARY KEY,
    customer_id INT NOT NULL,
    price DECIMAL(10, 2),
    placed DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    total DECIMAL(12, 2) GENERATED ALWAYS AS (price * 2) STORED,
    status ENUM('new', 'paid') NOT NULL DEFAULT 'new',
    CONSTRAINT orders_customer FOREIGN KEY (customer_id)
        REFERENCES orc_golden_customers (id) ON DELETE CASCADE,
    CONSTRAINT orders_price_positive CHECK (price > 0),
    KEY orders_placed (placed)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_bin;
CREATE VIEW orc_golden_order_totals AS
    SELECT customer_id, SUM(price) AS spent FROM orc_golden_orders GROUP BY customer_id;
CREATE TRIGGER orc_golden_orders_touch BEFORE INSERT ON orc_golden_orders
    FOR EACH ROW SET NEW.price = NEW.price;
CREATE FUNCTION orc_golden_double(v INT) RETURNS INT DETERMINISTIC RETURN v * 2;
-- teardown
DROP FUNCTION IF EXISTS orc_golden_double;
DROP VIEW IF EXISTS orc_golden_order_totals;
DROP TABLE IF EXISTS orc_golden_orders;
DROP TABLE IF EXISTS orc_golden_customers;
