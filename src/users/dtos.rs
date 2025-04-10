use crate::{prelude::model_prelude::*};
use super::models::{User, UserForUpdate};

impl WebDtoFrom<User> for UserDto {
    fn try_to_dto(auth_user: &User, item: User) -> Result<Self, NeptisError>
    where
        Self: serde::Serialize + Sized,
    {
        // Just return the information right now.
        Ok(UserDto {
            user_name: item.user_name,
            first_name: item.first_name,
            last_name: item.last_name,
            create_date: item.create_date,
            is_admin: item.is_admin,
            max_data_bytes: if auth_user.is_admin {
                Some(item.max_data_bytes)
            } else {
                None
            },
            max_snapshot_bytes: if auth_user.is_admin {
                Some(item.max_snapshot_bytes)
            } else {
                None
            },
        })
    }
}

#[derive(Serialize, Deserialize, WebDto)]
#[web_dto(User)]
pub struct UserDto {
    pub user_name: String,
    pub first_name: String,
    pub last_name: String,
    pub create_date: NaiveDateTime,
    pub is_admin: bool,
    pub max_data_bytes: Option<i64>, // depends on privledge
    pub max_snapshot_bytes: Option<i64> // depends on privledge
}

#[derive(Serialize, Deserialize)]
pub struct UserForCreateApi {
    pub user_name: String,
    pub password: String,
    pub first_name: String,
    pub last_name: String,
    pub is_admin: bool,
    pub max_data_bytes: Option<i64>,
    pub max_snapshot_bytes: Option<i64>,
}

impl From<UserForCreateApi> for User {
    fn from(value: UserForCreateApi) -> Self {
        User {
            user_name: value.user_name,
            password_hash: EncodedHash::hash(value.password),
            first_name: value.first_name,
            last_name: value.last_name,
            create_date: Utc::now().naive_utc(),
            is_admin: value.is_admin,
            max_data_bytes: value.max_data_bytes.unwrap_or(0),
            max_snapshot_bytes: value.max_snapshot_bytes.unwrap_or(0),
        }
    }
}

#[derive(Serialize, Deserialize)]
pub struct UserForUpdateApi {
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    pub is_admin: Option<bool>,
    pub max_data_bytes: Option<i64>,
    pub max_snapshot_bytes: Option<i64>,
    pub password: Option<String>
}

impl UserForUpdateApi {
    pub fn to_db(&self, name: &str) -> UserForUpdate {
        UserForUpdate {
            user_name: name.to_string(),
            first_name: self.first_name.clone(),
            last_name: self.last_name.clone(),
            is_admin: self.is_admin,
            max_data_bytes: self.max_data_bytes.clone(),
            max_snapshot_bytes: self.max_snapshot_bytes.clone(),
            password_hash: self.password.clone().map(|x|EncodedHash::hash(x))
        }
    }
}

#[derive(Serialize, Deserialize)]
pub struct UserForLoginApi {
    pub user_name: String,
    pub password: String
}