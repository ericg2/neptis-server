use super::dtos::*;
use super::models::*;
use super::rustic_async::NonBlockingRustic;
use crate::api::traits::WebDtoFrom;
use crate::mounts::rustic_async::JobLaunchInfo;
use crate::prelude::action_prelude::*;
use base64::prelude::*;
use chrono::TimeZone;
use diesel::result;
use nix::sys::time::TimeSpec;
use passwords::PasswordGenerator;
use rustic_backend::BackendOptions;
use rustic_core::BackupOptions;
use rustic_core::PathList;
use rustic_core::SnapshotOptions;
use rustic_core::{ConfigOptions, KeyOptions, Repository, RepositoryOptions};
use serde::Serialize;
use std::fs;
use std::fs::File;
use std::fs::OpenOptions;
use std::io::Read;
use std::io::Seek;
use std::io::SeekFrom;
use std::io::Write;
use std::path::Path;
use std::thread;
use std::time::Duration;

pub struct MountStats {
    pub path: String,
    pub b_total: usize,
    pub b_used: usize,
    pub b_avail: usize,
}

fn get_img_block_size(img_path: &str) -> Result<usize, NeptisError> {
    (|| {
        cmd!("tune2fs -l {} | grep 'Block size'", img_path)?
            .replace("Block size:", "")
            .trim()
            .parse::<usize>()
            .ok()
    })()
    .ok_or(NeptisError::InternalError(
        "Failed to determine block size of FS".into(),
    ))
}

fn get_system_info(mount_path: &str) -> Result<MountStats, NeptisError> {
    (|| {
        if mount_path.is_empty() {
            return None;
        }
        let res = cmd!("df {} -B1", mount_path)?;
        /*
           Filesystem        1B-blocks         Used    Available Use% Mounted on
           /dev/nvme0n1p5 433992540160 255224807424 156646924288  62% /
        */
        let spl = res
            .split_terminator("\n")
            .map(|x| x.to_string())
            .collect::<Vec<_>>()
            .get(1)?
            .split_whitespace()
            .map(|x| x.trim().to_string())
            .collect::<Vec<_>>();

        let b_total = spl.get(1)?.parse::<usize>().ok()?;
        let b_used = spl.get(2)?.parse::<usize>().ok()?;
        let b_avail = spl.get(3)?.parse::<usize>().ok()?;

        // Pull the block size as well.
        Some(MountStats {
            path: mount_path.to_string(),
            b_total,
            b_used,
            b_avail,
        })
    })()
    .ok_or(NeptisError::InternalError(
        "Failed to get system info!".into(),
    ))
}

fn ensure_single_limit(auth_user: &User, bytes: usize, is_data: bool) -> Result<(), NeptisError> {
    if is_data {
        if auth_user.max_data_bytes as usize <= bytes {
            return Ok(());
        }
    } else {
        if auth_user.max_snapshot_bytes as usize <= bytes {
            return Ok(());
        }
    }
    Err(NeptisError::BadRequest(
        "Not enough space on your account!".into(),
    ))
}

fn ensure_user_limit(auth_user: &User, d_total: usize, r_total: usize) -> Result<(), NeptisError> {
    match d_total <= auth_user.max_data_bytes as usize
        && r_total <= auth_user.max_snapshot_bytes as usize
    {
        true => Ok(()),
        false => Err(NeptisError::BadRequest(
            "Not enough space on your account!".into(),
        )),
    }
}

// Check to see if the point is mounted or not.
fn raw_mnt_check(img_path: &str, mnt_path: &str) -> Option<()> {
    for line in cmd!("mount -l")?.split_terminator("\n").into_iter() {
        if line.contains(format!("{} on {} type ext4", img_path, mnt_path).as_str()) {
            return Some(());
        }
    }
    None
}

fn has_any_entries<P: AsRef<Path>>(dir: P) -> Option<()> {
    match fs::read_dir(dir) {
        Ok(mut entries) => {
            if entries.next().is_some() {
                Some(())
            } else {
                None
            }
        }
        Err(_) => None,
    }
}

