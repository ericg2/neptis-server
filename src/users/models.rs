use crate::prelude::model_prelude::*;

#[derive(Insertable, Queryable, Clone)]
pub struct User {
    pub user_name: String,
    pub first_name: String,
    pub last_name: String,
    pub password_hash: EncodedHash,
    pub create_date: NaiveDateTime,
    pub is_admin: bool,
    pub max_data_bytes: i64,
    pub max_snapshot_bytes: i64,
}

impl CleanValidate for User {
    fn validate(mut self) -> Result<Self, ValidateError> {
        trim!(self.user_name, self.first_name, self.last_name);
        vreq!(self.user_name, "Username is empty!");
        vreq!(self.first_name, "First name is empty!");
        vreq!(self.last_name, "Last name is empty!");
        Ok(self)
    }
}


#[derive(AsChangeset)]
#[diesel(table_name = users)]
pub struct UserForUpdate {
    pub user_name: String,
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    pub is_admin: Option<bool>,
    pub max_data_bytes: Option<i64>,
    pub max_snapshot_bytes: Option<i64>,
    pub password_hash: Option<EncodedHash>
}

#[derive(Insertable, Queryable)]
pub struct Session {
    pub id: Uuid,
    pub user_name: String,
    pub create_date: NaiveDateTime,
    pub expire_date: NaiveDateTime,
    pub enabled: bool,
}

impl Session {
    pub fn new(name: &str) -> Self {
        Session {
            id: Uuid::new_v4(),
            user_name: name.to_string(),
            create_date: Utc::now().naive_utc(),
            expire_date: Utc::now().naive_utc() + Duration::hours(1),
            enabled: true,
        }
    }
}
