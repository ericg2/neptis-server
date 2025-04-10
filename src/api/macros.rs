#[macro_export]
macro_rules! to_dto {
    // Case: Single value
    ($base_type:ty, $dto_type:ty, $auth_user:expr, $item:expr) => {
        Ok(<$dto_type as crate::api::traits::WebDtoFrom<$base_type>>::try_to_dto($auth_user, $item)?)
    };

    // Case: Multiple values (Vec-like)
    ($base_type:ty, $dto_type:ty,  $auth_user:expr, $($item:expr),+ $(,)?) => {
        Ok(vec![$(
            <$dto_type as crate::api::traits::WebDtoFrom<$base_type>>::try_to_dto($auth_user, $item)?
        ),+])
    };
}

#[macro_export]
macro_rules! get_env {
    ($value:expr) => {
        std::env::var($value)
            .expect("Failed to pull environment variable!")
            .trim()
            .to_string()
    };
}

#[macro_export]
macro_rules! trim {
    ($($field:expr),*) => {
        $(
            $field = $field.trim().to_string();
        )*
    };
}

#[macro_export]
macro_rules! vreq {
    // Custom error message
    ($value:expr, $msg:expr) => {{
        if $value.is_empty() {
            return Err(crate::api::errors::ValidateError::ValueRequired(
                $msg.to_string(),
            ));
        }
    }};
}

#[macro_export]
macro_rules! vmin {
    ($value:expr, $min:expr, $msg:expr) => {{
        if $value < $min {
            return Err(crate::api::errors::ValidateError::OutOfRange(
                $msg.to_string(),
            ));
        }
    }};
}

#[macro_export]
macro_rules! vmax {
    ($value:expr, $max:expr, $msg:expr) => {{
        if $value > $max {
            return Err(crate::api::errors::ValidateError::OutOfRange(
                $msg.to_string(),
            ));
        }
    }};
}
#[macro_export]
macro_rules! cmd {
    ($arg:expr) => {{
        // Delegate to the formatted version
        cmd!("{}", $arg)
    }};
    ($fmt:expr, $($arg:tt)*) => {{
        use std::process::Command;
        let command_str = format!($fmt, $($arg)*);
        let output = Command::new("sudo")
            .arg("su")
            .arg("-s")
            .arg("/bin/bash")
            .arg("root")
            .arg("-c")
            .arg(&command_str)
            .output()
            .ok();
        output.and_then(|o| {
            if o.status.success() {
                Some(String::from_utf8_lossy(&o.stdout).trim().to_string())
            } else {
                None
            }
        })
    }};
}

#[macro_export]
macro_rules! utc_now {
    () => {
        chrono::Utc::now().naive_utc()
    };
}

#[macro_export]
macro_rules! vreqany {
    // Default message case
    ($value:expr) => {{
        vrequired_any!($value, "This field is required");
    }};

    // Custom message case
    ($value:expr, $msg:expr) => {{
        let is_invalid = match &$value {
            None => true,   // `Option<T>` is `None`
            Err(e) => true, // `Result<T, E>` is `Err(e)`
            _ => false,     // Otherwise valid
        };

        if is_invalid {
            let error_message = match &$value {
                Err(e) => format!("{}: {}", $msg, e), // Include error message from Result<T, E>
                _ => $msg.to_string(),
            };
            return Err(crate::api::errors::ValidateError::ValueRequired(
                error_message,
            ));
        }
    }};
}

#[macro_export]
macro_rules! vcheck {
    ($closure:expr) => {{
        let result: Result<(), &str> = $closure(); // Call the closure
        if let Err(e) = result {
            return Err(crate::api::errors::ValidateError::CustomError(
                e.to_string(),
            )); // Convert error to String
        }
    }};
}

#[macro_export]
macro_rules! verr {
    ($msg:expr) => {
        return Err(crate::api::errors::ValidateError::CustomError(
            msg.to_string(),
        ));
    };
}

#[macro_export]
macro_rules! bind_dto {
    // Match the input for two types: base type (e.g., User) and DTO type (e.g., UserDto)
    ($base_type:ident, $dto_type:ident) => {
        // Generate the implementation for WebDtoFrom<Vec<$base_type>> for Vec<$dto_type>
        impl crate::api::traits::WebDtoFrom<Vec<$base_type>> for Vec<$dto_type> {
            fn try_to_dto(
                auth_user: &crate::users::models::User,
                item: Vec<$base_type>,
            ) -> Result<Self, crate::api::errors::NeptisError>
            where
                Self: serde::Serialize + Sized,
            {
                let mut output = vec![];
                for x in item {
                    // Call try_to_dto for each item, dynamically using the $dto_type
                    output.push($dto_type::try_to_dto(auth_user, x)?);
                }
                Ok(output)
            }
        }
    };
}