fn ensure_point_mounted(point: &Mount, use_repo: bool) -> Result<(), NeptisError> {
    if point.data_img_path.is_empty()
        || point.data_mnt_path.is_empty()
        || point.repo_img_path.is_empty()
        || point.data_img_path.is_empty()
    {
        return Err(NeptisError::BadRequest(
            "Image / Mount path(s) are blank!".into(),
        ));
    }

    // We need to attempt to create the directories if not valid.
    if !fs::exists(point.data_img_path.as_str())? || !fs::exists(point.repo_img_path.as_str())? {
        return Err(NeptisError::InternalError("Point is corrupted".into()));
    }

    let repo_dir = format!("{}/repo", point.repo_mnt_path.as_str());
    let repo_dir_mnt = format!("{}/repo-mnt", point.repo_mnt_path.as_str());

    for path in [point.data_mnt_path.as_str(), point.repo_mnt_path.as_str()] {
        if !fs::exists(path)? {
            fs::create_dir_all(path)?;
        }
    }

    for (img_path, mnt_path) in [
        (point.data_img_path.as_str(), point.data_mnt_path.as_str()),
        (point.repo_img_path.as_str(), point.repo_mnt_path.as_str()),
    ] {
        if raw_mnt_check(img_path, mnt_path).is_none() {
            // The point is not mounted - we need to mount it!
            (|| {
                cmd!("mount -o rw,sync,loop {0} {1}", img_path, mnt_path)?;
                cmd!("chmod 777 {}", mnt_path)?;
                cmd!("chmod 777 {}", img_path)?;
                raw_mnt_check(img_path, mnt_path)
            })()
            .ok_or(NeptisError::InternalError("Failed to mount point!".into()))?;
        }
    }

    // Make sure everything is mounted first!
    if use_repo && !fs::exists(repo_dir.as_str())? {
        return Err(NeptisError::InternalError("Repository is corrupted".into()));
    }
    if use_repo && !fs::exists(repo_dir_mnt.as_str())? {
        fs::create_dir_all(repo_dir_mnt.as_str())?; // we can do this
    }

    // Finally: we need to ensure the repository is mounted.
    if use_repo {
        match has_any_entries(repo_dir_mnt.as_str()) {
            Some(_) => Some(()),
            None => {
                let repo_pass = point.repo_password.to_string();
                let rd_mnt = repo_dir_mnt.clone();
                thread::spawn(move || {
                    cmd!(
                        "export RESTIC_PASSWORD='{0}' && restic mount -r {1} {2} --allow-other",
                        repo_pass.as_str(),
                        repo_dir.as_str(),
                        rd_mnt
                    )
                });
                thread::sleep(Duration::from_secs(2));
                has_any_entries(repo_dir_mnt.as_str())
            }
        }
        .ok_or(NeptisError::InternalError(
            "Failed to mount the repository!".into(),
        ))?;
    }
    Ok(())
}

impl WebDtoFrom<RepoJob> for RepoJobDto {
    fn try_to_dto(_: &User, item: RepoJob) -> Result<Self, NeptisError>
    where
        Self: Serialize + Sized,
    {
        Ok(Self {
            id: item.id,
            snapshot_id: item.snapshot_id.clone(),
            point_owned_by: item.point_owned_by.clone(),
            point_name: item.point_name.clone(),
            job_type: item.job_type.clone(),
            job_status: item.job_status.clone(),
            used_bytes: item.used_bytes.clone(),
            total_bytes: item.total_bytes.clone(),
            errors: item.errors.clone(),
            create_date: item.create_date.clone(),
            end_date: item.end_date.clone(),
        })
    }
}
impl WebDtoFrom<Mount> for MountDto {
    fn try_to_dto(
        auth_user: &crate::prelude::route_prelude::User,
        item: Mount,
    ) -> Result<Self, NeptisError>
    where
        Self: Serialize + Sized,
    {
        if !auth_user.is_admin && auth_user.user_name != item.owned_by {
            return Err(NeptisError::Unauthorized("You do not have access!".into()));
        }
        let mut d_used: Option<i64> = None;
        let mut r_used: Option<i64> = None;
        if ensure_point_mounted(&item, true).is_ok() {
            d_used = Some(get_system_info(item.data_mnt_path.as_str())?.b_used as i64);
            r_used = Some(get_system_info(item.repo_mnt_path.as_str())?.b_used as i64);
        }
        // Attempt to pull the information - only if it is mounted.
        Ok(MountDto {
            name: item.mount_name,
            owned_by: item.owned_by,
            data_max_bytes: item.data_max_bytes,
            repo_max_bytes: item.repo_max_bytes,
            data_used_bytes: d_used,
            repo_used_bytes: r_used,
            date_created: item.date_created,
            repo_accessed: item.repo_accessed,
            data_accessed: item.data_accessed,
        })
    }
}

#[action(Vec<Mount>)]
pub async fn get_all_mounts_for_user() -> Result<Vec<MountDto>, NeptisError> {
    use crate::schema::mounts::dsl::*;
    Ok(mounts
        .filter(owned_by.eq(auth_user.user_name.as_str()))
        .get_results(conn)
        .await?)
}

#[action(Mount)]
pub async fn get_one_mount(p_name: &str) -> Result<MountDto, NeptisError> {
    use crate::schema::mounts::dsl::*;
    Ok(mounts
        .find((auth_user.user_name.clone(), p_name.to_string()))
        .get_result(conn)
        .await?)
}

