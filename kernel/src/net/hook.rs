use std::{
    collections::btree_map::BTreeMap,
    cow::Acow,
    println, r_lock_w_info,
    sync::{arc::Arc, rw_lock::RWSpinlock},
    vec::Vec,
    w_lock_w_info,
};

use crate::net::{NetLayerType, packet::NetPacket, socket::NetSocket};

type NetHookFunction = fn(&mut Acow<NetPacket>) -> HookResult;

static NET_SOCKETS: RWSpinlock<SocketStorage> = RWSpinlock::new(SocketStorage {
    sockets: BTreeMap::new(),
});

struct SocketStorage {
    sockets: BTreeMap<u64, Vec<Arc<NetSocket>>>,
}

pub fn add_socket(socket: Arc<NetSocket>) {
    let mut sockets = w_lock_w_info!(NET_SOCKETS);
    let sock_hash = socket.get_addr_hash();
    let _ = sockets.sockets.try_insert(sock_hash, Vec::new());
    let entry = sockets.sockets.get_mut(&sock_hash).expect("just tried adding smh");
    entry.push(socket);
    println!(
        "adding socket with hash {} to storage, currently has {} sockets",
        sock_hash,
        entry.len()
    );
}

pub fn remove_socket(socket: &NetSocket) {
    let mut sockets = w_lock_w_info!(NET_SOCKETS);
    let Some(sock_vec) = sockets.sockets.get_mut(&socket.get_addr_hash()) else {
        return;
    };

    sock_vec.retain(|vec_sock| vec_sock.id() != socket.id());
}

pub(in crate::net) static NET_HOOK_STORAGE: RWSpinlock<HookStorage> = RWSpinlock::new(HookStorage::new());

pub(in crate::net) struct HookStorage {
    hook_index_counter: u64,
    hook_registrations: BTreeMap<HookStage, Vec<u64>>,
    hooks: BTreeMap<u64, NetHookFunction>,
}

#[derive(Debug, PartialEq, Eq)]
pub(in crate::net) enum HookResult {
    Nothing,
    LayerModified,
    Drop,
}

pub(in crate::net) enum HookFilter {
    Continue,
    Drop,
}

impl HookStorage {
    pub const fn new() -> Self {
        Self {
            hook_index_counter: 0,
            hook_registrations: BTreeMap::new(),
            hooks: BTreeMap::new(),
        }
    }

    pub fn register_hook(&mut self, handler: NetHookFunction, stage: HookStage) -> u64 {
        let curr_hook_index = self.hook_index_counter;
        self.hook_index_counter += 1;
        self.hooks.insert(curr_hook_index, handler);
        if let Some(vec) = self.hook_registrations.get_mut(&stage) {
            vec.push(curr_hook_index);
        } else {
            let mut vec = Vec::new();
            vec.push(curr_hook_index);
            self.hook_registrations.insert(stage, vec);
        };
        curr_hook_index
    }

    pub fn call_hooks(&self, packet: &mut Acow<NetPacket>, stage: HookStage) -> HookResult {
        let Some(hooks) = self.hook_registrations.get(&stage) else {
            return HookResult::Nothing;
        };
        let mut hook_result = HookResult::Nothing;
        for hook in hooks {
            let Some(handler) = self.hooks.get(hook) else {
                continue;
            };
            match handler(packet) {
                HookResult::Nothing => {}
                HookResult::LayerModified => hook_result = HookResult::LayerModified,
                HookResult::Drop => return HookResult::Drop,
            }
        }
        hook_result
    }

    pub fn deregister_hook(&mut self, hook_id: u64) {
        self.hooks.remove(&hook_id);
    }

    pub fn clean_unused_hooks(&mut self) {
        for stage in self.hook_registrations.values_mut() {
            stage.retain(|hook_id| self.hooks.contains_key(hook_id));
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(in crate::net) enum HookStage {
    BadPacket,
    Inbound(NetLayerType),
    Bridge(NetLayerType),
    Outbound(NetLayerType),
}

pub(in crate::net) fn call_hooks(packet: &mut Acow<NetPacket>, stage: HookStage) -> HookFilter {
    let hook_result = match r_lock_w_info!(NET_HOOK_STORAGE).call_hooks(packet, stage.clone()) {
        HookResult::Nothing => HookFilter::Continue,
        HookResult::LayerModified => {
            todo!("fix layer modification");
            HookFilter::Continue
        }
        HookResult::Drop => return HookFilter::Drop,
    };

    'block: {
        if let HookStage::Inbound(_layer) = stage {
            println!("packet passed inbound hooks, checking sockets");
            let sockets = r_lock_w_info!(NET_SOCKETS);
            let addresses = packet.get_addresses();
            let addrs_cnt = addresses.len();
            if addrs_cnt == 0 {
                break 'block;
            }
            println!("packet has {} addresses, checking combinations", addrs_cnt);
            let top_layer_addr = &addresses[addrs_cnt - 1];
            for i in 0..(1 << (addrs_cnt - 1)) {
                let mut addrs_vec = Vec::new();
                for (j, addr) in addresses.iter().enumerate() {
                    if i & (1 << j) != 0 {
                        addrs_vec.push(addr.reverse());
                    }
                }
                addrs_vec.push(top_layer_addr.reverse());
                println!("checking socket for address combination: {:?}", addrs_vec);
                let hash = crate::net::hash_addr_slice(&addrs_vec);
                println!("hash for combination: {}", hash);
                let relevant_sockets = sockets.sockets.get(&hash);
                println!(
                    "found {} relevant sockets for this combination",
                    relevant_sockets.map(|vec| vec.len()).unwrap_or(0)
                );
                if let Some(sockets) = relevant_sockets {
                    for socket in sockets.iter() {
                        println!("socket has addresses {:?}", socket.addresses());
                        let are_same = socket.addresses() == addrs_vec;
                        println!("{}", are_same);
                        if are_same {
                            socket.push_packet(packet.clone().into_processed());
                        }
                    }
                }
            }
        }
    }
    hook_result
}
