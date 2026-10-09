-- SPDX-License-Identifier: Apache-2.0
-- Public account attributes only; the credential-bearing Priv document is
-- never returned. MariaDB's mysql.user is a compatibility view.
SELECT u.User, u.Host, JSON_VALUE(p.Priv, '$.account_locked'), u.password_expired, u.is_role
FROM mysql.user u JOIN mysql.global_priv p ON p.User = u.User AND p.Host = u.Host
ORDER BY u.User, u.Host