#[action]
pub async fn delete_one_mount(p_name: &str) -> Result<usize, NeptisError> {
    use crate::schema::mounts::dsl::*;
    // First, delete the mount from the database to prevent corruption.
    let p_mount: Mount = mounts
        .find((auth_user.user_name.clone(), p_name.to_string()))
        .get_result(conn)
        .await?;
    if p_mount.locked {
        return Err(NeptisError::InternalError(
            "The point is currently locked".into(),
        ));
    }

    if diesel::delete(mounts)
        .filter(
            owned_by
                .eq(auth_user.user_name.clone())
                .and(mount_name.eq(p_name.to_string())),
        )
        .execute(conn)
        .await?
        <= 0
    {
        return Err(NeptisError::BadRequest("The point does not exist!".into()));
    }
    // Delete the files and make it work!
    for (i_path, m_path) in [
        (
            p_mount.data_img_path.as_str(),
            p_mount.data_mnt_path.as_str(),
        ),
        (
            p_mount.repo_img_path.as_str(),
            p_mount.repo_mnt_path.as_str(),
        ),
    ] {
        if raw_mnt_check(i_path, m_path).is_some() {
            cmd!("umount {}", m_path).ok_or(NeptisError::InternalError(
                "Failed to unmount point!".into(),
            ))?;
        }
        fs::remove_dir(m_path)?;
        fs::remove_file(i_path)?;
    }

    Ok(1)
}

fn from_rel_s2(path: &str, point: &Mount) -> Result<(String, bool), NeptisError> {
    // Begin by stripping the prefix and suffix.
    let mut rel_path = path.to_string();
    rel_path = rel_path
        .strip_suffix("/")
        .unwrap_or(rel_path.as_str())
        .to_string();
    rel_path = rel_path
        .strip_prefix("/")
        .unwrap_or(rel_path.as_str())
        .to_string();

    if rel_path.is_empty() {
        return Err(NeptisError::BadRequest(format!("Cannot resolve {}", path)));
    }

    match rel_path.split_once("/") {
        Some(("repo", s2)) => Ok((
            format!("{}/repo-mnt/{}", point.repo_mnt_path, s2),
            !s2.trim().is_empty(),
        )),
        Some(("data", s2)) => Ok((format!("{}/{}", point.data_mnt_path, s2), true)),
        Some(_) => Err(NeptisError::BadRequest(format!(
            "Invalid prefix in {}",
            rel_path
        ))),
        None => Err(NeptisError::BadRequest(format!(
            "Cannot resolve {}",
            rel_path
        ))),
    }
}

fn to_rel_s2(path: &str, point: &Mount) -> Result<String, NeptisError> {
    let rel_path = path.to_string();
    if rel_path.is_empty() {
        return Err(NeptisError::BadRequest(format!("Cannot resolve {}", path)));
    }

    if rel_path.starts_with(point.data_mnt_path.as_str()) {
        return Ok(rel_path.replace(point.data_mnt_path.as_str(), "/data"));
    }

    let r_fmt = format!("{}/repo-mnt", point.repo_mnt_path);
    if rel_path.starts_with(r_fmt.as_str()) {
        return Ok(rel_path.replace(r_fmt.as_str(), "/repo"));
    }

    Err(NeptisError::BadRequest(format!(
        "Cannot resolve {}",
        rel_path
    )))
}

async fn stage_user_s2d(
    path: &str,
    auth_user: &User,
    conn: &mut AsyncPgConnection,
) -> Result<(Mount, String), NeptisError> {
    use crate::schema::mounts::dsl::*;
    let user_mounts: Vec<Mount> = mounts
        .filter(owned_by.eq(auth_user.user_name.as_str()))
        .get_results(conn)
        .await
        .ok()
        .ok_or(NeptisError::InternalError("Failed to pull DB".into()))?;
    return stage_user_s2(path, user_mounts).await;
}

async fn stage_user_s2(
    path: &str,
    user_mounts: Vec<Mount>,
) -> Result<(Mount, String), NeptisError> {
    (async || {
        // Take the first split to determine the name of the repository.
        let mut rel_path = path.to_string();
        rel_path = rel_path
            .strip_suffix("/")
            .unwrap_or(rel_path.as_str())
            .to_string();
        rel_path = rel_path
            .strip_prefix("/")
            .unwrap_or(rel_path.as_str())
            .to_string();

        // Take the first split to determine the name of the repository.
        if rel_path.is_empty() {
            return None;
        }

        let (s1, s2) = rel_path.split_once("/")?;
        for mount in user_mounts {
            if s1 == mount.mount_name.as_str() {
                return Some((mount, s2.to_string()));
            }
        }
        None
    })()
    .await
    .ok_or(NeptisError::BadRequest(format!(
        "Failed to parse path: {}",
        path
    )))
}

#[action]
pub async fn delete_file(path: &str) -> Result<usize, NeptisError> {
    let (fp, is_rw) = stage_user_s2d(path, auth_user, conn)
        .await
        .and_then(|(a, b)| from_rel_s2(b.as_str(), &a))?;
    if !is_rw {
        return Err(NeptisError::BadRequest("The path is not writable!".into()));
    }
    // Attempt to see if it's a file or directory.
    let meta = fs::metadata(fp.as_str())?;
    if meta.is_dir() {
        fs::remove_dir_all(fp.as_str())
            .ok()
            .ok_or(NeptisError::InternalError(
                "Failed to remove directory".into(),
            ))?;
    } else {
        fs::remove_file(fp.as_str())
            .ok()
            .ok_or(NeptisError::InternalError(
                "Failed to remove directory".into(),
            ))?;
    }
    Ok(1)
}

