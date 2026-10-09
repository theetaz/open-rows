-- SPDX-License-Identifier: Apache-2.0
SELECT Role FROM mysql.roles_mapping WHERE User = ? AND Host = ? ORDER BY Role
