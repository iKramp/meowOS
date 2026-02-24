use core::any::Any;
use std::{cow::Acow, println, w_lock_w_info};

use crate::net::{
    self, NetLayerType, NetPacketListNode, address_pair::AddressPair, flow::{IncomingFlowDirection, LayerDownType, OutgoingFlowDirection}, hook::HookResult, packet::NetPacket, protocols::{MacAddress, NetLayer, NetLayerFlowID, ethernet::EthernetFlowId, ipv4}, routing_tables::{self, get_self_arp_entry}
};

#[derive(Debug, Clone)]
pub(in crate::net) struct ArpHeader {
    offset: u32,
    flow_id: ArpFlowId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::net) struct ArpFlowId {
    operation: u16,
    protocol: AddressPair<ProtocolAddr>,
    hardware: AddressPair<HardwareAddr>,
}

pub(super) fn init() {
    w_lock_w_info!(net::hook::NET_HOOK_STORAGE).register_hook(process_arp, net::hook::HookStage::Bridge(NetLayerType::Arp));
}

impl NetLayer for ArpHeader {
    fn incoming_flow_direction(&self) -> IncomingFlowDirection {
        IncomingFlowDirection::Bridge
    }

    fn current_layer_type(&self) -> NetLayerType {
        NetLayerType::Arp
    }

    fn current_layer_offset(&self) -> u32 {
        self.offset
    }

    fn upper_layer_type(&self) -> NetLayerType {
        NetLayerType::None
    }

    fn upper_layer_offset(&self) -> u32 {
        self.offset
    }

    fn bridge_to_out_set_layers(&self, out_layers: &mut std::vec::Vec<super::NetLayerFlowID>) {
        let arp_data = super::NetLayerFlowID::Arp(ArpFlowId {
            operation: 2, // packet through arp bridge means response
            protocol: self.flow_id.protocol.reverse(),
            hardware: self.flow_id.hardware.reverse(),
        });

        println!("Bridging ARP packet, setting out layers to: {:?}", arp_data);

        out_layers.push(arp_data);
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::net) enum HardwareAddr {
    Ethernet(MacAddress),
}

impl HardwareAddr {
    fn from_bytes(hardware_type: u16, data: &[u8]) -> Option<Self> {
        match hardware_type {
            1 => {
                let mut addr = MacAddress([0u8; 6]);
                addr.0.copy_from_slice(&data[0..6]);
                Some(HardwareAddr::Ethernet(addr))
            }
            _ => None,
        }
    }

