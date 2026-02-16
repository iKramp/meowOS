use std::{collections::btree_map::BTreeMap, println, r_lock_w_info, sync::{arc::Arc, rw_lock::RWSpinlock}, vec::Vec, w_lock_w_info};

use crate::net::{
    NIC, NicIdentifier, protocols::{MacAddress, arp}
};

struct MacTables {
    nic_storage: BTreeMap<NicIdentifier, Arc<dyn NIC>>,
    mac_to_nic: BTreeMap<MacAddress, NicIdentifier>,
    own_nics: BTreeMap<MacAddress, NicIdentifier>,
}

static MAC_TABLE: RWSpinlock<MacTables> = RWSpinlock::new(MacTables {
    nic_storage: BTreeMap::new(),
    mac_to_nic: BTreeMap::new(),
    own_nics: BTreeMap::new(),
});

static FOREIGN_ARP_TABLE: RWSpinlock<BTreeMap<arp::ProtocolAddr, arp::HardwareAddr>> = RWSpinlock::new(BTreeMap::new());
//used for mapping from self ip to self ethernet
static OWN_ARP_TABLE: RWSpinlock<BTreeMap<arp::ProtocolAddr, arp::HardwareAddr>> = RWSpinlock::new(BTreeMap::new());

pub(in crate::net) fn register_foreign_mac_address(mac_addr: MacAddress, nic_id: NicIdentifier) {
    //faster path
    let res = r_lock_w_info!(MAC_TABLE).mac_to_nic.get(&mac_addr).cloned();
    if let Some(res_id) = res && res_id == nic_id {
        return;
    }

    w_lock_w_info!(MAC_TABLE).mac_to_nic.insert(mac_addr, nic_id);
}

pub fn register_nic(mac_addr: MacAddress, nic: Arc<dyn NIC>) {
    let nic_id = nic.get_identifier();
    let mut table = w_lock_w_info!(MAC_TABLE);
    table.nic_storage.insert(nic_id, nic.clone());
    table.own_nics.insert(mac_addr, nic_id);

    let mut own_arp = w_lock_w_info!(OWN_ARP_TABLE);
    if own_arp.is_empty() {
        //assign static ip
        let protocol_addr = arp::ProtocolAddr::Ipv4([192, 168, 178, 249]);
        let hardware_addr = arp::HardwareAddr::Ethernet(mac_addr);
        own_arp.insert(protocol_addr, hardware_addr);
    }
}

pub fn deregister_nic(nic_id: NicIdentifier) {
    let mut mac_tables = w_lock_w_info!(MAC_TABLE);
    mac_tables.nic_storage.remove(&nic_id);
}

pub(in crate::net) fn get_broadcast_nices(in_nic_id: Option<NicIdentifier>) -> Vec<Arc<dyn NIC>> {
    let in_nic_id = in_nic_id.unwrap_or(u32::MAX);
    r_lock_w_info!(MAC_TABLE).nic_storage.values().filter(|nic| matches!(nic.nic_type(), crate::net::NicType::Ethernet) && nic.get_identifier() != in_nic_id).cloned().collect()
}

pub(in crate::net) fn get_mac_nic(mac_addr: &MacAddress) -> Option<Arc<dyn NIC>> {
    let tables = r_lock_w_info!(MAC_TABLE);

    if mac_addr.is_broadcast() {
        return None; //should have called a different function for broadcast address
    }

    let nic_id = tables.mac_to_nic.get(mac_addr)?;
    if let Some(nic) = tables.nic_storage.get(nic_id) {
        Some(nic.clone())
    } else {
        drop(tables);
        let mut tables = w_lock_w_info!(MAC_TABLE);
        tables.mac_to_nic.remove(mac_addr)?;
        None
    }
}

pub(in crate::net) fn is_own_mac(mac_addr: &MacAddress) -> bool {
    r_lock_w_info!(MAC_TABLE).own_nics.contains_key(mac_addr)
}

pub fn update_arp_entry(hardware_addr: arp::HardwareAddr, protocol_addr: arp::ProtocolAddr) {
    println!("Updating ARP entry: {:?} -> {:?}", protocol_addr, hardware_addr);
    w_lock_w_info!(FOREIGN_ARP_TABLE).insert(protocol_addr, hardware_addr);
}

pub fn get_arp_entry(protocol_addr: &arp::ProtocolAddr) -> Option<arp::HardwareAddr> {
    r_lock_w_info!(FOREIGN_ARP_TABLE).get(protocol_addr).cloned()
}

pub fn update_self_arp_entry(hardware_addr: arp::HardwareAddr, protocol_addr: arp::ProtocolAddr) {
    w_lock_w_info!(OWN_ARP_TABLE).insert(protocol_addr, hardware_addr);
}

pub fn get_self_arp_entry(protocol_addr: &arp::ProtocolAddr) -> Option<arp::HardwareAddr> {
    r_lock_w_info!(OWN_ARP_TABLE).get(protocol_addr).cloned()
}
