use crate::albius::{Parameter, PostInstallation};
use serde::{Deserialize, Serialize};

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
            Parameter::String(self.username),
            Parameter::String(self.display_name),
            Parameter::Array(self.groups.into_iter().map(Parameter::String).collect()),
        ];

        if let Some(password) = self.password {
            params.push(Parameter::String(password));
        }
        PostInstallation {
            chroot: true,
            operation: crate::albius::PostInstallationOperation::Adduser,
            params,
        }
    }
}
