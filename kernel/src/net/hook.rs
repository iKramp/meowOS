use std::{collections::btree_map::BTreeMap, r_lock_w_info, sync::rw_lock::RWSpinlock, vec::Vec};

use crate::net::{NetLayerType, NetPacketListNode};

type NetHookFunction = fn(&mut NetPacketListNode) -> HookResult;

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

    pub fn call_hooks(&self, packet: &mut NetPacketListNode, stage: HookStage) -> HookResult {
        let Some(hooks) = self.hook_registrations.get(&stage) else {
            return HookResult::Nothing;
        };
        let mut hook_result = HookResult::Nothing;
        for hook in hooks {
            let Some(handler) = self.hooks.get(hook) else {
                continue;
            };
            match handler(packet) {
                HookResult::Nothing => {},
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

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(in crate::net) enum HookStage {
    BadPacket,
    Inbound(NetLayerType),
    Bridge(NetLayerType),
    Outbound(NetLayerType),
}


pub(in crate::net) fn call_hooks(packet: &mut NetPacketListNode, stage: HookStage) -> HookFilter {
    match r_lock_w_info!(NET_HOOK_STORAGE).call_hooks(packet, stage) {
        HookResult::Nothing => HookFilter::Continue,
        HookResult::LayerModified => {
            //modify packet bytes
            HookFilter::Continue
        },
        HookResult::Drop => HookFilter::Drop,
    }
}
