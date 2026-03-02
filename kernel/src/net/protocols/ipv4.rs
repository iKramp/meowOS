use std::{cow::Acow, println, vec::Vec, w_lock_w_info};

use crate::net::{
    self, NetLayerType, NetPacketListNode, NetPacketSource, NicType, ProtocolAddr,
    address_pair::AddressPair,
    flow::{LayerDownType, OutgoingFlowDirection},
    hook::HookResult,
    packet::NetPacket,
    protocols::{NetAddress, NetLayer, NetLayerFlowID, arp::HardwareAddr, icmp},
    routing_tables,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::net) struct Ipv4FragmentInfo {
    pub identification: u16,
    pub flags: Ipv4Flags,
    pub fragment_offset: u32,
}

#[derive(Debug, Clone)]
pub(in crate::net) struct Ipv4Header {
    offset: u32,
    pub ihl: u8,
    diff_services: u8,
    total_length: u16,
    fragment_info: Ipv4FragmentInfo,
    ttl: u8,
    protocol: u8,
    header_checksum: u16,
    pub address: AddressPair<Ipv4Address>,
    pub in_interface_networks: Vec<Ipv4Network>,
    checksum_checked: bool,
}

bitfield::bitfield! {
    #[derive(PartialEq, Eq)]
    pub(in crate::net) struct Ipv4Flags(u8);
    impl Debug;
    pub reserved, set_reserved: 2;
    pub dont_fragment, set_dont_fragment: 1;
    pub more_fragments, set_more_fragments: 0;
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(in crate::net) struct Ipv4FlowId {
    pub address: AddressPair<Ipv4Address>,
    pub protocol: u8,
    pub diff_services: u8,
    pub ttl: u8,
    pub fragment_info: Ipv4FragmentInfo,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(in crate::net) struct Ipv4Address(pub [u8; 4]);

impl Ipv4Address {
    pub fn is_multicast(&self) -> bool {
        (self.0[0] & 0xF0) == 0xE0
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(in crate::net) struct Ipv4Network {
    pub address: Ipv4Address,
    pub mask: Ipv4Address,
}

impl Ord for Ipv4Network {
    fn cmp(&self, other: &Self) -> core::cmp::Ordering {
        let self_prefix_len = self.prefix_len();
        let other_prefix_len = other.prefix_len();
        self_prefix_len.cmp(&other_prefix_len).reverse()
    }
}

impl PartialOrd for Ipv4Network {
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ipv4Network {
    pub fn contains(&self, ip: &Ipv4Address) -> bool {
        let ip_u32 = u32::from_be_bytes(ip.0);
        let network_u32 = u32::from_be_bytes(self.address.0);
        let mask_u32 = u32::from_be_bytes(self.mask.0);
        (ip_u32 & mask_u32) == (network_u32 & mask_u32)
    }

    pub fn broadcast_for(&self, ip: &Ipv4Address) -> bool {
        let ip_u32 = u32::from_be_bytes(ip.0);
        let network_u32 = u32::from_be_bytes(self.address.0);
        let mask_u32 = u32::from_be_bytes(self.mask.0);
        ip_u32 == (network_u32 | !mask_u32)
    }

    pub fn prefix_len(&self) -> u32 {
        let mask_u32 = u32::from_be_bytes(self.mask.0);
        mask_u32.count_ones()
    }
}

enum Ipv4Error {
    NetworkUnreachable,
    HostUnreachable,
    OtherNonFatal,
    OtherFatal, //net stream must be closed
}

pub(super) fn init() {
    w_lock_w_info!(net::hook::NET_HOOK_STORAGE).register_hook(ipv4_bridge_hook, net::hook::HookStage::Bridge(NetLayerType::Ipv4));
}

fn ipv4_bridge_hook(packet: &mut Acow<NetPacket>) -> HookResult {
    let Some(ipv4_layer) = packet
        .get_highest_layer()
        .and_then(|layer| (layer as &dyn std::any::Any).downcast_ref::<Ipv4Header>())
    else {
        return HookResult::Drop;
    };

    if ipv4_layer.ttl == 1 {
        //ICMP TTL
        icmp::send_icmp_error(packet, 11, 0, 0);

        return HookResult::Drop; //drop packets that would expire after bridging
    }

    //NAT here

    HookResult::Nothing
}

impl NetLayer for Ipv4Header {
    fn incoming_flow_direction(&self) -> crate::net::flow::IncomingFlowDirection {
        let matching_network = self
            .in_interface_networks
            .iter()
            .find(|net| net.contains(&self.address.target));
        if matching_network.is_some() {
            //is for us
            crate::net::flow::IncomingFlowDirection::LayerUp(self.upper_layer_type(), self.upper_layer_offset() as usize)
        } else {
            crate::net::flow::IncomingFlowDirection::Bridge
        }
    }

    fn current_layer_type(&self) -> NetLayerType {
        NetLayerType::Ipv4
    }

    fn current_layer_offset(&self) -> u32 {
        self.offset
    }

    fn upper_layer_type(&self) -> NetLayerType {
        match self.protocol {
            1 => NetLayerType::Icmp,    //ICMP, not yet supported
            2 => NetLayerType::Unknown, //IGMP, not yet supported
            6 => NetLayerType::Tcp,
            17 => NetLayerType::Udp,
            41 => NetLayerType::Ipv6,
            89 => NetLayerType::Unknown,  //OSPF, not yet supported
            132 => NetLayerType::Unknown, //SCTP, not yet supported
            _ => NetLayerType::Unknown,
        }
    }

    fn upper_layer_offset(&self) -> u32 {
        self.offset + self.ihl as u32 * 4
    }

    fn bridge_to_out_set_layers(&self, out_layers: &mut std::vec::Vec<super::NetLayerFlowID>) {
        let flow_id = Ipv4FlowId {
            address: self.address.clone(),
            protocol: self.protocol,
            diff_services: self.diff_services,
            ttl: self.ttl - 1,
            fragment_info: self.fragment_info.clone(),
        };

        out_layers.push(super::NetLayerFlowID::Ipv4(flow_id));
    }
}

pub(super) fn parse_ipv4_packet(packet: &mut Acow<NetPacket>, offset: usize) -> Option<Ipv4Header> {
    let packet_len = packet.len();
    let offset = offset as u32;
    if packet_len < offset + 1 {
        return None;
    }
    packet.ensure_length(offset + 1);

    let v_ihl = packet.get_chunks()[0].data()[offset as usize];
    if v_ihl >> 4 != 4 {
        return None; // Not IPv4
    }

    let header_len = ((v_ihl & 0x0F) as u32) * 4;

    if packet_len < header_len {
        return None;
    }

    let checksum = net::compute_internet_checksum(&packet.get_chunks()[0].data()[offset as usize..(offset + header_len) as usize]);
    if checksum != 0 {
        println!(level:warn, "IPv4 header checksum mismatch, expected 0 got {:04x}, dropping packet", checksum);
        return None;
    }

    packet.ensure_length(header_len);
    let data = &packet.get_chunks()[0].data()[offset as usize..];
    let differentated_services = data[1] >> 2;
    let total_length = u16::from_be_bytes([data[2], data[3]]);
    let identification = u16::from_be_bytes([data[4], data[5]]);
    let flags = Ipv4Flags((data[6] & 0xE0) >> 5);
    let fragment_offset = ((u16::from_be_bytes([data[6], data[7]])) & 0x1FFF) as u32;
    let ttl = data[8];
    let protocol = data[9];
    let header_checksum = u16::from_be_bytes([data[10], data[11]]);
    let source = Ipv4Address([data[12], data[13], data[14], data[15]]);
    let destination = Ipv4Address([data[16], data[17], data[18], data[19]]);
    //ignore potential options

    let upper_layer_size = total_length as u32 - header_len;
    packet.upper_layer_size = Some(upper_layer_size as usize);

    let mut interface_addresses = match packet.source {
        crate::net::NetPacketSource::Nic(nic_id) => {
            if let Some(addresses) = routing_tables::get_nic_addresses_from_id(&nic_id) {
                addresses
                    .into_iter()
                    .filter_map(|addr| {
                        if let NetAddress::Ipv4(ipv4_addr) = addr {
                            Some(ipv4_addr)
                        } else {
                            None
                        }
                    })
                    .collect()
            } else {
                Vec::new()
            }
        }
        _ => Vec::new(),
    };

    interface_addresses.sort();

    Some(Ipv4Header {
        offset,
        ihl: v_ihl & 0x0F,
        diff_services: differentated_services,
        total_length,
        fragment_info: Ipv4FragmentInfo {
            identification,
            flags,
            fragment_offset,
        },
        ttl,
        protocol,
        header_checksum,
        address: AddressPair::new(source, destination),
        checksum_checked: false,
        in_interface_networks: interface_addresses,
    })
}

fn write_data_to_packet(packet: &mut [u8], data: Ipv4FlowId, offset: usize, upper_layers_length: usize) {
    let Ipv4FlowId {
        address: AddressPair { source, target },
        protocol,
        diff_services,
        ttl,
        fragment_info: Ipv4FragmentInfo {
            identification,
            flags,
            fragment_offset,
        },
    } = data;

    let total_length = (upper_layers_length + 20) as u16;

    packet[offset] = (4 << 4) | 5; //version and IHL
    packet[offset + 1] = diff_services << 2;
    packet[offset + 2..offset + 4].copy_from_slice(&total_length.to_be_bytes());
    packet[offset + 4..offset + 6].copy_from_slice(&identification.to_be_bytes());
    packet[offset + 6..offset + 8].copy_from_slice(&(((flags.0 as u16) << 13) | (fragment_offset as u16)).to_be_bytes());
    packet[offset + 8] = ttl;
    packet[offset + 9] = protocol;
    packet[offset + 10..offset + 12].copy_from_slice(&0u16.to_be_bytes()); //checksum placeholder
    packet[offset + 12..offset + 16].copy_from_slice(&source.0);
    packet[offset + 16..offset + 20].copy_from_slice(&target.0);

    println!("computing ipv4 checksum");
    let checksum = net::compute_internet_checksum(&packet[offset..offset + 20]);
    packet[offset + 10..offset + 12].copy_from_slice(&checksum.to_be_bytes());
}

pub(super) fn construct_layer(packet: &mut Acow<NetPacket>) -> OutgoingFlowDirection {
    match construct_layer_internal(packet) {
        Ok(direction) => direction,
        Err(err) => {
            match err {
                Ipv4Error::NetworkUnreachable => {
                    println!(level:error, "construct_layer for IPv4 failed: Network Unreachable");

                    match packet.source {
                        NetPacketSource::Nic(_) => unreachable!("packets bridged from nices should have OtherPacket source"),
                        NetPacketSource::Proc(_pid) => todo!(),
                        NetPacketSource::OtherPacket(ref mut og_packet) => {
                            icmp::send_icmp_error(og_packet, 3, 0, 0);
                        }
                        NetPacketSource::Other => {}
                    }
                }
                Ipv4Error::HostUnreachable => {
                    println!(level:error, "construct_layer for IPv4 failed: Host Unreachable");

                    match packet.source {
                        NetPacketSource::Nic(_) => unreachable!("packets bridged from nices should have OtherPacket source"),
                        NetPacketSource::Proc(_pid) => todo!(),
                        NetPacketSource::OtherPacket(ref mut og_packet) => {
                            icmp::send_icmp_error(og_packet, 3, 1, 0);
                        }
                        NetPacketSource::Other => {}
                    }
                }
                Ipv4Error::OtherNonFatal => {}
                Ipv4Error::OtherFatal => {
                    todo!(
                        "handle fatal error in construct_layer for IPv4, currently just drops packet but should also close stream if applicable"
                    );
                }
            }
            OutgoingFlowDirection::Drop
        }
    }
}

fn construct_layer_internal(packet: &mut Acow<NetPacket>) -> Result<OutgoingFlowDirection, Ipv4Error> {
    let Some(NetLayerFlowID::Ipv4(data)) = packet.layers_to_construct.pop() else {
        println!(level:error, "construct_layer called for IPv4 but highest layer is not Ipv4FlowId");
        return Err(Ipv4Error::OtherNonFatal); //error in some driver, no need to break any stream
    };

    let out_flow_direction: OutgoingFlowDirection;

    if packet.layers_to_construct.is_empty() {
        println!("out ipv4 packet doesn't have lower layer set");
        println!("target is: {:?}", &data.address.target);
        let Some(route) = routing_tables::get_ipv4_route(&data.address.target) else {
            println!(level:error, "construct_layer for IPv4 failed to find route for target {:?}", data.address.target);
            return Err(Ipv4Error::NetworkUnreachable);
        };

        let Some(own_mac) = routing_tables::get_own_ipv4_mac(&route.network.address) else {
            println!(level:error, "construct_layer for IPv4 failed to get own MAC for interface IP {:?}", route.network.address);
            return Err(Ipv4Error::OtherFatal); //rare enough to not need a specific error type
        };

        println!("next hop via: {:?}", route.first_hop_ip);
        let Some(remote_hardware) =
            routing_tables::get_arp_entry(ProtocolAddr::Ipv4(route.first_hop_ip), HardwareAddr::Ethernet(own_mac))
        else {
            println!(level:warn, "construct_layer for IPv4 failed to find ARP entry for next hop {:?}, dropping packet", route.first_hop_ip);
            if route.first_hop_ip == data.address.target {
                return Err(Ipv4Error::HostUnreachable);
            } else {
                return Err(Ipv4Error::NetworkUnreachable);
            }
        };

        let Some(nic_info) = routing_tables::get_nic_info_from_own_addr(&NetAddress::Mac(own_mac))
            .first()
            .cloned()
        else {
            println!(level:error, "construct_layer for IPv4 failed to find NIC for own MAC address {:?}", own_mac);
            return Err(Ipv4Error::OtherFatal); //rare enough to not need a specific error type
        };

        match nic_info.1.1 {
            NicType::Ethernet => {
                #[allow(irrefutable_let_patterns)] //might add more hw addresses later
                let HardwareAddr::Ethernet(remote_mac) = remote_hardware else {
                    println!(level:error, "construct_layer for IPv4 expected Ethernet hardware address for next hop but got different type");
                    return Err(Ipv4Error::OtherFatal);
                };

                packet
                    .layers_to_construct
                    .push(NetLayerFlowID::Ethernet(super::ethernet::EthernetFlowId {
                        mac_addr: AddressPair::new(own_mac, remote_mac),
                        ether_type: 0x0800,
                        out_interface: None,
                    }));

                out_flow_direction = OutgoingFlowDirection::LayerDown(LayerDownType::Normal(NetLayerType::Ethernet));
            }
            NicType::Ipv4 => {
                let Some(nic) = routing_tables::get_nic_from_id(&nic_info.0) else {
                    println!(level:error, "construct_layer for IPv4 failed to get NIC from ID {:?}", nic_info.0);
                    return Err(Ipv4Error::OtherFatal);
                };

                out_flow_direction = OutgoingFlowDirection::LayerDown(LayerDownType::Nic(nic));
            }
        }
    } else {
        match packet
            .layers_to_construct
            .last()
            .expect("layers was checked, has at least 1 layer")
        {
            NetLayerFlowID::Ethernet(_) => {
                out_flow_direction = OutgoingFlowDirection::LayerDown(LayerDownType::Normal(NetLayerType::Ethernet))
            }
            NetLayerFlowID::Ipv4(_) => {
                out_flow_direction = OutgoingFlowDirection::LayerDown(LayerDownType::Normal(NetLayerType::Ipv4))
            } //nested v4 yippie
            _ => {
                println!(level:error, "construct_layer for IPv4 found unsupported upper layer type in layers_to_construct");
                return Err(Ipv4Error::OtherFatal);
            }
        }
    }

    let packet_len = packet.len();
    let chunk_to_edit = packet.insert_chunk_front(20);
    write_data_to_packet(chunk_to_edit.data_mut(), data, 0, packet_len as usize);

    Ok(out_flow_direction)
}
