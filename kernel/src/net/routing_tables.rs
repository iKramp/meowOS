use core::sync::atomic::AtomicBool;
use std::{
    collections::btree_map::BTreeMap,
    error::KernelError,
    kerror, println, r_lock_w_info,
    sync::{arc::Arc, rw_lock::RWSpinlock},
    vec::Vec,
    w_lock_w_info,
};

use crate::net::{
    NIC, NicIdentifier, NicType, ProtocolAddr,
    protocols::{
        self, MacAddress, arp,
        ipv4::{self, Ipv4Address, Ipv4Network},
    },
};

const DEFAULT_IP: [u8; 4] = [10, 0, 0, 2];

struct MacTables {
    nic_storage: BTreeMap<NicIdentifier, Arc<dyn NIC>>,
    foreign_mac_nic: Vec<(NicIdentifier, MacAddress)>,
}

static MAC_BRIDGE_DOMAIN_ID_COUNTER: core::sync::atomic::AtomicU32 = core::sync::atomic::AtomicU32::new(0);
struct MacBridgeDomain {
    interfaces: Vec<(MacAddress, bool)>,
    id: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(in crate::net) struct Ipv4Route {
    pub network: ipv4::Ipv4Network, //local interface ip and mask
    pub first_hop_ip: ipv4::Ipv4Address,
    pub local_interface_ip: Ipv4Address,
    pub priority: u32,
}

impl PartialOrd for Ipv4Route {
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Ipv4Route {
    fn cmp(&self, other: &Self) -> core::cmp::Ordering {
        let prefix_ord = self.network.cmp(&other.network);
        if prefix_ord != core::cmp::Ordering::Equal {
            return prefix_ord;
        }
        self.priority.cmp(&other.priority)
    }
}

static MAC_TABLE: RWSpinlock<MacTables> = RWSpinlock::new(MacTables {
    nic_storage: BTreeMap::new(),
    foreign_mac_nic: Vec::new(),
});

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(in crate::net) struct ArpKey {
    foreign_protocol: arp::ProtocolAddr,
    self_hardware: arp::HardwareAddr,
}

static FOREIGN_ARP_TABLE: RWSpinlock<BTreeMap<ArpKey, arp::HardwareAddr>> = RWSpinlock::new(BTreeMap::new());

static IPV4_ROUTING_TABLE: RWSpinlock<(Vec<Ipv4Network>, Vec<Ipv4Route>)> = RWSpinlock::new((Vec::new(), Vec::new()));

static MAC_BRIDGE_DOMAINS: RWSpinlock<Vec<MacBridgeDomain>> = RWSpinlock::new(Vec::new());

type NicInfo = (Vec<protocols::NetAddress>, NicType);
static NIC_INFO: RWSpinlock<Vec<(NicIdentifier, NicInfo)>> = RWSpinlock::new(Vec::new());

static FIRST_IP_ADDED: AtomicBool = AtomicBool::new(false);

pub(in crate::net) fn register_foreign_mac_address(mac_addr: MacAddress, nic_id: NicIdentifier) {
    //faster path
    let res = r_lock_w_info!(MAC_TABLE)
        .foreign_mac_nic
        .iter()
        .find(|element| mac_addr == element.1)
        .cloned();
    if let Some((res_id, _res_mac)) = res
        && res_id == nic_id
    {
        return;
    }

    let mut mac_table = w_lock_w_info!(MAC_TABLE);
    mac_table
        .foreign_mac_nic
        .retain_mut(|(id, mac)| id != &nic_id && mac != &mac_addr);
    mac_table.foreign_mac_nic.push((nic_id, mac_addr));
}

pub fn register_nic(mac_addr: MacAddress, nic: Arc<dyn NIC>) {
    println!("Registering NIC with MAC {:?} and ID {:?}", mac_addr, nic.get_identifier());
    let nic_id = nic.get_identifier();
    let mut table = w_lock_w_info!(MAC_TABLE);
    table.nic_storage.insert(nic_id, nic.clone());

    let mut nic_addresses = w_lock_w_info!(NIC_INFO);
    nic_addresses.iter_mut().for_each(|elem| {
        elem.1.0.retain_mut(|addr| addr != &protocols::NetAddress::Mac(mac_addr));
        if elem.0 == nic_id {
            elem.1.0.push(protocols::NetAddress::Mac(mac_addr));
        }
    });
    nic_addresses.retain_mut(|elem| elem.0 != nic_id);

    let mut nic_address_vec = Vec::new();
    nic_address_vec.push(protocols::NetAddress::Mac(mac_addr));

    //temporary
    if !FIRST_IP_ADDED.swap(true, core::sync::atomic::Ordering::SeqCst) {
        let interface_network = ipv4::Ipv4Network {
            address: ipv4::Ipv4Address(DEFAULT_IP),
            mask: ipv4::Ipv4Address([255, 255, 255, 0]),
        };

        nic_address_vec.push(protocols::NetAddress::Ipv4Network(interface_network.clone()));

        let default_network = ipv4::Ipv4Network {
            //all goes here
            address: ipv4::Ipv4Address([0, 0, 0, 0]),
            mask: ipv4::Ipv4Address([0, 0, 0, 0]),
        };

        w_lock_w_info!(IPV4_ROUTING_TABLE).0.push(interface_network.clone());
        w_lock_w_info!(IPV4_ROUTING_TABLE).1.push(Ipv4Route {
            network: default_network,
            first_hop_ip: ipv4::Ipv4Address([10, 0, 0, 1]), //tap0 addr
            local_interface_ip: interface_network.address,
            priority: 100,
        });
    }

    nic_addresses.push((nic_id, (nic_address_vec, nic.nic_type())));
}

pub fn deregister_nic(nic_id: NicIdentifier) {
    let mut mac_tables = w_lock_w_info!(MAC_TABLE);
    mac_tables.nic_storage.remove(&nic_id);
    mac_tables.foreign_mac_nic.retain_mut(|(id, _mac)| id != &nic_id);

    w_lock_w_info!(NIC_INFO).retain_mut(|(id, _addrs)| id != &nic_id);
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
    r_lock_w_info!(NIC_INFO)
        .iter()
        .any(|(_id, info)| info.0.iter().any(|addr| addr == &protocols::NetAddress::Mac(*mac_addr)))
}

pub(in crate::net) fn get_mac_of_own_nic(nic_id: NicIdentifier) -> Option<MacAddress> {
    r_lock_w_info!(NIC_INFO).iter().find_map(|(id, info)| {
        if id == &nic_id {
            info.0.iter().find_map(|addr| {
                if let protocols::NetAddress::Mac(mac) = addr {
                    Some(*mac)
                } else {
                    None
                }
            })
        } else {
            None
        }
    })
}

pub(in crate::net) fn get_nic_of_own_address(net_addr: &protocols::NetAddress) -> Option<Arc<dyn NIC>> {
    r_lock_w_info!(NIC_INFO).iter().find_map(|(id, addrs)| {
        if addrs.0.iter().any(|addr| addr == net_addr) {
            r_lock_w_info!(MAC_TABLE).nic_storage.get(id).cloned()
        } else {
            None
        }
    })
}

pub(in crate::net) fn get_nic_info_from_own_addr(
    net_addr: &protocols::NetAddress,
) -> Vec<(NicIdentifier, (Vec<protocols::NetAddress>, NicType))> {
    r_lock_w_info!(NIC_INFO)
        .iter()
        .filter(|(_id, (addrs, _nic_type))| addrs.iter().any(|addr| addr == net_addr))
        .cloned()
        .collect()
}

pub(in crate::net) fn get_nic_from_id(nic_id: &NicIdentifier) -> Option<Arc<dyn NIC>> {
    r_lock_w_info!(MAC_TABLE).nic_storage.get(nic_id).cloned()
}

pub(in crate::net) fn get_nic_addresses_from_id(nic_id: &NicIdentifier) -> Option<Vec<protocols::NetAddress>> {
    r_lock_w_info!(NIC_INFO)
        .iter()
        .find(|(id, _addrs)| id == nic_id)
        .map(|(_id, (addrs, _nic_type))| addrs.clone())
}

pub fn update_arp_entry(
    foreign_hardware_addr: arp::HardwareAddr,
    self_hardware_addr: arp::HardwareAddr,
    protocol_addr: arp::ProtocolAddr,
) {
    println!("Updating ARP entry: {:?} -> {:?}", protocol_addr, foreign_hardware_addr);
    w_lock_w_info!(FOREIGN_ARP_TABLE).insert(
        ArpKey {
            foreign_protocol: protocol_addr,
            self_hardware: self_hardware_addr,
        },
        foreign_hardware_addr,
    );
}

pub fn get_arp_entry(protocol_addr: arp::ProtocolAddr, self_hardware_addr: arp::HardwareAddr) -> Option<arp::HardwareAddr> {
    r_lock_w_info!(FOREIGN_ARP_TABLE)
        .get(&ArpKey {
            foreign_protocol: protocol_addr,
            self_hardware: self_hardware_addr,
        })
        .cloned()
}

pub fn add_ipv4_route(network: ipv4::Ipv4Network) {
    let mut routing_table = w_lock_w_info!(IPV4_ROUTING_TABLE);
    let pos = routing_table.0.binary_search(&network).unwrap_or_else(|e| e);
    routing_table.0.insert(pos, network);
}

pub fn add_default_ipv4_route(network: ipv4::Ipv4Network, first_hop_ip: Ipv4Address, local_ip: Ipv4Address, priority: u32) {
    let mut routing_table = w_lock_w_info!(IPV4_ROUTING_TABLE);
    let info = Ipv4Route {
        network,
        first_hop_ip,
        local_interface_ip: local_ip,
        priority,
    };
    let pos = routing_table.1.binary_search(&info).unwrap_or_else(|e| e);
    routing_table.1.insert(pos, info);
}

pub(in crate::net) fn get_ipv4_route(destination: &ipv4::Ipv4Address) -> Option<Ipv4Route> {
    let routing_table = r_lock_w_info!(IPV4_ROUTING_TABLE);
    //first try to find a direct route
    for network in routing_table.0.iter() {
        if network.contains(destination) {
            println!("returning interface network: {:?}", network);
            return Some(Ipv4Route {
                network: network.clone(),
                first_hop_ip: *destination,
                local_interface_ip: network.address,
                priority: 0,
            });
        }
    }
    //then try to find a default route
    for info in routing_table.1.iter() {
        if info.network.contains(destination) {
            println!("returning default route: {:?}", info);
            return Some(info.clone());
        }
    }
    None
}

pub(in crate::net) fn get_own_ipv4_mac(own_ipv4: &Ipv4Address) -> Option<MacAddress> {
    let nic_addresses = r_lock_w_info!(NIC_INFO);
    for (_id, (addrs, _nic_type)) in nic_addresses.iter() {
        if addrs
            .iter()
            .any(|addr| addr.clone().into_protocol() == Some(ProtocolAddr::Ipv4(*own_ipv4)))
        {
            for addr in addrs.iter() {
                if let protocols::NetAddress::Mac(mac) = addr {
                    return Some(*mac);
                }
            }
        }
    }
    None
}

pub(in crate::net) fn add_mac_bridge(domain: u32, mac: MacAddress) -> Result<(), KernelError> {
    let mut domains = w_lock_w_info!(MAC_BRIDGE_DOMAINS);
    //ensure it's not in any existing domain
    for domain in domains.iter_mut() {
        domain.interfaces.retain_mut(|(m, _)| m != &mac);
    }

    if let Some(domain) = domains.iter_mut().find(|d| d.id == domain) {
        domain.interfaces.push((mac, false));
        Ok(())
    } else {
        kerror!(NoEntry)
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
