-- SPDX-License-Identifier: Apache-2.0
-- pg_sequences reads last_value without invoking nextval; NULL means it is
-- uncalled or the current user has no USAGE/SELECT permission.
SELECT data_type::text, start_value::text, increment_by::text, min_value::text,
    max_value::text, cycle::text, cache_size::text, last_value::text
FROM pg_sequences WHERE schemaname = $1 AND sequencename = $2