#[action]
pub async fn dump_file(dto: GetForDumpApi) -> Result<String, NeptisError> {
    // Convert to a relative path.
    let (abs_path, _) = stage_user_s2d(dto.path.as_str(), auth_user, conn)
        .await
        .and_then(|(a, b)| from_rel_s2(b.as_str(), &a))?;

    let a_path = abs_path.as_str();
    let mut file = File::open(a_path)?;
    file.seek(dto.offset.into())?;

    let mut buffer = vec![0u8; dto.size];
    let bytes_read = file.read(&mut buffer)?;
    buffer.truncate(bytes_read); // in case file has fewer bytes than `size`

    Ok(BASE64_STANDARD.encode(&buffer))
}

#[action]
pub async fn post_file(dto: PostForFileApi) -> Result<(), NeptisError> {
    // First, attempt to run the main function - then rename if required.
    let (abs_path, is_rw) = stage_user_s2d(dto.path.as_str(), auth_user, conn)
        .await
        .and_then(|(a, b)| from_rel_s2(b.as_str(), &a))?;
    if !is_rw {
        return Err(NeptisError::BadRequest(
            "This requested file is read-only!".into(),
        ));
    }

    let a_path = abs_path.as_str();
    if fs::exists(a_path)? {
        return Err(NeptisError::BadRequest(
            "File already exists! Use PUT method.".into(),
        ));
    }

    if dto.is_dir {
        if dto.base64.is_some() {
            return Err(NeptisError::BadRequest(
                "You cannot write file contents to a directory!".into(),
            ));
        } else {
            return Ok(fs::create_dir_all(a_path)?);
        }
    } else {
        let decoded_data = dto
            .base64
            .map(|x| BASE64_STANDARD.decode(x).ok())
            .and_then(|x| x)
            .unwrap_or_else(Vec::new);
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(a_path)?;

        if let Some(offset) = dto.offset {
            file.seek(offset.into())?;
        }

        file.write_all(decoded_data.as_slice())?;
        return Ok(());
    }
}

#[action]
pub async fn put_xattr(dto: PutForXattrApi) -> Result<(), NeptisError> {
    // First, attempt to run the main function - then rename if required.
    let (abs_path, is_rw) = stage_user_s2d(dto.path.as_str(), auth_user, conn)
        .await
        .and_then(|(a, b)| from_rel_s2(b.as_str(), &a))?;
    if !is_rw {
        return Err(NeptisError::BadRequest(
            "This requested file is read-only!".into(),
        ));
    }
    xattr::set(
        abs_path,
        dto.key,
        BASE64_STANDARD
            .decode(dto.base64)
            .ok()
            .ok_or(NeptisError::BadRequest("Failed to decode base64!".into()))?
            .as_slice(),
    )
    .ok()
    .ok_or(NeptisError::InternalError("Failed to set XATTR".into()))
}

#[action]
pub async fn delete_xattr(dto: DeleteForXattrApi) -> Result<(), NeptisError> {
    // First, attempt to run the main function - then rename if required.
    let (abs_path, is_rw) = stage_user_s2d(dto.path.as_str(), auth_user, conn)
        .await
        .and_then(|(a, b)| from_rel_s2(b.as_str(), &a))?;
    if !is_rw {
        return Err(NeptisError::BadRequest(
            "This requested file is read-only!".into(),
        ));
    }
    xattr::remove(abs_path, dto.key)
        .ok()
        .ok_or(NeptisError::InternalError("Failed to remove XATTR".into()))
}

#[action]
async fn get_xattrs(path: &str) -> Result<Vec<PutForXattrApi>, NeptisError> {
    // First, attempt to run the main function - then rename if required.
    let (abs_path, _) = stage_user_s2d(path, auth_user, conn)
        .await
        .and_then(|(a, b)| from_rel_s2(b.as_str(), &a))?;
    let a_str = abs_path.as_str();
    let mut output = vec![];
    for attr_name in xattr::list(a_str)
        .ok()
        .ok_or(NeptisError::InternalError(
            "Failed to get list of XATTR".into(),
        ))?
        .into_iter()
    {
        // Attempt to pull it and see if there is a value.
        if let Some(val) = xattr::get(path, attr_name.as_os_str())
            .ok()
            .ok_or(NeptisError::InternalError("Failed to pull XATTR".into()))?
        {
            output.push(PutForXattrApi {
                path: abs_path.to_string(),
                key: attr_name.into_string().unwrap(),
                base64: BASE64_STANDARD.encode(val),
            });
        }
    }
    Ok(output)
}

