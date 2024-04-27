use crate::albius::PostInstallation;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Serialize, Deserialize)]
pub struct User {
    pub username: String,
    pub display_name: String,
    pub groups: Vec<String>,
    pub password: Option<String>,
}

impl From<User> for PostInstallation {
    fn from(val: User) -> Self {
        let mut params = vec![
            Value::String(val.username),
            Value::String(val.display_name),
            Value::Array(val.groups.into_iter().map(Value::String).collect()),
        ];

        if let Some(password) = val.password {
            params.push(Value::String(password));
        }
        PostInstallation {
            chroot: true,
            operation: crate::albius::PostInstallationOperation::Adduser,
            params,
        }
    }
}
