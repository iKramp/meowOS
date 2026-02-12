use std::{collections::{btree_map::BTreeMap, btree_set::BTreeSet}, r_lock_w_info, sync::rw_lock::RWSpinlock, w_lock_w_info};

use crate::net::{NicIdentifier, protocols::{MacAddress, arp}};

static OWN_MAC_ADDRS: RWSpinlock<BTreeSet<MacAddress>> = RWSpinlock::new(BTreeSet::new());
static MAC_TABLE: RWSpinlock<BTreeMap<MacAddress, NicIdentifier>> = RWSpinlock::new(BTreeMap::new());

static FOREIGN_ARP_TABLE: RWSpinlock<BTreeMap<arp::ProtocolAddr, arp::HardwareAddr>> = RWSpinlock::new(BTreeMap::new());
//used for mapping from self ip to self ethernet
static OWN_ARP_TABLE: RWSpinlock<BTreeMap<arp::ProtocolAddr, arp::HardwareAddr>> = RWSpinlock::new(BTreeMap::new());

pub(in crate::net) fn register_mac_address(nic_id: NicIdentifier, mac_addr: MacAddress) {
    let mut mac_table = w_lock_w_info!(MAC_TABLE);
    mac_table.insert(mac_addr, nic_id);
}

pub(in crate::net) fn register_own_mac_address(mac_addr: MacAddress) {
    w_lock_w_info!(OWN_MAC_ADDRS).insert(mac_addr);
}

pub(in crate::net) fn get_mac_nic(mac_addr: &MacAddress) -> Option<NicIdentifier> {
    r_lock_w_info!(MAC_TABLE).get(mac_addr).cloned()
}

pub(in crate::net) fn is_own_mac(mac_addr: &MacAddress) -> bool {
    r_lock_w_info!(OWN_MAC_ADDRS).contains(mac_addr)
}

pub fn update_arp_entry(hardware_addr: arp::HardwareAddr, protocol_addr: arp::ProtocolAddr) {
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
