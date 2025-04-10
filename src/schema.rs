table! {
    users(user_name) {
        user_name -> Text,
        first_name -> Text,
        last_name -> Text,
        password_hash -> crate::api::hash::EncodedHashType,
        create_date -> Timestamp,
        is_admin -> Bool,
        max_data_bytes -> BigInt,
        max_snapshot_bytes -> BigInt
    }
}
table! {
    sessions(id) {
        id -> Uuid,
        user_name -> Text,
        create_date -> Timestamp,
        expire_date -> Timestamp,
        enabled -> Bool
    }
}
table! {
    mounts(owned_by, mount_name) {
        owned_by -> Text,
        mount_name -> Text,
        data_img_path -> Text,
        data_mnt_path -> Text,
        repo_password -> Text,
        data_max_bytes -> BigInt,
        repo_img_path -> Text,
        repo_mnt_path -> Text,
        repo_max_bytes -> BigInt,
        date_created -> Timestamp,
        data_accessed -> Timestamp,
        repo_accessed -> Timestamp,
        locked -> Bool
    }
}
table! {
    repo_jobs(id) {
        id -> Uuid,
        snapshot_id -> Nullable<Text>,
        point_owned_by -> Text,
        point_name -> Text,
        job_type -> SmallInt,
        job_status -> SmallInt,
        used_bytes -> BigInt,
        total_bytes -> Nullable<BigInt>,
        errors -> Array<Text>,
        create_date -> Timestamp,
        end_date -> Nullable<Timestamp>
    }
}