#[action]
pub async fn put_file(dto: PutForFileApi) -> Result<(), NeptisError> {
    use nix::sys::stat::{UtimensatFlags, utimensat};
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};
    fn to_timespec(time_or_now: &TimeOrNow) -> TimeSpec {
        match time_or_now {
            TimeOrNow::SpecificTime(t) => {
                let duration = t.duration_since(UNIX_EPOCH).unwrap();
                TimeSpec::from(duration)
            }
            TimeOrNow::Now => TimeSpec::from(SystemTime::now().duration_since(UNIX_EPOCH).unwrap()),
        }
    }
    // First, attempt to run the main function - then rename if required.
    let (abs_path, is_rw) = stage_user_s2d(dto.path.as_str(), auth_user, conn)
        .await
        .and_then(|(a, b)| from_rel_s2(b.as_str(), &a))?;
    if !is_rw {
        return Err(NeptisError::BadRequest(
            "This requested file is read-only!".into(),
        ));
    }
    let a_path = abs_path.as_str();

    if !fs::exists(a_path)? {
        return Err(NeptisError::BadRequest(
            "File does not exist! Use POST method.".into(),
        ));
    } else if fs::metadata(a_path)?.is_dir() && dto.base64.is_some() {
        return Err(NeptisError::BadRequest(
            "Cannot write file contents of directory!".into(),
        ));
    }

    if let Some(b_vec) = dto
        .base64
        .map(|x| BASE64_STANDARD.decode(x).ok())
        .and_then(|x| x)
    {
        let mut file = OpenOptions::new().write(true).open(a_path)?;
        if let Some(offset) = dto.offset {
            file.seek(offset.into())?;
        }
        file.write_all(b_vec.as_slice())?;
        file.flush()?;
    }

    let mut use_path = a_path.to_string();
    if let Some(new_path) = dto.new_path {
        let (new_abs_path, new_is_rw) = stage_user_s2d(new_path.as_str(), auth_user, conn)
            .await
            .and_then(|(a, b)| from_rel_s2(b.as_str(), &a))?;
        if !new_is_rw {
            return Err(NeptisError::BadRequest("You cannot move here!".into()));
        }
        fs::rename(a_path, new_abs_path.as_str())?;
        use_path = new_abs_path.clone();
    }

    // Finally, attempt to set the attribute on the path.
    if let Some(r_attr) = dto.attr {
        let metadata = fs::metadata(use_path.as_str())?;
        let atime = to_timespec(
            &r_attr
                .atime
                .unwrap_or(TimeOrNow::SpecificTime(metadata.accessed()?)),
        );
        let mtime = to_timespec(
            &r_attr
                .mtime
                .unwrap_or(TimeOrNow::SpecificTime(metadata.modified()?)),
        );

        // Set the times using utimensat
        utimensat(
            None,              // CWD, None means current working directory
            use_path.as_str(), // Path to file
            &atime,
            &mtime,                          // atime and mtime
            UtimensatFlags::NoFollowSymlink, // Don't follow symlinks
        )
        .ok()
        .ok_or(NeptisError::InternalError(
            "Failed to set attributes!".into(),
        ))?;

        // Finally, set the size of the file if necessary.
        if let Some(size) = r_attr.size {
            let file = OpenOptions::new().write(true).open(&use_path)?;
            file.set_len(size)?;
        }
    }
    Ok(())
}

