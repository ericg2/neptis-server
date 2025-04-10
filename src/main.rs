#![recursion_limit = "2048"]

use api::hash::EncodedHash;
use diesel::query_dsl::methods::FindDsl;
use mounts::rustic_async::NonBlockingRustic;
use rocket::serde::json::serde_json::json;
use rocket::{Orbit, Rocket};
use rocket_db_pools::diesel::prelude::RunQueryDsl;
use rocket_db_pools::Database;
use rocket_db_pools::diesel::PgPool;
use users::dtos::UserForCreateApi;
use users::models::User;

#[macro_use]
extern crate diesel;

#[macro_use]
extern crate rocket;

mod api;
mod prelude;
mod schema;
mod users;
mod mounts;

#[derive(Database)]
#[database("neptis_db")]
pub struct Db(PgPool);

#[rocket::launch]
fn rocket() -> _ {
    // Make sure to create the admin user.
    let nb = NonBlockingRustic::new();
    dotenvy::dotenv().expect("No environment variable file found!");
    rocket::build()
        .attach(Db::init())
        .mount("/api/users", users::handlers::get_routes())
        .mount("/api/mounts", mounts::handlers::get_routes())
        .manage(nb)
        .register("/", catchers![not_found, unauthorized])
        .attach(rocket::fairing::AdHoc::on_liftoff("Database Init", |rocket| {
            Box::pin(async move {
                if let Some(mut conn) = async {
                    rocket.state::<Db>()?.0.get().await.ok()
                }.await {
                    // Ensure the admin user exists!
                    use crate::schema::users::dsl::*;
                    let c_user: User = UserForCreateApi {
                        user_name: "admin".into(),
                        password: "XXXXXXXXX".into(),
                        first_name: "Admin".into(),
                        last_name: "Admin".into(),
                        is_admin: true,
                        max_data_bytes: None,
                        max_snapshot_bytes: None
                    }.into();
                    let r = diesel::insert_into(users).values(c_user).execute(&mut conn).await;
                    if r.is_err() {
                        let y = r.unwrap_err();
                        println!("{}", y);
                    }
                }
            })
        }))
}

#[catch(404)]
fn not_found() -> &'static str {
    return "Not Found";
}

#[catch(401)]
fn unauthorized() -> &'static str {
    return "Unauthorized";
}