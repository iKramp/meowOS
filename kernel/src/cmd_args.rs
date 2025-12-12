use core::str::FromStr;
use uuid::{self, Uuid};

#[derive(Debug)]
pub struct CmdArgs {
    pub root_partition: Uuid,
}

impl CmdArgs {
    pub fn new(arg_str: &str) -> Self {
        let args = arg_str.split_whitespace();
        let mut root_partition = None;
        for arg in args {
            // let (key, value) = arg.split_at(arg.find("=").unwrap());
            let Some(pos) = arg.find('=') else {
                continue;
            };
            let (key, value) = arg.split_at(pos);
            if key == "root" {
                //value is in uuid format
                let uuid = Uuid::from_str(&value[1..]).expect("Invalid UUID format for root partition");
                root_partition = Some(uuid);
            }
        }

        Self {
            root_partition: root_partition.expect("Root partition not specified in cmd args"),
        }
    }
}