#[action(Vec<NodeDto>)]
pub async fn browse_file(t_path: &str, depth: u16) -> Result<Vec<NodeDto>, NeptisError> {
    // Recursively scan a directory up to a given relative depth.
    fn raw_browse_dir<P: AsRef<Path>>(
        abs_path: P,
        point: &Mount,
        current_depth: u16,
        max_depth: u16,
    ) -> Option<Vec<NodeDto>> {
        let mut output = Vec::new();
        if current_depth >= max_depth {
            return None;
        }
        (|| {
            // Always attempt to include the original file information.
            let data = fs::metadata(abs_path.as_ref()).ok()?;
            let mut rel_path = to_rel_s2(abs_path.as_ref().to_str()?, point).ok()?;
            if rel_path.is_empty() {
                rel_path = "/".into();
            }
            if !rel_path.starts_with("/data/lost+found") {
                // Do not add this directory since it's R/O and will cause errors
                output.push(NodeDto::from_metadata(
                    format!("/{}{}", point.mount_name, rel_path).as_str(),
                    data,
                ));
            }
            Some(())
        })();
        if let Some(p_ret) = fs::read_dir(abs_path).ok() {
            for p_dir in p_ret {
                (|| {
                    let entry = p_dir.ok()?;
                    let data = entry.metadata().ok()?;
                    if data.is_dir() {
                        if let Some(mut sub_nodes) =
                            raw_browse_dir(entry.path(), point, current_depth + 1, max_depth)
                        {
                            output.append(&mut sub_nodes);
                        }
                    }
                    // Convert to relative path from mount
                    let mut rel_path = to_rel_s2(entry.path().to_str()?, point).ok()?;
                    if rel_path.is_empty() {
                        rel_path = "/".into();
                    }
                    if !rel_path.starts_with("/data/lost+found") {
                        let fp = format!("/{}{}", point.mount_name, rel_path);
                        if !output.iter().any(|x| x.path == fp) {
                            output.push(NodeDto::from_metadata(fp.as_str(), data));
                        }
                    }
                    Some(())
                })();
            }
        }
        Some(output)
    }

    use crate::schema::mounts::dsl::*;
    let user_mounts: Vec<Mount> = mounts
        .filter(owned_by.eq(auth_user.user_name.as_str()))
        .get_results(conn)
        .await
        .ok()
        .ok_or(NeptisError::InternalError("Failed to pull DB".into()))?;

    // Make sure to wipe the end slash!
    let path = if t_path.trim() == "/" {
        "/"
    } else {
        t_path.strip_suffix("/").unwrap_or(t_path)
    };
    let mut result_nodes = Vec::new();
    let request_path = if path.is_empty() { "/" } else { path };
    let starting_depth = request_path.matches('/').count();
    let allowed_depth = starting_depth + depth as usize;

    for mount in user_mounts {
        ensure_point_mounted(&mount, true)?;

        let mount_root = format!("/{}", mount.mount_name);

        // Always include "fake" top-level entries if they are within the allowed relative depth.
        for gen_p in [
            mount_root.clone(),
            format!("{}/repo", mount_root),
            format!("{}/data", mount_root),
        ] {
            if gen_p.starts_with(request_path) && gen_p.matches('/').count() <= allowed_depth {
                result_nodes.push(NodeDto {
                    path: gen_p,
                    atime: Utc.from_utc_datetime(&mount.data_accessed).into(),
                    ctime: Utc.from_utc_datetime(&mount.data_accessed).into(),
                    mtime: Utc.from_utc_datetime(&mount.data_accessed).into(),
                    is_dir: true,
                    bytes: 0,
                });
            }
        }

        // Only traverse mounts relevant to the requested path.
        if request_path.starts_with(&mount_root) {
            // Remove the mount root prefix and trim leading '/'
            let subpath = request_path
                .strip_prefix(&mount_root)
                .unwrap_or("")
                .trim_start_matches('/');

            // Determine the physical starting path based on whether the request is for "repo" or "data"
            let base_path = if subpath.starts_with("repo") {
                let repo_sub = subpath
                    .strip_prefix("repo")
                    .unwrap_or("")
                    .trim_start_matches('/');
                if repo_sub.is_empty() {
                    format!("{}/repo-mnt", mount.repo_mnt_path)
                } else {
                    format!("{}/repo-mnt/{}", mount.repo_mnt_path, repo_sub)
                }
            } else if subpath.starts_with("data") {
                let data_sub = subpath
                    .strip_prefix("data")
                    .unwrap_or("")
                    .trim_start_matches('/');
                if data_sub.is_empty() {
                    mount.data_mnt_path.clone()
                } else {
                    format!("{}/{}", mount.data_mnt_path, data_sub)
                }
            } else {
                // If the subpath doesn't start with a known folder, default to data mount.
                if subpath.is_empty() {
                    mount.data_mnt_path.clone()
                } else {
                    format!("{}/{}", mount.data_mnt_path, subpath)
                }
            };

            // Perform the traversal using the computed starting point.
            if let Some(nodes) = raw_browse_dir(&base_path, &mount, 0, depth) {
                for node in nodes {
                    if node.path.starts_with(request_path)
                        && node.path.matches('/').count() <= allowed_depth
                    {
                        if !result_nodes.iter().any(|x| x.path == node.path) {
                            result_nodes.push(node);
                        }
                    }
                }
            }
        }
    }

    Ok(result_nodes)
}

#[action(Vec<RepoJob>)]
pub async fn get_all_jobs(p_name: &str) -> Result<Vec<RepoJobDto>, NeptisError> {
    use crate::schema::repo_jobs::dsl::*;

    Ok(repo_jobs
        .filter(
            point_name
                .eq(p_name)
                .and(point_owned_by.eq(auth_user.user_name.as_str())),
        )
        .get_results(conn)
        .await?)
}

