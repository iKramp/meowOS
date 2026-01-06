use core::ops::Add;
use std::{error::ErrorCode, format, print, println};

static DEVICE_CODES: &str = include_str!("../../../../assets/pci.ids");

#[derive(Debug)]
pub(super) struct DeviceIdentification {
    pub vendor_id: u16,
    pub device_id: u16,
    pub subvendor_id: u16,
    pub subdevice_id: u16,
    pub vendor_name: &'static str,
    pub device_name: &'static str,
    pub subsystem_name: &'static str,
}

impl DeviceIdentification {
    pub fn new(vendor_id: u16, device_id: u16, subvendor_id: u16, subdevice_id: u16) -> Self {
        Self {
            vendor_id,
            device_id,
            subvendor_id,
            subdevice_id,
            vendor_name: "",
            device_name: "",
            subsystem_name: "",
        }
    }
}

pub fn get_device_identification(id_struct: &mut DeviceIdentification) {
    let _res = get_device_identification_inner(id_struct); //it's fine if we get Err, not all
    //devices have subsystems
    print!("@BOTH");
}

fn get_device_identification_inner(id_struct: &mut DeviceIdentification) -> Result<(), ErrorCode> {
    let vendor_str = format!("{:x}", id_struct.vendor_id);
    let device_str = format!("{:x}", id_struct.device_id);
    let subsystem_str = format!("{:x} {:x}", id_struct.subvendor_id, id_struct.subdevice_id);

    print!("@DBG");
    println!("vendor str: {}", vendor_str);
    println!("device str: {}", device_str);
    println!("subsystem str: {}", subsystem_str);

    let file_lines = DEVICE_CODES.split('\n').filter(|line| !line.starts_with("#"));
    let lines_total = file_lines.clone().count();
    println!("get_device_identification: lines total {:#X}", lines_total);

    let vendor_line = file_lines
        .clone()
        .position(|line| line.starts_with(&vendor_str))
        .ok_or(ErrorCode::NoEntry)?;
    println!("get_device_identification: vendor line {:#X}", vendor_line);
    let vendor_str = &file_lines.clone().nth(vendor_line).expect("??")[6..];
    id_struct.vendor_name = vendor_str;

    let next_vendor_line = file_lines
        .clone()
        .skip(vendor_line + 1)
        .position(|line| line.len() > 3 && !line.chars().next().expect("?").is_whitespace())
        .unwrap_or(lines_total - vendor_line)
        .add(vendor_line + 1);
    println!("get_device_identification: next vendor line {:#X}", next_vendor_line);

    let device_line = file_lines
        .clone()
        .skip(vendor_line + 1)
        .position(|line| line.trim().starts_with(&device_str))
        .ok_or(ErrorCode::NoEntry)?
        .add(vendor_line + 1);
    println!("get_device_identification: device line {:#X}", device_line);
    if device_line > next_vendor_line {
        return Err(ErrorCode::NoEntry);
    }
    let device_str = &file_lines.clone().nth(device_line).expect("??")[7..];
    id_struct.device_name = device_str;

    let next_device_line = file_lines
        .clone()
        .skip(device_line + 1)
        .position(|line| line.len() > 4 && line.chars().nth(1).expect("?") != '\t' && line.starts_with('\t'))
        .unwrap_or(lines_total - device_line)
        .add(device_line + 1);
    println!("get_device_identification: next device line {:#X}", next_device_line);

    let subsystem_line = file_lines
        .clone()
        .skip(device_line + 1)
        .position(|line| line.trim().starts_with(&subsystem_str))
        .ok_or(ErrorCode::NoEntry)?
        .add(device_line + 1);
    println!("get_device_identification: subsystem line {:#X}", subsystem_line);
    if subsystem_line > next_device_line {
        return Err(ErrorCode::NoEntry);
    }
    let subsystem_str = &file_lines.clone().nth(subsystem_line).expect("??")[13..];
    id_struct.subsystem_name = subsystem_str;

    Ok(())
}
