#[derive(Debug, Clone)]
pub struct LsblkOutput {
    pub path: String,
    pub uuid: String,
    pub parttype: String,
    pub parttypename: String,
}

impl LsblkOutput {
    pub fn match_device(&self, device: &str) -> bool {
        self.path.contains(device)
    }
}

pub fn generate_lsblk() -> Option<Vec<LsblkOutput>> {
    let disks = rs_drivelist::drive_list().ok()?;
    for _disk in disks.into_iter().filter(super::_lsblk_filter) {}
    todo!()
}
