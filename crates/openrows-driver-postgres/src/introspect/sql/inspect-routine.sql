-- SPDX-License-Identifier: Apache-2.0
SELECT (quote_ident(n.nspname) || '.' || quote_ident(p.proname) || '(' || pg_get_function_identity_arguments(p.oid) || ')')::text,
    pg_get_function_arguments(p.oid)::text, pg_get_function_result(p.oid)::text,
    l.lanname::text, CASE p.provolatile WHEN 'i' THEN 'immutable' WHEN 's' THEN 'stable' ELSE 'volatile' END::text,
    p.prosecdef::text
FROM pg_proc p JOIN pg_namespace n ON n.oid = p.pronamespace
JOIN pg_language l ON l.oid = p.prolang
WHERE n.nspname = $1 AND p.proname = $2
    AND (p.prokind::text = $3 OR ($3 = 'f' AND p.prokind = 'w'))
    AND ($4::text IS NULL OR pg_get_function_identity_arguments(p.oid) = $4)
ORDER BY p.oid LIMIT 1