#[action(RepoJob)]
pub async fn backup_mount(
    handler: &NonBlockingRustic,
    dto: PostForBackupApi,
) -> Result<RepoJobDto, NeptisError> {
    // Attempt to validate to ensure the DTO is valid.
    use crate::schema::mounts::dsl::*;
    use crate::schema::repo_jobs::dsl::*;

    let f_point: Mount = mounts
        .find((dto.point_user.clone(), dto.point_name.clone()))
        .get_result(conn)
        .await?;

    let options = JobLaunchInfo {
        point_owned_by: dto.point_user.clone(),
        point_name: dto.point_name.clone(),
        repo_path: format!("{}/repo", f_point.repo_mnt_path.as_str()),
        repo_pass: f_point.repo_password.clone(),
    };

    let b_opts = BackupOptions::default().dry_run(dto.dry_run);
    let source = PathList::from_string(f_point.data_mnt_path.as_str())?
        .sanitize()
        .unwrap();
    let mut s_opts = SnapshotOptions::default();
    if let Some(ref tags) = dto.tags {
        s_opts = s_opts.add_tags(tags.join(",").as_str())?;
    }
    let ret_id = handler.start_backup(&options, source, s_opts, b_opts)?;

    // Finally, return the job information.
    Ok(repo_jobs.find(ret_id).get_result(conn).await?)
}

