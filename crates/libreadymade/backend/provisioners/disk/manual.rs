use std::str::FromStr;

use crate::{
    backend::provisioners::disk::DiskProvisionerModule, backend::util::fs::get_whole_disk,
    prelude::*,
};

#[derive(Debug, Serialize, Deserialize, Clone, Eq, PartialEq)]
pub struct Manual {
    pub mounts: Mounts,
}

impl DiskProvisionerModule for Manual {
    fn run(&self, _: &crate::playbook::Playbook) -> Result<Mounts> {
        // let mounts = self.mounts.clone();
        // let block_devices = lsblk::BlockDevice::list()?;

        // let disk_parts = self.mounts.0.iter().map(|mount| {
        //     (get_whole_disk(&mount.partition).expect("cannot get whole disk"), (block_devices.iter().find(|b| b.fullname == mount.mountpoint).expect("cannot find block device").partuuid.unwrap(), mount))
        // }).into_group_map();

        // disk_parts.iter().for_each(|(disk, parts)| {
        //    let disk = gpt::disk::read_disk(disk).expect("cannot read disk");
        //       disk.partitions().values().for_each(|p| {
        //             let partuuid = p.part_guid.to_string();
        //             if let Some(mount) = parts.iter().find(|(uuid, _)| uuid::Uuid::from_str(uuid).unwrap() == p.part_guid) {
        //                 mount.gpt_type = Some(p.part_type_guid);
        //             } else {
        //                 tracing::warn!(?partuuid, "Partition with partuuid {partuuid} not found in mounts");
        //             }
        //         });
        // });
        // let blockdevs = lsblk::BlockDevice::list().expect("cannot lsblk");

        // // FIXME: is this compatible with encryptions?
        // let diskparts = self.mounts.0.iter().into_group_map(|part| {
        //     let diskname = blockdevs
        //         .iter()
        //         .find(|bd| &bd.fullname == &part.partition) // TODO: this currently assumes that the partition path is in the /dev/<disk><number> format
        //         .expect("not partition?")
        //         .disk_name()
        //         .expect("can't get disk name");
        //     (diskname, part)
        // });
        // let partuuid2mount = |partuuid: &str| mounts.0.iter_mut().find(|m| m.partition == blockdevs.iter().find(|bd| bd.partuuid.is_some_and(|x| == partuuid)).expect("can't find part by partuuid").fullname);
        // diskparts.iter().for_each(|(disk, parts)| {
        //     let disk = gpt::disk::read_disk(&disk.fullname).expect("cannot read disk");
        //     disk.partitions().values().for_each(|p| {
        //         let mount = partuuid2mount(&p.part_guid.to_string());
        //         mount.gpt_type = p.part_type_guid;
        //     });
        // });
        Ok(self.mounts.clone())
    }
}
