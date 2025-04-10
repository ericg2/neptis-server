use crossbeam_channel::unbounded;
use crossbeam_channel::{Receiver, Sender};
use diesel::{Connection, PgConnection, RunQueryDsl};
use rustic_backend::BackendOptions;
use rustic_core::repofile::SnapshotFile;
use rustic_core::{
    BackupOptions, ConfigOptions, KeyOptions, LocalDestination, LsOptions, NoProgress, PathList,
    Progress, ProgressBars, Repository, RepositoryOptions, RestoreOptions, RestorePlan,
    RusticResult, SnapshotOptions,
};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::Duration;
use std::{env, thread};
use uuid::Uuid;

use super::models::*;
use crate::api::errors::NeptisError;
use crate::diesel::QueryDsl;
use crate::utc_now;

pub type ProgressType = (Uuid, SendUpdate);

pub struct DbProgressBars {
    job_id: Uuid,
    tx: Sender<ProgressType>,
}

pub enum SendUpdate {
    Increment(u64),
    LengthSet(u64),
    TitleSet(String),
}

#[derive(Clone)]
pub struct DbProgress {
    tx: Option<Sender<ProgressType>>,
    prefix: Option<String>,
    is_hidden: bool,
    job_id: Uuid,
}

impl Progress for DbProgress {
    fn is_hidden(&self) -> bool {
        self.is_hidden
    }

    fn finish(&self) {
        //self.sender.send(SendUpdate::Finished).unwrap();
    }
    fn inc(&self, inc: u64) {
        if let Some(ref tx) = self.tx {
            tx.send((self.job_id.clone(), SendUpdate::Increment(inc)))
                .unwrap();
        }
    }
    fn set_length(&self, len: u64) {
        if let Some(ref tx) = self.tx {
            tx.send((self.job_id.clone(), SendUpdate::LengthSet(len)))
                .unwrap();
        }
    }
    fn set_title(&self, title: &'static str) {
        if let Some(ref tx) = self.tx {
            tx.send((self.job_id.clone(), SendUpdate::TitleSet(title.to_string())))
                .unwrap();
        }
    }
}

impl DbProgressBars {
    pub fn new(job_id: Uuid, tx: Sender<ProgressType>) -> DbProgressBars {
        DbProgressBars { job_id, tx }
    }
}

impl ProgressBars for DbProgressBars {
    type P = DbProgress;
    fn progress_bytes(&self, prefix: impl Into<std::borrow::Cow<'static, str>>) -> Self::P {
        DbProgress {
            job_id: self.job_id,
            tx: Some(self.tx.clone()),
            prefix: Some(prefix.into().to_string()),
            is_hidden: false,
        }
    }
    fn progress_counter(&self, prefix: impl Into<std::borrow::Cow<'static, str>>) -> Self::P {
        DbProgress {
            job_id: self.job_id,
            tx: None,
            prefix: Some(prefix.into().to_string()),
            is_hidden: false,
        }
    }
    fn progress_spinner(&self, prefix: impl Into<std::borrow::Cow<'static, str>>) -> Self::P {
        DbProgress {
            job_id: self.job_id,
            tx: None,
            prefix: Some(prefix.into().to_string()),
            is_hidden: false,
        }
    }
    fn progress_hidden(&self) -> Self::P {
        DbProgress {
            job_id: self.job_id,
            tx: None,
            prefix: None,
            is_hidden: true,
        }
    }
}

pub struct NonBlockingRustic {
    tx: Sender<ProgressType>,
    u_thread: JoinHandle<()>,
}

pub struct JobLaunchInfo {
    pub point_owned_by: String,
    pub point_name: String,
    pub repo_path: String,
    pub repo_pass: String,
}

impl NonBlockingRustic {
    pub fn new() -> NonBlockingRustic {
        dotenvy::dotenv().expect("Failed to load environment variable!".into());

        let database_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");

        // Attempt to connect multiple times to ensure it works...
        let mut t_conn: Option<PgConnection> = None;
        const MAX_ATTEMPTS: i32 = 5;

        for i in 0..=MAX_ATTEMPTS {
            println!("Attempting to connect to DB ({}/{})", i, MAX_ATTEMPTS);
            if let Some(c) = PgConnection::establish(&database_url).ok() {
                println!("Connected to DB!");
                t_conn = Some(c);
                break;
            }
            thread::sleep(Duration::from_secs(2));
        }

        let mut conn = t_conn.expect("Expected the DB to connect!");
        let (tx, rx) = unbounded::<ProgressType>();

        let u_thread = thread::spawn(move || {
            Self::handle_progress_update(&mut conn, rx.clone());
        });

        NonBlockingRustic { tx, u_thread }
    }

    pub fn handle_progress_update(conn: &mut PgConnection, rx: Receiver<ProgressType>) {
        loop {
            match rx.recv() {
                Ok((job_id, r_update)) => {
                    let _: Result<usize, NeptisError> = (|| {
                        use crate::schema::repo_jobs::dsl::*;
                        let mut job: RepoJob = repo_jobs
                            .find(job_id)
                            .get_result(conn)
                            .expect("Expected Job ID to be valid in `DbProgressBars`");

                        match r_update {
                            SendUpdate::Increment(inc) => job.used_bytes += inc as i64,
                            SendUpdate::LengthSet(len) => job.total_bytes = Some(len as i64),
                            _ => return Ok(0),
                        }

                        return Ok(diesel::update(repo_jobs).set(&job).execute(conn)?);
                    })();
                }
                Err(_) => break, // terminate the loop
            }
            thread::yield_now();
        }
    }

