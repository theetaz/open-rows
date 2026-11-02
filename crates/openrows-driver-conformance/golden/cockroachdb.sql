-- SPDX-License-Identifier: Apache-2.0
CREATE SCHEMA orc_golden;
CREATE TYPE orc_golden.mood AS ENUM ('sad', 'happy');
CREATE SEQUENCE orc_golden.ticket_seq;
CREATE TABLE orc_golden.customers (
    id integer PRIMARY KEY,
    email varchar(80) NOT NULL UNIQUE,
    nick text DEFAULT 'anon',
    feeling orc_golden.mood
);
COMMENT ON TABLE orc_golden.customers IS 'Who buys';
COMMENT ON COLUMN orc_golden.customers.nick IS 'Display name';
CREATE TABLE orc_golden.orders (
    id bigint GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    customer_id integer NOT NULL REFERENCES orc_golden.customers (id) ON DELETE CASCADE,
    price numeric(10, 2) CONSTRAINT orders_price_positive CHECK (price > 0),
    placed timestamp with time zone NOT NULL DEFAULT now(),
    tags text[]
);
CREATE INDEX orders_placed_idx ON orc_golden.orders (placed DESC) WHERE price > 1;
CREATE VIEW orc_golden.order_totals AS
    SELECT customer_id, sum(price) AS spent FROM orc_golden.orders GROUP BY customer_id;
-- teardown
DROP SCHEMA IF EXISTS orc_golden CASCADE;
