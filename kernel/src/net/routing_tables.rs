use std::{collections::{btree_map::BTreeMap, btree_set::BTreeSet}, r_lock_w_info, sync::rw_lock::RWSpinlock, w_lock_w_info};

use crate::net::{NicIdentifier, protocols::MacAddress};


static OWN_MAC_ADDRS: RWSpinlock<BTreeSet<MacAddress>> = RWSpinlock::new(BTreeSet::new());
static MAC_TABLE: RWSpinlock<BTreeMap<MacAddress, NicIdentifier>> = RWSpinlock::new(BTreeMap::new());

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
