use diesel::sql_types::SmallInt;
use diesel_enum::DbEnum;

use crate::prelude::model_prelude::*;

#[derive(Insertable, Queryable, Clone, AsChangeset)]
pub struct Mount {
    pub owned_by: String,
    pub mount_name: String,
    pub data_img_path: String,
    pub data_mnt_path: String,
    pub repo_password: String,
    pub data_max_bytes: i64,
    pub repo_img_path: String,
    pub repo_mnt_path: String,
    pub repo_max_bytes: i64,
    pub date_created: NaiveDateTime,
    pub data_accessed: NaiveDateTime,
    pub repo_accessed: NaiveDateTime,
    pub locked: bool,
}

#[derive(Insertable, Queryable, Clone, AsChangeset)]
pub struct RepoJob {
    pub id: Uuid,
    pub snapshot_id: Option<String>,
    pub point_owned_by: String,
    pub point_name: String,
    pub job_type: JobType,
    pub job_status: JobStatus,
    pub used_bytes: i64,
    pub total_bytes: Option<i64>,
    pub errors: Vec<String>,
    pub create_date: NaiveDateTime,
    pub end_date: Option<NaiveDateTime>
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, AsExpression, FromSqlRow, DbEnum, Serialize, Deserialize)]
#[diesel(sql_type = SmallInt)]
#[diesel_enum(error_fn = NeptisError::enum_not_found)]
#[diesel_enum(error_type = NeptisError)]
pub enum JobType {
    Backup,
    Restore
}


#[derive(Debug, Clone, Copy, PartialEq, Eq, AsExpression, FromSqlRow, DbEnum, Serialize, Deserialize)]
#[diesel(sql_type = SmallInt)]
#[diesel_enum(error_fn = NeptisError::enum_not_found)]
#[diesel_enum(error_type = NeptisError)]
pub enum JobStatus {
    NotStarted,
    Running,
    Successful,
    Failed
}

impl CleanValidate for Mount {
    fn validate(mut self) -> Result<Self, ValidateError>
    where
        Self: Sized,
    {
        trim!(
            self.mount_name,
            self.owned_by,
            self.data_img_path,
            self.data_mnt_path,
            self.repo_img_path,
            self.repo_mnt_path
        );
        vreq!(self.mount_name, "You must enter a name!");
        vreq!(self.data_img_path, "You must enter a data image path!");
        vreq!(self.data_mnt_path, "You must enter a data mount path!");
        vreq!(self.repo_img_path, "You must enter a repo image path!");
        vreq!(self.repo_mnt_path, "You must enter a repo mount path!");
        vreq!(self.owned_by, "You must enter an owner!");
        vmin!(self.data_max_bytes, 0, "Max data bytes must be greater than zero!");
        vmin!(self.repo_max_bytes, 0, "Max repo bytes must be greater than zero!");
        Ok(self)
    }
}