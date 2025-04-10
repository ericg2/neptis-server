use rocket::serde::json::Value;
use serde::Serialize;
use crate::mounts::dtos::{NodeDto, PutForXattrApi};
use crate::users::models::User;
use crate::api::errors::*;

macro_rules! setup {
    ($($t:ty),*) => {
        $(
            impl WebDtoFrom<$t> for $t {
                fn try_to_dto(_auth_user: &User, item: $t) -> Result<Self, NeptisError>
                where 
                    Self: Serialize + Sized 
                {
                    Ok(item)
                }
            }

            impl WebDtoFrom<Vec<$t>> for Vec<$t> {
                fn try_to_dto(_auth_user: &User, item: Vec<$t>) -> Result<Self, NeptisError>
                where 
                    Self: Serialize + Sized 
                {
                    Ok(item)
                }
            }
        )*
    };
}

// Setup all primitive types for implementations.
setup!(
    u8, u16, u32, u64, usize, i8, i16, i32, i64, isize, f32, f64, bool, char, String, NodeDto, Value, (), PutForXattrApi
);

pub trait WebDtoFrom<TBase> {
    fn try_to_dto(auth_user: &User, item: TBase) -> Result<Self, NeptisError>
    where
        Self: Serialize + Sized;
}

pub trait CleanValidate {
    fn validate(self) -> Result<Self, ValidateError>
    where
        Self: Sized;
}