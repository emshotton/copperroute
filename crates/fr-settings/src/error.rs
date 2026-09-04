#[derive(Debug, thiserror::Error)]
pub enum SettingsError {
            #[error(transparent)]
    Dsn(#[from] fr_dsn::DsnError),

        #[error(transparent)]
    Io(#[from] std::io::Error),

        #[error(transparent)]
    Json(#[from] serde_json::Error),
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum MergeError {
            #[error("no such field: {path}")]
    NoSuchField {
                path: String,
    },

            #[error("field {path}: {value:?} is not a valid number")]
    NumberFormat {
                path: String,
                value: String,
    },

            #[error("field {path}: {value:?} is not a valid enum constant")]
    EnumName {
                path: String,
                value: String,
    },

                                #[error("field {path}: {value:?} cannot be assigned to a field of this type")]
    TypeMismatch {
                path: String,
                value: String,
    },
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MergeReport {
        pub fields_changed: usize,
        pub errors: Vec<MergeError>,
}
