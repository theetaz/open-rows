// SPDX-License-Identifier: Apache-2.0
//! Version/platform contracts and the DMV visibility permission each one uses.

/// Where the engine runs; Azure SQL Database scopes DMVs to the database.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Edition {
    OnPremises,
    ManagedInstance,
    Edge,
    AzureDatabase,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct Platform {
    pub(super) major: u32,
    pub(super) edition: Edition,
}

impl Platform {
    /// SQL Server 2017+ (ADR-0011 floor), Managed Instance, Edge or Azure SQL
    /// Database; Synapse and Fabric expose different DMVs.
    pub(super) fn detect(version: &str, flavour: Option<&str>) -> Option<Self> {
        let major = version.split('.').next()?.parse::<u32>().ok()?;
        let edition = match flavour {
            None => Edition::OnPremises,
            Some("Azure SQL Managed Instance") => Edition::ManagedInstance,
            Some("Azure SQL Edge") => Edition::Edge,
            Some("Azure SQL Database") => Edition::AzureDatabase,
            Some(_) => return None,
        };
        (edition != Edition::OnPremises || major >= 14).then_some(Self { major, edition })
    }

    /// Server-wide DMVs, or database-scoped ones on Azure SQL Database.
    pub(super) fn database_scoped(self) -> bool {
        self.edition == Edition::AzureDatabase
    }

    /// SQL Server 2022 (16.x) split the performance-state permission out.
    fn performance_permission(self) -> bool {
        self.major >= 16 || self.edition == Edition::AzureDatabase
    }

    /// One row: whether the login holds the state permission and, where it
    /// exists, the narrower performance-state permission.
    pub(super) fn permission_probe(self) -> String {
        let (target, state, performance) = if self.database_scoped() {
            (
                "DB_NAME(), 'DATABASE'",
                "VIEW DATABASE STATE",
                "VIEW DATABASE PERFORMANCE STATE",
            )
        } else {
            (
                "NULL, NULL",
                "VIEW SERVER STATE",
                "VIEW SERVER PERFORMANCE STATE",
            )
        };
        let narrow = if self.performance_permission() {
            format!("HAS_PERMS_BY_NAME({target}, '{performance}')")
        } else {
            "NULL".into()
        };
        format!("SELECT HAS_PERMS_BY_NAME({target}, '{state}'), {narrow}")
    }

    /// The reviewed permission a DBA would grant; monitoring never grants it.
    pub(super) fn grant(self) -> &'static str {
        match (self.database_scoped(), self.performance_permission()) {
            (true, _) => "VIEW DATABASE STATE (or VIEW DATABASE PERFORMANCE STATE)",
            (false, true) => "VIEW SERVER STATE (or VIEW SERVER PERFORMANCE STATE)",
            (false, false) => "VIEW SERVER STATE",
        }
    }

    /// Server start time as the counter epoch; Azure SQL Database has no
    /// stable instance start, so continuity there is best effort.
    pub(super) fn epoch(self) -> &'static str {
        if self.database_scoped() {
            "DECLARE @epoch varchar(30) = NULL;"
        } else {
            "DECLARE @epoch varchar(30) = (SELECT CONVERT(varchar(30), sqlserver_start_time, 126) FROM sys.dm_os_sys_info);"
        }
    }

    pub(super) fn waits(self) -> &'static str {
        if self.database_scoped() {
            "sys.dm_db_wait_stats"
        } else {
            "sys.dm_os_wait_stats"
        }
    }
}
