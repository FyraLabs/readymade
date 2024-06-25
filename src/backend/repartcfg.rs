use std::env::consts::ARCH;

//* https://www.freedesktop.org/software/systemd/man/latest/repart.d.html

#[derive(serde::Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct RepartConfig {
    partition: Partition,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct Partition {
    r#type: PartTypeIdent
}

#[derive(serde_enum_str::Deserialize_enum_str, serde_enum_str::Serialize_enum_str, Default)]
#[serde(rename_all = "kebab-case")]
pub enum PartTypeIdent {
    Esp,
    Xbootldr,
    Swap,
    Home,
    Srv,
    Var,
    Tmp,
    #[default]
    LinuxGeneric,
    Root,
    RootVerity,
    RootVeritySig,
    RootSecondary,
    RootSecondaryVerity,
    RootSecondaryVeritySig,
    // #[serde(rename = concat!("root-", ARCH))]
    RootArch,
    // #[serde(rename = concat!("root-", ARCH, "-verity"))]
    RootArchVerity,
    // #[serde(rename = concat!("root-", ARCH, "-verity-sig"))]
    RootArchVeritySig,
    Usr,
    UsrVerity,
    UsrVeritySig,
    UsrSecondary,
    UsrSecondaryVerity,
    UsrSecondaryVeritySig,
    // #[serde(rename = concat!("root-", ARCH))]
    UsrArch,
    // #[serde(rename = concat!("root-", ARCH, "-verity"))]
    UsrArchVerity,
    // #[serde(rename = concat!("root-", ARCH, "-verity-sig"))]
    UsrArchVeritySig,
    #[serde(other)]
    Others(String)
}

const ROOTARCH = concat!("root-", ARCH);
const

impl std::borrow::Borrow<str> for PartTypeIdent {
    fn borrow(&self) -> &str {
        if let Self::Others(s) = self {
            match s {
                ROOTARCH => {

                }
            }
        }
    }
}
