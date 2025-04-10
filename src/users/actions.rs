use base64::{Engine, prelude::BASE64_STANDARD};
use diesel::ExpressionMethods;
use rocket::serde::json;
use rocket::serde::json::Value;

use super::dtos::*;
use super::models::*;
use crate::api::traits::CleanValidate;
use crate::api::util::{decrypt, encrypt};
use crate::{api::traits::WebDtoFrom, prelude::action_prelude::*};
use json::serde_json::json;

#[admin_action(User)]
pub async fn create_user(user: UserForCreateApi) -> Result<UserDto, NeptisError> {
    use crate::schema::users::dsl::*;

    let u: User = user.into();
    Ok(diesel::insert_into(users)
        .values(u.validate()?)
        .get_result(conn)
        .await?)
}

#[admin_action(User)]
pub async fn update_user(name: &str, user: UserForUpdateApi) -> Result<UserDto, NeptisError> {
    use crate::schema::users::dsl::*;
    Ok(diesel::update(users)
        .set(user.to_db(name))
        .get_result(conn)
        .await?)
}

#[action(User)]
pub async fn get_one_user(name: &str) -> Result<UserDto, NeptisError> {
    use crate::schema::users::dsl::*;
    Ok(users.find(name).get_result(conn).await?)
}

#[action(Vec<User>)]
pub async fn get_all_users() -> Result<Vec<UserDto>, NeptisError> {
    use crate::schema::users::dsl::*;
    Ok(users.load(conn).await?)
}

#[action]
pub async fn delete_one_user(name: &str) -> Result<usize, NeptisError> {
    use crate::schema::users::dsl::*;
    if !auth_user.is_admin {
        return Err(NeptisError::Unauthorized(
            "You must be admin to delete a user!".to_string(),
        ));
    }
    if name == auth_user.user_name {
        return Err(NeptisError::BadRequest(
            "You cannot delete yourself!".into(),
        ));
    }
    Ok(diesel::delete(users)
        .filter(user_name.eq(name.to_string()))
        .execute(conn)
        .await?)
}

fn get_key() -> Vec<u8> {
    std::env::var("SIGNING_KEY")
        .expect("SIGNING_KEY must be set")
        .as_bytes()
        .to_vec()
}

#[no_auth_action]
pub async fn register_session(user: UserForLoginApi) -> Result<Value, NeptisError> {
    use crate::schema::sessions::dsl::*;
    use crate::schema::users::dsl::*;

    let signing_key = get_key();
    let session = Session::new(user.user_name.as_str());

    // First, make sure the passwords match!
    let d_user: User = users.find(user.user_name).get_result(conn).await?;
    if d_user.password_hash != user.password {
        return Err(NeptisError::BadRequest("Invalid password!".into()));
    }
    // First, attempt to encrypt the session before saving to database.
    let output = encrypt(signing_key.as_slice(), session.id.to_string().as_bytes())
        .ok_or(NeptisError::InternalError(
            "Failed to encrypt signing key!".into(),
        ))
        .map(|x| BASE64_STANDARD.encode(x))?;
    diesel::insert_into(sessions)
        .values(&session)
        .execute(conn)
        .await?;
    Ok(json!({"token": output, "expire_date": session.expire_date}))
}

async fn parse_token<'r>(req: &'r Request<'_>) -> Option<User> {
    use crate::schema::sessions::dsl::*;
    use crate::schema::users::dsl::*;

    let header = req.headers().get_one("Authorization")?.to_string();
    let token_spl = header
        .split_ascii_whitespace()
        .into_iter()
        .map(|x| x.to_string())
        .collect::<Vec<_>>();
    let a_type = token_spl.get(0)?;
    let a_key = token_spl.get(1)?;
    if a_type == "Bearer" {
        let dec = decrypt(
            get_key().as_slice(),
            BASE64_STANDARD.decode(a_key).ok()?.as_slice(),
        )?;
        let token = Uuid::try_parse_ascii(dec.as_slice()).ok()?;
        let mut db = match req.guard::<rocket_db_pools::Connection<Db>>().await {
            rocket::outcome::Outcome::Success(conn) => Some(conn),
            _ => None,
        }?;

        let conn = &mut **db;
        users
            .find(
                sessions
                    .find(token)
                    .get_result::<Session>(conn)
                    .await
                    .map(|x| {
                        if x.enabled && Utc::now().naive_utc() < x.expire_date {
                            Some(x.user_name)
                        } else {
                            None
                        }
                    })
                    .ok()??,
            )
            .get_result::<User>(conn)
            .await
            .ok()
    } else {
        None
    }
}

#[rocket::async_trait]
impl<'r> FromRequest<'r> for User {
    type Error = ();
    async fn from_request(req: &'r Request<'_>) -> request::Outcome<Self, ()> {
        match parse_token(req).await {
            Some(user) => request::Outcome::Success(user),
            None => request::Outcome::Forward(Status::Unauthorized),
        }
    }
}
