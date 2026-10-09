-- SPDX-License-Identifier: Apache-2.0
-- Public account attributes only; authentication_string is never read.
SELECT User, Host, account_locked, password_expired FROM mysql.user ORDER BY User, Host
