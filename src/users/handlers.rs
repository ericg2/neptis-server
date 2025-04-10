use rocket::serde::json::Value;

use super::{actions, dtos::*};
use crate::prelude::route_prelude::*;

#[get("/")]
async fn get_all_users(
    mut conn: Connection<Db>,
    auth_user: User,
) -> Result<Json<Vec<UserDto>>, NeptisError> {
    Ok(Json(
        actions::get_all_users_async(&mut **conn, &auth_user).await?,
    ))
}

#[get("/<name>")]
async fn get_one_user(
    mut conn: Connection<Db>,
    auth_user: User,
    name: &str,
) -> Result<Json<UserDto>, NeptisError> {
    Ok(Json(
        actions::get_one_user_async(&mut **conn, &auth_user, name).await?,
    ))
}

#[put("/<name>", data = "<dto>")]
async fn put_one_user(
    mut conn: Connection<Db>,
    auth_user: User,
    name: &str,
    dto: Json<UserForUpdateApi>,
) -> Result<Json<UserDto>, NeptisError> {
    Ok(Json(
        actions::priv_update_user_async(&mut **conn, &auth_user, name, dto.into_inner()).await?,
    ))
}

#[post("/", data = "<dto>")]
async fn create_one_user(
    mut conn: Connection<Db>,
    auth_user: User,
    dto: Json<UserForCreateApi>,
) -> Result<Json<UserDto>, NeptisError> {
    Ok(Json(
        actions::priv_create_user_async(&mut **conn, &auth_user, dto.into_inner()).await?,
    ))
}

#[post("/auth", data = "<dto>")]
async fn do_auth(
    mut conn: Connection<Db>,
    dto: Json<UserForLoginApi>,
) -> Result<Json<Value>, NeptisError> {
    Ok(Json(
        actions::register_session_async(&mut **conn, dto.into_inner()).await?,
    ))
}

#[delete("/<name>")]
async fn delete_one_user(
    mut conn: Connection<Db>,
    auth_user: User,
    name: &str,
) -> Result<(), NeptisError> {
    actions::delete_one_user_async(&mut **conn, &auth_user, name).await?;
    Ok(())
}

pub fn get_routes() -> Vec<Route> {
    routes![
        get_all_users,
        get_one_user,
        put_one_user,
        create_one_user,
        do_auth,
        delete_one_user
    ]
}