    fn length(&self) -> usize {
        match self {
            HardwareAddr::Ethernet(_) => 6,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(in crate::net) enum ProtocolAddr {
    Ipv4(ipv4::Ipv4Address),
    Ipv6([u8; 16]), //not really used
}

impl ProtocolAddr {
    fn from_bytes(protocol_type: u16, data: &[u8]) -> Option<Self> {
        match protocol_type {
            0x0800 => {
                let mut addr = [0u8; 4];
                addr.copy_from_slice(&data[0..4]);
                Some(ProtocolAddr::Ipv4(ipv4::Ipv4Address(addr)))
            }
            0x86DD => {
                let mut addr = [0u8; 16];
                addr.copy_from_slice(&data[0..16]);
                Some(ProtocolAddr::Ipv6(addr))
            }
            _ => None,
        }
    }

    fn length(&self) -> usize {
        match self {
            ProtocolAddr::Ipv4(_) => 4,
            ProtocolAddr::Ipv6(_) => 16,
        }
    }
}

fn process_arp(packet: &mut NetPacketListNode) -> HookResult {
    let Some(arp_layer) = packet
        .get_highest_layer_mut()
        .and_then(|layer| (layer as &mut dyn Any).downcast_mut::<ArpHeader>())
    else {
        println!(level:error, "ARP hook called but highest layer is not ArpHeader");
        return HookResult::Drop;
    };
    routing_tables::update_arp_entry(
        arp_layer.flow_id.hardware.source().clone(),
        arp_layer.flow_id.protocol.source().clone(),
    );
    if arp_layer.flow_id.operation == 2 {
        //response to some other request, ignore
        println!("Received ARP response, ignoring");
        return HookResult::Drop;
    }
    let Some(self_entry) = get_self_arp_entry(arp_layer.flow_id.protocol.target()) else {
        println!("Received ARP request for {:?}, but it does not match any of our addresses, ignoring", arp_layer.flow_id.protocol.target());
        return HookResult::Drop; //drop packet, it is not meant for us
    };
    println!("Received ARP request for {:?}, responding with {:?}", arp_layer.flow_id.protocol.target(), self_entry);
    arp_layer.flow_id.hardware.target = self_entry;

    HookResult::Nothing
}

pub(in crate::net::protocols) fn parse_arp(packet: &mut Acow<NetPacket>, mut offset: usize) -> Option<ArpHeader> {
    packet.ensure_length(offset as u32 + 28);

    let arp_offset = offset as u32;
    let chunks = packet.get_chunks();
    let data = chunks[0].data();
    let hardware_type = u16::from_be_bytes([data[offset], data[offset + 1]]);
    offset += 2;

    let protocol_type = u16::from_be_bytes([data[offset], data[offset + 1]]);
    offset += 2;

    let hardware_size = data[offset] as usize;
    offset += 1;
    let protocol_size = data[offset] as usize;
    offset += 1;

    let operation = u16::from_be_bytes([data[offset], data[offset + 1]]);
    offset += 2;

    let sender_hardware = HardwareAddr::from_bytes(hardware_type, &data[offset..(offset + hardware_size)])?;
    offset += hardware_size;

    let sender_protocol = ProtocolAddr::from_bytes(protocol_type, &data[offset..(offset + protocol_size)])?;
    offset += protocol_size;

    let target_hardware = HardwareAddr::from_bytes(hardware_type, &data[offset..(offset + hardware_size)])?;
    offset += hardware_size;

    let target_protocol = ProtocolAddr::from_bytes(protocol_type, &data[offset..(offset + hardware_size)])?;

    let arp_header = ArpHeader {
        offset: arp_offset,
        flow_id: ArpFlowId {
            operation,
            hardware: AddressPair::new(sender_hardware, target_hardware),
            protocol: AddressPair::new(sender_protocol, target_protocol),
        },
    };
    Some(arp_header)
}

pub(in crate::net::protocols) fn write_modified_header_to_packet(packet: &mut NetPacket, offset: usize, header: ArpHeader) {
    let data = packet.get_chunks_mut()[0].data_mut();
    write_data_to_packet(data, header.flow_id, offset);
}

fn write_data_to_packet(packet: &mut [u8], data: ArpFlowId, offset: usize) {
    packet[offset + 6..offset + 8].copy_from_slice(&data.operation.to_be_bytes());

    let mut addr_offset = 8 + offset;

    match data.hardware.source() {
        HardwareAddr::Ethernet(mac) => {
            packet[offset..(offset + 2)].copy_from_slice(&1u16.to_be_bytes()); //hw type Ethernet
            packet[offset + 4] = 6_u8; //len
            packet[addr_offset..(addr_offset + 6)].copy_from_slice(&mac.0);
            addr_offset += 6;
        }
    };

    match data.protocol.source() {
        ProtocolAddr::Ipv4(v4_addr) => {
            packet[offset + 2..offset + 4].copy_from_slice(&0x0800u16.to_be_bytes());
            packet[offset + 5] = 4_u8; //len
            packet[addr_offset..(addr_offset + 4)].copy_from_slice(&v4_addr.0);
            addr_offset += 4;
        }
        ProtocolAddr::Ipv6(v6_addr) => {
            packet[offset + 2..offset + 4].copy_from_slice(&0x86DDu16.to_be_bytes());
            packet[offset + 5] = 16_u8; //len
            packet[addr_offset..(addr_offset + 16)].copy_from_slice(v6_addr);
            addr_offset += 16;
        }
    };

    match data.hardware.target() {
        HardwareAddr::Ethernet(mac) => {
            packet[addr_offset..(addr_offset + 6)].copy_from_slice(&mac.0);
            addr_offset += 6;
        }
    };

    match data.protocol.target() {
        ProtocolAddr::Ipv4(v4_addr) => {
            packet[addr_offset..(addr_offset + 4)].copy_from_slice(&v4_addr.0);
            addr_offset += 4;
        }
        ProtocolAddr::Ipv6(v6_addr) => {
            packet[addr_offset..(addr_offset + 16)].copy_from_slice(v6_addr);
            addr_offset += 16;
        }
    };
    let _ = addr_offset;
}

pub(in crate::net::protocols) fn construct_layer(packet: &mut Acow<NetPacket>, bridged: bool) -> OutgoingFlowDirection {
    let Some(NetLayerFlowID::Arp(data)) = packet.layers_to_construct.pop() else {
        println!(level:error, "construct_layer called for ARP but highest layer is not ArpFlowId");
        return OutgoingFlowDirection::Drop;
    };

    match data.hardware {
        AddressPair { source: HardwareAddr::Ethernet(source_mac), target: HardwareAddr::Ethernet(dest_mac) } => {
            packet.layers_to_construct.push(NetLayerFlowID::Ethernet(EthernetFlowId::new(source_mac, dest_mac, 0x0806)));
        }
    }

    let total_length = 8
        + data.hardware.source().length() * 2
        + data.protocol.source().length() * 2;


    let chunk_to_edit = if bridged {
        packet.truncate(total_length as u32);
        packet
            .get_chunks_mut()
            .first_mut()
            .expect("ARP layer should always have at least one chunk")
    } else {
        packet.insert_chunk_front(total_length as u32)
    };

    let chunk_data = chunk_to_edit.data_mut();
    write_data_to_packet(chunk_data, data, 0);

    OutgoingFlowDirection::LayerDown(LayerDownType::Normal(NetLayerType::Ethernet))
}
