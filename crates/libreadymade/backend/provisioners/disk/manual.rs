use crate::{
    backend::provisioners::{Mounts, disk::DiskProvisionerModule},
    prelude::*,
};

#[derive(Debug, Serialize, Deserialize, Clone, Eq, PartialEq)]
pub struct Manual {
    pub mounts: Mounts,
}

impl DiskProvisionerModule for Manual {
    fn run(&self, _: &crate::playbook::Playbook) -> Result<Mounts> {
        Ok(self.mounts.clone())
    }
}
