use super::{actions, dtos::*, rustic_async::NonBlockingRustic};
use crate::prelude::route_prelude::*;
use rocket::{State, response::content::RawText};

#[get("/")]
async fn get_all_mounts(
    mut conn: Connection<Db>,
    auth_user: User,
) -> Result<Json<Vec<MountDto>>, NeptisError> {
    Ok(Json(
        actions::get_all_mounts_for_user_async(&mut **conn, &auth_user).await?,
    ))
}

#[get("/id/<name>")]
async fn get_one_mount(
    mut conn: Connection<Db>,
    auth_user: User,
    name: &str,
) -> Result<Json<MountDto>, NeptisError> {
    Ok(Json(
        actions::get_one_mount_async(&mut **conn, &auth_user, name).await?,
    ))
}

#[put("/id/<name>", data = "<bytes>")]
async fn put_one_mount(
    mut conn: Connection<Db>,
    auth_user: User,
    name: &str,
    bytes: Json<PutForMountApi>,
) -> Result<Json<MountDto>, NeptisError> {
    Ok(Json(
        actions::put_mount_async(&mut **conn, &auth_user, name, bytes.into_inner()).await?,
    ))
}

#[post("/id/<name>/backup")]
async fn post_one_backup(
    mut conn: Connection<Db>,
    handler: &State<NonBlockingRustic>,
    auth_user: User,
    name: &str,
) -> Result<Json<RepoJobDto>, NeptisError> {
    Ok(Json(
        actions::backup_mount_async(
            &mut **conn,
            &auth_user,
            handler.inner(),
            PostForBackupApi {
                point_user: auth_user.user_name.clone(),
                point_name: name.to_string(),
                tags: None,
                dry_run: false,
            },
        )
        .await?,
    ))
}

#[get("/id/<name>/jobs")]
async fn get_all_jobs_for_mount(
    mut conn: Connection<Db>,
    auth_user: User,
    name: &str,
) -> Result<Json<Vec<RepoJobDto>>, NeptisError> {
    Ok(Json(
        actions::get_all_jobs_async(&mut **conn, &auth_user, name).await?,
    ))
}

#[delete("/id/<name>")]
async fn delete_one_mount(
    mut conn: Connection<Db>,
    auth_user: User,
    name: &str,
) -> Result<(), NeptisError> {
    actions::delete_one_mount_async(&mut **conn, &auth_user, name).await?;
    Ok(())
}

#[get("/browse", data = "<path>")]
async fn browse_file(
    mut conn: Connection<Db>,
    auth_user: User,
    path: &str,
) -> Result<Json<Vec<NodeDto>>, NeptisError> {
    Ok(Json(
        actions::browse_file_async(&mut **conn, &auth_user, path, 2).await?,
    ))
}

#[get("/dump", data = "<dto>")]
async fn dump_file(
    mut conn: Connection<Db>,
    auth_user: User,
    dto: Json<GetForDumpApi>,
) -> Result<RawText<String>, NeptisError> {
    Ok(RawText(
        actions::dump_file_async(&mut **conn, &auth_user, dto.into_inner()).await?,
    ))
}

#[put("/file", data = "<dto>")]
async fn put_file(
    mut conn: Connection<Db>,
    auth_user: User,
    dto: Json<PutForFileApi>,
) -> Result<(), NeptisError> {
    Ok(actions::put_file_async(&mut **conn, &auth_user, dto.into_inner()).await?)
}

#[get("/xattrs", data = "<path>")]
async fn get_xattrs(
    mut conn: Connection<Db>,
    auth_user: User,
    path: &str,
) -> Result<Json<Vec<PutForXattrApi>>, NeptisError> {
    Ok(Json(
        actions::get_xattrs_async(&mut **conn, &auth_user, path).await?,
    ))
}

#[put("/xattrs", data = "<dto>")]
async fn put_xattrs(
    mut conn: Connection<Db>,
    auth_user: User,
    dto: Json<PutForXattrApi>,
) -> Result<(), NeptisError> {
    Ok(actions::put_xattr_async(&mut **conn, &auth_user, dto.into_inner()).await?)
}

#[delete("/xattrs", data = "<dto>")]
async fn delete_xattr(
    mut conn: Connection<Db>,
    auth_user: User,
    dto: Json<DeleteForXattrApi>,
) -> Result<(), NeptisError> {
    Ok(actions::delete_xattr_async(&mut **conn, &auth_user, dto.into_inner()).await?)
}

#[post("/file", data = "<dto>")]
async fn post_file(
    mut conn: Connection<Db>,
    auth_user: User,
    dto: Json<PostForFileApi>,
) -> Result<(), NeptisError> {
    Ok(actions::post_file_async(&mut **conn, &auth_user, dto.into_inner()).await?)
}

#[delete("/file", data = "<path>")]
async fn delete_file(
    mut conn: Connection<Db>,
    auth_user: User,
    path: &str,
) -> Result<Json<usize>, NeptisError> {
    Ok(Json(
        actions::delete_file_async(&mut **conn, &auth_user, path).await?,
    ))
}

pub fn get_routes() -> Vec<Route> {
    routes![
        get_all_mounts,
        get_one_mount,
        put_one_mount,
        delete_one_mount,
        post_one_backup,
        browse_file,
        put_file,
        delete_file,
        get_all_jobs_for_mount,
        dump_file,
        post_file,
        get_xattrs,
        put_xattrs,
        delete_xattr
    ]
}