#[action(Mount)]
pub async fn put_mount(m_name: &str, dto: PutForMountApi) -> Result<MountDto, NeptisError> {
    use crate::schema::mounts::dsl::*;

    let d_max_bytes = dto.data_bytes;
    let r_max_bytes = dto.repo_bytes;
    if d_max_bytes <= 5_000_000 {
        return Err(NeptisError::BadRequest(
            "You must allocate at least 5MB on your data storage!".into(),
        ));
    }

    if r_max_bytes <= 5_000_000 {
        return Err(NeptisError::BadRequest(
            "You must allocate at least 5MB on your repository storage.".into(),
        ));
    }

    // Get the free space to ensure we don't go over.
    let data_path = get_env!("DATA_PATH");
    let repo_path = get_env!("REPO_PATH");

    let d_sys_info = get_system_info(data_path.as_str())?;
    let r_sys_info = get_system_info(repo_path.as_str())?;

    let all_mounts: Vec<Mount> = mounts
        .filter(owned_by.eq(auth_user.user_name.clone()))
        .get_results(conn)
        .await?;

    // If the mount exists, we will attempt to resize it - otherwise, just
    // create a new one instead.
    if let Some(f_point) = all_mounts
        .iter()
        .find(|x| x.mount_name == m_name && x.owned_by == auth_user.user_name.clone())
    {
        // Begin performing the re-size, and update the DB.
        ensure_point_mounted(f_point, true)?;

        // If the snapshot part is 0, just delete it.
        for (b_inc, s_info, is_data, mount_path, image_path) in [
            (
                d_max_bytes - f_point.data_max_bytes,
                get_system_info(f_point.data_mnt_path.as_str())?,
                true,
                f_point.data_mnt_path.as_str(),
                f_point.data_img_path.as_str(),
            ),
            (
                r_max_bytes - f_point.repo_max_bytes,
                get_system_info(f_point.repo_mnt_path.as_str())?,
                false,
                f_point.repo_mnt_path.as_str(),
                f_point.repo_img_path.as_str(),
            ),
        ] {
            if b_inc == 0 {
                return Err(NeptisError::BadRequest(
                    "No modification is necessary".into(),
                ));
            }
            if (d_max_bytes as usize) < s_info.b_used {
                // Not allowed - need more free space to shrink partition.
                return Err(NeptisError::BadRequest(
                    "Not enough free space to shrink. Please delete files and try again".into(),
                ));
            }
            if b_inc > 0 && d_sys_info.b_avail < b_inc as usize {
                return Err(NeptisError::BadRequest(
                    "Not enough free space on server".into(),
                ));
            }
            ensure_single_limit(
                auth_user,
                (all_mounts
                    .iter()
                    .map(|x| {
                        if is_data {
                            x.data_max_bytes as usize
                        } else {
                            x.repo_max_bytes as usize
                        }
                    })
                    .sum::<usize>() as i64
                    + b_inc as i64) as usize,
                is_data,
            )?;

            // At this point, the commands are different based
            // on increase or decrease.
            let b_str = d_max_bytes.to_string();
            let sec_str = format!("{}", d_max_bytes / get_img_block_size(image_path)? as i64);
            let temp_dir = format!("{}/{}", data_path, Uuid::new_v4().to_string());
            if b_inc > 0 {
                (|| {
                    if !is_data {
                        cmd!("umount {}/repo-mnt", mount_path)?; // unmount the repository to prevent busy requests.
                    }
                    fs::create_dir_all(temp_dir.as_str()).ok()?;
                    cmd!("umount {}", mount_path)?;
                    cmd!("e2fsck -f -y {}", image_path)?;
                    cmd!("fallocate -l {} {}", b_str.as_str(), image_path)?;
                    cmd!("e2fsck -f -y {}", image_path)?;
                    cmd!("resize2fs {}", image_path)?;
                    cmd!("mount -o rw,sync,loop {} {}", image_path, temp_dir.as_str())?;
                    cmd!("umount {}", temp_dir.as_str())?;
                    fs::remove_dir(temp_dir.as_str()).ok()?;
                    Some(())
                })()
                .ok_or(NeptisError::InternalError("Failed to perform grow".into()))?;
            } else {
                (|| {
                    if !is_data {
                        cmd!("umount {}/repo-mnt", mount_path)?; // unmount the repository to prevent busy requests.
                    }
                    fs::create_dir_all(temp_dir.as_str()).ok()?;
                    cmd!("umount {}", mount_path)?;
                    cmd!("e2fsck -f -y {}", image_path)?;
                    cmd!("resize2fs {} {}", image_path, sec_str.as_str())?;
                    cmd!("e2fsck -f -y {}", image_path)?;
                    cmd!("mount -o rw,sync,loop {} {}", image_path, temp_dir.as_str())?;
                    cmd!("umount {}", temp_dir.as_str())?;
                    fs::remove_dir(temp_dir.as_str()).ok()?;
                    Some(())
                })()
                .ok_or(NeptisError::InternalError(
                    "Failed to perform shrink".into(),
                ))?;
            }
        }

        // We are done! Simply update the DB to continue.
        let mut u_point = f_point.clone();
        u_point.data_max_bytes = d_max_bytes;
        u_point.repo_max_bytes = r_max_bytes;
        u_point.repo_accessed = utc_now!();
        u_point.data_accessed = utc_now!();
        Ok(diesel::update(mounts)
            .set(&u_point)
            .get_result(conn)
            .await?)
    } else {
        if d_sys_info.b_avail < d_max_bytes as usize {
            return Err(NeptisError::BadRequest(
                "Not enough data free space on server".into(),
            ));
        }
        if r_sys_info.b_avail < r_max_bytes as usize {
            return Err(NeptisError::BadRequest(
                "Not enough repo free space on server".into(),
            ));
        }
        ensure_user_limit(
            auth_user,
            all_mounts
                .iter()
                .map(|x| x.data_max_bytes as usize)
                .sum::<usize>()
                + d_max_bytes as usize,
            all_mounts
                .iter()
                .map(|x| x.repo_max_bytes as usize)
                .sum::<usize>()
                + r_max_bytes as usize,
        )?;

        // Attempt to create the directory and run the allocating commands.
        let p_mount = Mount {
            mount_name: m_name.to_string(),
            owned_by: auth_user.user_name.clone(),
            data_img_path: format!(
                "{}/{}-{}-DATA.img",
                data_path.clone(),
                m_name,
                auth_user.user_name.clone()
            ),
            data_mnt_path: format!(
                "{}/{}-{}-DATA",
                data_path.clone(),
                m_name,
                auth_user.user_name.clone()
            ),
            repo_img_path: format!(
                "{}/{}-{}-REPO.img",
                repo_path.clone(),
                m_name,
                auth_user.user_name.clone()
            ),
            repo_mnt_path: format!(
                "{}/{}-{}-REPO",
                repo_path.clone(),
                m_name,
                auth_user.user_name.clone()
            ),
            repo_password: PasswordGenerator::new()
                .length(8)
                .numbers(true)
                .lowercase_letters(true)
                .uppercase_letters(true)
                .symbols(false)
                .spaces(false)
                .exclude_similar_characters(true)
                .strict(true)
                .generate_one()
                .unwrap(),
            data_max_bytes: d_max_bytes as i64,
            repo_max_bytes: r_max_bytes as i64,
            date_created: utc_now!(),
            data_accessed: utc_now!(),
            repo_accessed: utc_now!(),
            locked: false, // will not be inserted until very end
        };

        for (m_path, i_path, m_bytes) in [
            (
                p_mount.data_mnt_path.as_str(),
                p_mount.data_img_path.as_str(),
                p_mount.data_max_bytes,
            ),
            (
                p_mount.repo_mnt_path.as_str(),
                p_mount.repo_img_path.as_str(),
                p_mount.repo_max_bytes,
            ),
        ] {
            fs::create_dir_all(m_path)
                .map_err(|_| NeptisError::InternalError("Failed to create directory".into()))?;
            (|| {
                cmd!("fallocate -l {0} {1}", m_bytes, i_path)?;
                cmd!("mkfs.ext4 {0}", i_path)?;
                cmd!("chmod 777 {0}", i_path)?;
                Some(())
            })()
            .ok_or(NeptisError::InternalError(
                "Failed to create/mount FS".into(),
            ))?;
        }

        // We need to actually create the repository via rustic.
        ensure_point_mounted(&p_mount, false)?;
        let repo_dir = format!("{}/repo", p_mount.repo_mnt_path.as_str());
        let backends = BackendOptions::default()
            .repository(repo_dir)
            .to_backends()?;
        let repo_opts = RepositoryOptions::default().password(p_mount.repo_password.as_str());
        let key_opts = KeyOptions::default();
        let config_opts = ConfigOptions::default();
        Repository::new(&repo_opts, &backends)?.init(&key_opts, &config_opts)?;

        // Finally, insert to the DB and finish.
        Ok(diesel::insert_into(mounts)
            .values(&p_mount)
            .get_result(conn)
            .await?)
    }
}
