use std::path::PathBuf;

use bytesize::ByteSize;

// use super::repartcfg::PartTypeIdent;

#[derive(Debug, Default, serde::Serialize, serde::Deserialize)]
// type should be just an array of partitions
#[serde(transparent)]
pub struct RepartOutput {
    #[serde(flatten)]
    pub partitions: Vec<RepartPartition>,
}

impl std::str::FromStr for RepartOutput {
    type Err = serde_json::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        serde_json::from_str(s)
    }
}

#[derive(Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct RepartPartition {
    // "type"
    #[serde(rename = "type")]
    part_type: String,
    label: String,
    uuid: uuid::Uuid,
    partno: i32,
    file: PathBuf,
    node: String,
    offset: usize,
    old_size: ByteSize,
    raw_size: ByteSize,
    old_padding: usize,
    raw_padding: usize,
    activity: String,
}




#[cfg(test)]
mod tests {
    use super::*;

    const OUTPUT_EXAMPLE: &str = include_str!("repart-out.json");
    
    #[test]
    fn test_deserialize() {
        let val = serde_json::from_str(OUTPUT_EXAMPLE).unwrap();
        // println!("{:#?}", val);
        let output: RepartOutput = val;
        println!("{:#?}", output);
        
        assert_eq!(output.partitions.len(), 4);
    }
}