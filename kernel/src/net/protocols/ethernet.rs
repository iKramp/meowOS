use core::any::Any;
use core::ptr::addr_of_mut;
use std::{cow::Acow, println, w_lock_w_info};

use crate::net::{
    self, NetPacketListNode, NetPacketSource,
    address_pair::AddressPair,
    flow::{LayerDownType, OutgoingFlowDirection},
    hook::HookResult,
    packet::NetPacket,
    protocols::{MacAddress, NetLayer, NetLayerFlowID, NetLayerType},
    routing_tables::{self, is_own_mac},
};

#[derive(Debug, Clone)]
pub(in crate::net) struct EthernetHeader {
    offset: u32,
    crc_offset: u32,
    source: MacAddress,
    destination: MacAddress,
    upper_type: u16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::net) struct EthernetFlowId {
    mac_addr: AddressPair<MacAddress>,
    ether_type: u16,
}

impl EthernetFlowId {
    pub fn new(source: MacAddress, destination: MacAddress, ether_type: u16) -> Self {
        Self {
            mac_addr: AddressPair::new(source, destination),
            ether_type,
        }
    }
}

pub(super) fn init() {
    w_lock_w_info!(net::hook::NET_HOOK_STORAGE).register_hook(bridge_hook, net::hook::HookStage::Bridge(NetLayerType::Ethernet));
}

fn bridge_hook(packet: &mut NetPacketListNode) -> HookResult {
    let ether_layer = match packet
        .get_highest_layer()
        .and_then(|layer| (layer as &dyn Any).downcast_ref::<EthernetHeader>())
    {
        Some(layer) => layer,
        None => return HookResult::Drop,
    };

    let //IDFK what i was doing here
}

impl NetLayer for EthernetHeader {
    fn incoming_flow_direction(&self) -> crate::net::flow::IncomingFlowDirection {
        let destination_is_broadcast = self.destination.is_broadcast();
        let destination_is_me = is_own_mac(&self.destination);
        println!(
            "Ethernet packet with destination {:?}, is_broadcast: {}, is_me: {}",
            self.destination, destination_is_broadcast, destination_is_me
        );

        match (destination_is_broadcast, destination_is_me) {
            (true, _) => {
                crate::net::flow::IncomingFlowDirection::Both(self.upper_layer_type(), self.upper_layer_offset() as usize)
            }
            (false, true) => {
                crate::net::flow::IncomingFlowDirection::LayerUp(self.upper_layer_type(), self.upper_layer_offset() as usize)
            }
            (false, false) => crate::net::flow::IncomingFlowDirection::Bridge,
        }
    }

    fn upper_layer_type(&self) -> NetLayerType {
        match self.upper_type {
            0x0800 => super::NetLayerType::Ipv4,
            0x86DD => super::NetLayerType::Unknown, // IPv6 is not supported yet
            0x0806 => super::NetLayerType::Arp,
            _ => super::NetLayerType::Unknown,
        }
    }

    fn upper_layer_offset(&self) -> u32 {
        self.offset + 14
    }

    fn current_layer_type(&self) -> NetLayerType {
        NetLayerType::Ethernet
    }

    fn current_layer_offset(&self) -> u32 {
        self.offset
    }

    fn bridge_to_out_set_layers(&self, out_layers: &mut std::vec::Vec<super::NetLayerFlowID>) {
        //bridging doesn't change Ethernet headers
        out_layers.push(NetLayerFlowID::Ethernet(EthernetFlowId::new(
            self.source,
            self.destination,
            self.upper_type,
        )));
    }
}

pub(super) fn parse_ethernet_frame(packet: &mut Acow<NetPacket>) -> Option<EthernetHeader> {
    let packet_len = packet.len();
    if packet_len < 14 {
        // Ethernet header + minimum payload + CRC
        println!("Ethernet frame too short: {}", packet_len);
        return None;
    }

    packet.ensure_length(14);
    let chunks = packet.get_chunks();
    let data = chunks[0].data();
    let mut destination = MacAddress([0; 6]);
    let mut source = MacAddress([0; 6]);
    unsafe { core::ptr::copy_nonoverlapping(data.as_ptr().byte_add(0), addr_of_mut!(destination) as *mut u8, 6) };
    unsafe { core::ptr::copy_nonoverlapping(data.as_ptr().byte_add(6), addr_of_mut!(source) as *mut u8, 6) };
    let lower_type = u16::from_be_bytes([data[12], data[13]]);

    if let NetPacketSource::Nic(nic) = packet.source {
        routing_tables::register_foreign_mac_address(source, nic);
    }

    Some(EthernetHeader {
        offset: 0,
        crc_offset: packet_len - 4,
        source,
        destination,
        upper_type: lower_type,
    })
}

fn write_to_packet(packet: &mut [u8], data: EthernetFlowId) {
    let EthernetFlowId {
        mac_addr: AddressPair { source, target },
        ether_type,
    } = data;
    packet[0..6].copy_from_slice(&target.0);
    packet[6..12].copy_from_slice(&source.0);
    packet[12..14].copy_from_slice(&ether_type.to_be_bytes());
}

pub(in crate::net::protocols) fn construct_layer(packet: &mut Acow<NetPacket>, bridged: bool) -> OutgoingFlowDirection {
    let Some(NetLayerFlowID::Ethernet(data)) = packet.layers_to_construct.pop() else {
        println!(level:error, "construct_layer called for Ethernet but highest layer is not Ethernet");
        return OutgoingFlowDirection::Drop;
    };

    let target_mac = *data.mac_addr.target();
    let in_nic = match packet.source {
        NetPacketSource::Nic(nic) => Some(nic),
        _ => None,
    };

    let chunk_to_edit = if bridged {
        packet
            .get_chunks_mut()
            .first_mut()
            .expect("Ethernet layer should always have at least one chunk")
    } else {
        packet.insert_chunk_front(14)
    };

    let chunk_data = chunk_to_edit.data_mut();
    write_to_packet(chunk_data, data);

    if target_mac.is_broadcast() {
        let nices = routing_tables::get_broadcast_nices(in_nic);
        OutgoingFlowDirection::LayerDown(LayerDownType::NicGroup(nices))
    } else {
        let Some(nic) = routing_tables::get_mac_nic(&target_mac) else {
            println!(level:warn, "construct_layer for Ethernet failed to find NIC for destination MAC {:?}, dropping packet", target_mac);
            return OutgoingFlowDirection::Drop;
        };
        OutgoingFlowDirection::LayerDown(LayerDownType::Nic(nic))
    }
}
