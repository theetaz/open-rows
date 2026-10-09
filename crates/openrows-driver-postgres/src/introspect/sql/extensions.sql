-- SPDX-License-Identifier: Apache-2.0
SELECT e.extname::text, e.extversion::text, a.default_version::text,
    n.nspname::text, COALESCE(obj_description(e.oid, 'pg_extension'), a.comment)::text
FROM pg_extension e JOIN pg_namespace n ON n.oid = e.extnamespace
LEFT JOIN pg_available_extensions a ON a.name = e.extname ORDER BY e.extname
