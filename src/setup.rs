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

impl Into<PostInstallation> for User {
    fn into(self) -> PostInstallation {
        let mut params = vec![
            Value::String(self.username),
            Value::String(self.display_name),
            Value::Array(self.groups.into_iter().map(Value::String).collect()),
        ];

        if let Some(password) = self.password {
            params.push(Value::String(password));
        }
        PostInstallation {
            chroot: true,
            operation: crate::albius::PostInstallationOperation::Adduser,
            params,
        }
    }
}
