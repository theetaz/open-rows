-- SPDX-License-Identifier: Apache-2.0
CREATE TABLE orc_golden_customers (
    id INT NOT NULL AUTO_INCREMENT,
    email VARCHAR(80) NOT NULL,
    nick VARCHAR(20) DEFAULT 'anon' COMMENT 'Display name',
    PRIMARY KEY (id),
    UNIQUE KEY customers_email (email)
) COMMENT='Who buys';
CREATE TABLE orc_golden_orders (
    id BIGINT NOT NULL AUTO_INCREMENT PRIMARY KEY,
    customer_id INT NOT NULL,
    price DECIMAL(10, 2),
    placed DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    status ENUM('new', 'paid') NOT NULL DEFAULT 'new',
    CONSTRAINT orders_customer FOREIGN KEY (customer_id)
        REFERENCES orc_golden_customers (id) ON DELETE CASCADE,
    KEY orders_placed (placed)
);
CREATE VIEW orc_golden_order_totals AS
    SELECT customer_id, SUM(price) AS spent FROM orc_golden_orders GROUP BY customer_id;
-- teardown
DROP VIEW IF EXISTS orc_golden_order_totals;
DROP TABLE IF EXISTS orc_golden_orders;
DROP TABLE IF EXISTS orc_golden_customers;
