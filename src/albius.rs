use std::path::PathBuf;

use serde::{Deserialize, Serialize};

// todo: rewrite this shit and kill albius

#[derive(Debug, Serialize, Deserialize)]
pub struct Recipe {
    pub setup: Vec<DiskOperation>,
    pub mountpoints: Vec<Mountpoint>,
    pub installation: Installation,
    pub post_installation: Vec<PostInstallaion>,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum DiskOperationType {
    Label,
    Mkpart,
    Rm,
    Resizepart,
    Namepart,
    Setflag,
    Format,
    LuksFormat,
    PvCreate,
    PvResize,
    PvRemove,
    VgCreate,
    VgRename,
    VgExtend,
    VgReduce,
    VgRemove,
    LvCreate,
    LvRename,
    LvRemove,
    MakeThinPool,
    LvCreatePool,
    LvmFormat,
    LvmLuksFormat,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DiskOperation {
    pub disk: PathBuf,
    pub operation: DiskOperationType,
    pub params: Vec<String>,
}

pub struct Mountpoint {
  pub partition: PathBuf,
  pub mountpoint: PathBuf,
}

pub struct Installation {

}