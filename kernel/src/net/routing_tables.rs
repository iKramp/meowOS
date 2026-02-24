use std::{
    collections::btree_map::BTreeMap, error::ErrorCode, println, r_lock_w_info, sync::{arc::Arc, rw_lock::RWSpinlock}, vec::Vec, w_lock_w_info
};

use crate::net::{
    NIC, NicIdentifier,
    protocols::{MacAddress, arp, ipv4},
};

struct MacTables {
    nic_storage: BTreeMap<NicIdentifier, Arc<dyn NIC>>,
    foreign_mac_nic: Vec<(NicIdentifier, MacAddress)>,
    own_nic_mac: Vec<(NicIdentifier, MacAddress)>,
}

static MAC_BRIDGE_DOMAIN_ID_COUNTER: core::sync::atomic::AtomicU32 = core::sync::atomic::AtomicU32::new(0);
struct MacBridgeDomain {
    interfaces: Vec<(MacAddress, bool)>,
    id: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Ipv4RoutingInfo {
    interface: ipv4::Ipv4NetworkInterface,
    priority: u32,
}

impl PartialOrd for Ipv4RoutingInfo {
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Ipv4RoutingInfo {
    fn cmp(&self, other: &Self) -> core::cmp::Ordering {
        let self_prefix_len = self.interface.network.prefix_len();
        let other_prefix_len = other.interface.network.prefix_len();
        let prefix_cmp = self_prefix_len.cmp(&other_prefix_len).reverse();
        if prefix_cmp != core::cmp::Ordering::Equal {
            return prefix_cmp;
        }
        self.priority.cmp(&other.priority)
    }
}

static MAC_TABLE: RWSpinlock<MacTables> = RWSpinlock::new(MacTables {
    nic_storage: BTreeMap::new(),
    foreign_mac_nic: Vec::new(),
    own_nic_mac: Vec::new(),
});

static FOREIGN_ARP_TABLE: RWSpinlock<BTreeMap<arp::ProtocolAddr, arp::HardwareAddr>> = RWSpinlock::new(BTreeMap::new());
//used for mapping from self ip to self ethernet
static OWN_ARP_TABLE: RWSpinlock<BTreeMap<arp::ProtocolAddr, arp::HardwareAddr>> = RWSpinlock::new(BTreeMap::new());

static IPV4_ROUTING_TABLE: RWSpinlock<Vec<Ipv4RoutingInfo>> = RWSpinlock::new(Vec::new());

static MAC_BRIDGE_DOMAINS: RWSpinlock<Vec<MacBridgeDomain>> = RWSpinlock::new(Vec::new());

pub(in crate::net) fn register_foreign_mac_address(mac_addr: MacAddress, nic_id: NicIdentifier) {
    //faster path
    let res = r_lock_w_info!(MAC_TABLE).foreign_mac_nic.iter().find(|element| mac_addr == element.1).cloned();
    if let Some((res_id, _res_mac)) = res
        && res_id == nic_id
    {
        return;
    }

    let mut mac_table = w_lock_w_info!(MAC_TABLE);
    mac_table.foreign_mac_nic.retain_mut(|(id, mac)| id != &nic_id && mac != &mac_addr);
    mac_table.foreign_mac_nic.push((nic_id, mac_addr));
}

pub fn register_nic(mac_addr: MacAddress, nic: Arc<dyn NIC>) {
    let nic_id = nic.get_identifier();
    let mut table = w_lock_w_info!(MAC_TABLE);
    table.nic_storage.insert(nic_id, nic.clone());

    table.own_nic_mac.retain_mut(|(id, mac)| id != &nic_id && mac != &mac_addr);
    table.own_nic_mac.push((nic_id, mac_addr));

    let mut own_arp = w_lock_w_info!(OWN_ARP_TABLE);
    if own_arp.is_empty() {
        //assign static ip
        let protocol_addr = arp::ProtocolAddr::Ipv4(ipv4::Ipv4Address([192, 168, 178, 249]));
        let hardware_addr = arp::HardwareAddr::Ethernet(mac_addr);
        own_arp.insert(protocol_addr, hardware_addr);
    }
}

pub fn deregister_nic(nic_id: NicIdentifier) {
    let mut mac_tables = w_lock_w_info!(MAC_TABLE);
    mac_tables.nic_storage.remove(&nic_id);
    mac_tables.own_nic_mac.retain_mut(|(id, _mac)| id != &nic_id);
    mac_tables.foreign_mac_nic.retain_mut(|(id, _mac)| id != &nic_id);
}

pub(in crate::net) fn get_route_mac_nic(mac_addr: &MacAddress) -> Option<Arc<dyn NIC>> {
    let tables = r_lock_w_info!(MAC_TABLE);

    if mac_addr.is_broadcast() {
        return None; //should have called a different function for broadcast address
    }

    let nic_id = tables.foreign_mac_nic.iter().find(|element| mac_addr == &element.1)?;
    if let Some(nic) = tables.nic_storage.get(&nic_id.0) {
        Some(nic.clone())
    } else {
        drop(tables);
        let mut tables = w_lock_w_info!(MAC_TABLE);
        tables.foreign_mac_nic.retain_mut(|element| &element.1 != mac_addr);
        None
    }
}

pub(in crate::net) fn is_own_mac(mac_addr: &MacAddress) -> bool {
    r_lock_w_info!(MAC_TABLE).own_nic_mac.iter().any(|e| mac_addr == &e.1)
}

pub(in crate::net) fn get_mac_of_own_nic(nic_id: NicIdentifier) -> Option<MacAddress> {
    r_lock_w_info!(MAC_TABLE).own_nic_mac.iter().find(|e| e.0 == nic_id).map(|e| e.1)
}

pub(in crate::net) fn get_nic_of_own_mac(mac_addr: &MacAddress) -> Option<Arc<dyn NIC>> {
    let tables = r_lock_w_info!(MAC_TABLE);
    let nic_id = tables.own_nic_mac.iter().find(|element| mac_addr == &element.1)?.0;
    tables.nic_storage.get(&nic_id).cloned()
}

pub(in crate::net) fn is_own_protocol_addr(protocol_addr: &arp::ProtocolAddr) -> bool {
    r_lock_w_info!(OWN_ARP_TABLE).contains_key(protocol_addr)
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

pub fn add_ipv4_route(interface: ipv4::Ipv4NetworkInterface, priority: u32) {
    let mut routing_table = w_lock_w_info!(IPV4_ROUTING_TABLE);
    let routing_info = Ipv4RoutingInfo { interface, priority };
    let pos = routing_table.binary_search(&routing_info).unwrap_or_else(|e| e);
    routing_table.insert(pos, routing_info);
}

pub fn get_ipv4_route(destination: &ipv4::Ipv4Address) -> Option<ipv4::Ipv4Address> {
    let routing_table = r_lock_w_info!(IPV4_ROUTING_TABLE);
    for entry in routing_table.iter() {
        if entry.interface.network.contains(destination) {
            return Some(entry.interface.interface_ip.clone());
        }
    }
    None
}

pub(in crate::net) fn add_mac_bridge(domain: u32, mac: MacAddress) -> Result<(), ErrorCode> {
    let mut domains = w_lock_w_info!(MAC_BRIDGE_DOMAINS);
    //ensure it's not in any existing domain
    for domain in domains.iter_mut() {
        domain.interfaces.retain_mut(|(m, _)| m != &mac);
    }

    if let Some(domain) = domains.iter_mut().find(|d| d.id == domain) {
        domain.interfaces.push((mac, false));
        Ok(())
    } else {
        Err(ErrorCode::NoEntry)
    }
}

pub(in crate::net) fn remove_mac_from_bridge(mac: &MacAddress) {
    let mut domains = w_lock_w_info!(MAC_BRIDGE_DOMAINS);
    for domain in domains.iter_mut() {
        domain.interfaces.retain_mut(|(m, _)| m != mac);
    }
}

pub(in crate::net) fn get_mac_bridges(mac: &MacAddress) -> Vec<MacAddress> {
    let domains = r_lock_w_info!(MAC_BRIDGE_DOMAINS);
    let mut bridges = Vec::new();
    for domain in domains.iter() {
        if domain.interfaces.iter().any(|(m, _)| m == mac) {
            for (m, enabled) in domain.interfaces.iter() {
                if m != mac && *enabled {
                    bridges.push(*m);
                }
            }
        }
    }
    bridges
}
