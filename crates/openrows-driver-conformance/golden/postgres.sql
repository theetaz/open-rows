-- SPDX-License-Identifier: Apache-2.0
CREATE SCHEMA orc_golden;
COMMENT ON SCHEMA orc_golden IS 'Golden fixture';
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
    total numeric GENERATED ALWAYS AS (price * 2) STORED,
    tags text[]
);
CREATE INDEX orders_placed_idx ON orc_golden.orders (placed DESC) WHERE price > 1;
-- A join, because servers before 16 qualify the columns of a one-table view.
CREATE VIEW orc_golden.order_totals AS
    SELECT c.id, sum(o.price) AS spent
    FROM orc_golden.customers AS c JOIN orc_golden.orders AS o ON o.customer_id = c.id
    GROUP BY c.id;
CREATE FUNCTION orc_golden.touch() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RETURN NEW; END $$;
CREATE TRIGGER orders_touch BEFORE INSERT ON orc_golden.orders
    FOR EACH ROW EXECUTE FUNCTION orc_golden.touch();
-- teardown
DROP SCHEMA IF EXISTS orc_golden CASCADE;