    pub fn start_full_restore(
        &self,
        launch_info: &JobLaunchInfo,
        snap_path: &str,
        abs_dest_path: &str,
        r_opts: RestoreOptions,
        dry_run: bool,
    ) -> Result<Uuid, NeptisError> {
        let backends = BackendOptions::default()
            .repository(launch_info.repo_path.as_str())
            .to_backends()?;
        let repo_opts = RepositoryOptions::default().password(launch_info.repo_pass.as_str());
        let job_id = Uuid::new_v4();

        let p_bar = DbProgressBars::new(job_id, self.tx.clone());

        // Finally, we need to spawn a new thread to handle everything.
        let s_job = RepoJob {
            id: job_id,
            snapshot_id: Some(snap_path.to_string()),
            point_owned_by: launch_info.point_owned_by.clone(),
            point_name: launch_info.point_name.clone(),
            job_type: JobType::Restore,
            job_status: JobStatus::Running,
            used_bytes: 0,
            total_bytes: None,
            errors: vec![],
            create_date: utc_now!(),
            end_date: None,
        };
        {
            use crate::schema::repo_jobs::dsl::*;
            let mut conn = PgConnection::establish(
                &env::var("DATABASE_URL").expect("DATABASE_URL must be set"),
            )?;
            diesel::insert_into(repo_jobs)
                .values(s_job)
                .execute(&mut conn)?;
            let s_path = snap_path.to_owned();
            let d_path = abs_dest_path.to_owned();
            thread::spawn(move || {
                Self::finish_restore(
                    job_id,
                    (|| {
                        let repo = Repository::new_with_progress(&repo_opts, &backends, p_bar)?
                            .open()?
                            .to_indexed()?;

                        let node = repo.node_from_snapshot_path(s_path.as_str(), |_| true)?;
                        let ls = repo.ls(&node, &LsOptions::default())?;
                        let dest = LocalDestination::new(d_path.as_str(), true, !node.is_dir())?;
                        let plan = repo.prepare_restore(&r_opts, ls.clone(), &dest, dry_run)?;

                        repo.restore(plan, &r_opts, ls, &dest)
                    })(),
                    &mut conn,
                );
            });
        }
        Ok(job_id)
    }

    pub fn start_backup(
        &self,
        launch_info: &JobLaunchInfo,
        source: PathList,
        s_opts: SnapshotOptions,
        b_opts: BackupOptions,
    ) -> Result<Uuid, NeptisError> {
        let backends = BackendOptions::default()
            .repository(launch_info.repo_path.as_str())
            .to_backends()?;
        let repo_opts = RepositoryOptions::default().password(launch_info.repo_pass.as_str());
        let job_id = Uuid::new_v4();
        let p_bar = DbProgressBars::new(job_id, self.tx.clone());

        let s_job = RepoJob {
            id: job_id,
            snapshot_id: None,
            point_owned_by: launch_info.point_owned_by.clone(),
            point_name: launch_info.point_name.clone(),
            job_type: JobType::Backup,
            job_status: JobStatus::Running,
            used_bytes: 0,
            total_bytes: None,
            errors: vec![],
            create_date: utc_now!(),
            end_date: None,
        };
        {
            use crate::schema::repo_jobs::dsl::*;
            let mut conn = PgConnection::establish(
                &env::var("DATABASE_URL").expect("DATABASE_URL must be set"),
            )?;
            diesel::insert_into(repo_jobs)
                .values(s_job)
                .execute(&mut conn)?;
            thread::spawn(move || {
                Self::finish_backup(
                    job_id,
                    (|| {
                        let repo = Repository::new_with_progress(&repo_opts, &backends, p_bar)?
                            .open()?
                            .to_indexed()?;

                        // Finally, we need to spawn a new thread to handle everything.
                        let s_file = s_opts.to_snapshot()?;
                        repo.backup(&b_opts, &source, s_file)
                    })(),
                    &mut conn,
                );
            });
        }
        Ok(job_id)
    }

    fn finish_restore(job_id: Uuid, ret: RusticResult<()>, conn: &mut PgConnection) {
        use crate::schema::repo_jobs::dsl::*;
        let mut f_job: RepoJob = repo_jobs
            .find(job_id)
            .get_result(conn)
            .expect("Job ID is supposed to be valid at this point!".into());

        f_job.end_date = Some(utc_now!());
        match ret {
            Ok(_) => {
                f_job.job_status = JobStatus::Successful;
            }
            Err(e) => {
                f_job.job_status = JobStatus::Failed;
                f_job.errors.push(e.to_string());
            }
        }
        let _ = diesel::update(repo_jobs)
            .set(&f_job)
            .execute(conn)
            .expect("Failed to access DB!".into());
    }

    fn finish_backup(job_id: Uuid, ret: RusticResult<SnapshotFile>, conn: &mut PgConnection) {
        use crate::schema::repo_jobs::dsl::*;
        let mut f_job: RepoJob = repo_jobs
            .find(job_id)
            .get_result(conn)
            .expect("Job ID is supposed to be valid at this point!".into());

        f_job.end_date = Some(utc_now!());
        match ret {
            Ok(x) => {
                f_job.job_status = JobStatus::Successful;
                f_job.snapshot_id = Some(x.id.to_string());
                if let Some(summary) = x.summary {
                    f_job.used_bytes = summary.total_bytes_processed as i64;
                }
            }
            Err(e) => {
                f_job.job_status = JobStatus::Failed;
                f_job.errors.push(e.to_string());
            }
        }
        let _ = diesel::update(repo_jobs)
            .set(&f_job)
            .execute(conn)
            .expect("Failed to access DB!".into());
    }
}
