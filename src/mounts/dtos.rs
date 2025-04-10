use std::{fs::Metadata, io::SeekFrom, time::{SystemTime, UNIX_EPOCH}};

use crate::{prelude::model_prelude::*};
use super::models::{JobStatus, JobType, Mount, RepoJob};

#[derive(Serialize, Deserialize)]
pub struct MountDto {
    pub name: String,
    pub owned_by: String,
    pub data_max_bytes: i64,
    pub repo_max_bytes: i64,
    pub data_used_bytes: Option<i64>,
    pub repo_used_bytes: Option<i64>,
    pub date_created: NaiveDateTime,
    pub data_accessed: NaiveDateTime,
    pub repo_accessed: NaiveDateTime
}

#[derive(Serialize, Deserialize)]
pub struct RepoJobDto {
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

#[derive(Serialize, Deserialize, Clone)]
pub struct NodeDto {
    pub path: String,
    pub atime: SystemTime,
    pub ctime: SystemTime,
    pub mtime: SystemTime,
    pub is_dir: bool,
    pub bytes: u64
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
/// Possible input arguments for atime & mtime, which can either be set to a specified time,
/// or to the current time
pub enum TimeOrNow {
    /// Specific time provided
    SpecificTime(SystemTime),
    /// Current time
    Now,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct SetAttrRequest {
    /// File size in bytes
    pub size: Option<u64>,
    /// Last access time
    pub atime: Option<TimeOrNow>,
    /// Last modification time
    pub mtime: Option<TimeOrNow>,
}
#[derive(Serialize, Deserialize, Clone)]
pub struct PutForFileApi {
    pub path: String,
    pub base64: Option<String>,
    pub new_path: Option<String>,
    pub offset: Option<SeekPos>,
    pub attr: Option<SetAttrRequest>
}

#[derive(Serialize, Deserialize, Clone)]
pub struct PutForXattrApi {
    pub path: String,
    pub key: String,
    pub base64: String
}

#[derive(Serialize, Deserialize, Clone)]
pub struct DeleteForXattrApi {
    pub path: String,
    pub key: String
}

#[derive(Serialize, Deserialize)]
pub struct PostForFileApi {
    pub path: String,
    pub is_dir: bool,
    pub base64: Option<String>,
    pub offset: Option<SeekPos>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SeekPos {
    Start(u64),
    End(i64),
    Current(i64),
}

impl From<SeekPos> for std::io::SeekFrom {
    fn from(pos: SeekPos) -> Self {
        match pos {
            SeekPos::Start(n) => std::io::SeekFrom::Start(n),
            SeekPos::End(n) => std::io::SeekFrom::End(n),
            SeekPos::Current(n) => std::io::SeekFrom::Current(n),
        }
    }
}

#[derive(Serialize, Deserialize)]
pub struct GetForDumpApi {
    pub path: String,
    pub offset: SeekPos,
    pub size: usize
}

#[derive(Serialize, Deserialize)]
pub struct PutForMountApi {
    pub data_bytes: i64,
    pub repo_bytes: i64
}

#[derive(Serialize, Deserialize)]
pub struct PostForBackupApi {
    pub point_user: String,
    pub point_name: String,
    pub tags: Option<Vec<String>>,
    pub dry_run: bool
}

impl NodeDto {
    fn safe_time(t: SystemTime) -> SystemTime {
        if t < UNIX_EPOCH {
            UNIX_EPOCH
        } else {
            t
        }
    }
    
    pub fn from_metadata(path: &str, data: Metadata) -> Self {
        NodeDto {
            path: path.to_string(),
            atime: Self::safe_time(data.accessed().ok().unwrap_or(SystemTime::now())),
            ctime: Self::safe_time(data.created().ok().unwrap_or(SystemTime::now())),
            mtime: Self::safe_time(data.modified().ok().unwrap_or(SystemTime::now())),
            is_dir: data.is_dir(),
            bytes: data.len()
        }
    }
}

bind_dto!(Mount, MountDto);
bind_dto!(RepoJob, RepoJobDto);