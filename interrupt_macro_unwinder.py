import gdb
from gdb.unwinder import Unwinder, FrameId

TARGET_FUNC = "set_entries::wrapper"

class InterruptHandlerUnwinder(Unwinder):
    """
    Unwinder for kernel interrupt handlers.
    
    This unwinder handles the custom stack layout created by the interrupt
    handler macro, which saves all registers and interrupt frame information. 
    """
    
    def __init__(self):
        super().__init__("InterruptHandlerUnwinder")
    
    def __call__(self, pending_frame):
        try:
            name = pending_frame.name()
            if not name or not name.endswith(TARGET_FUNC):
                return None
        except:
            return None
        
        # Get current RSP
        rsp = pending_frame. read_register("rsp")
        rip = pending_frame. read_register("rip")
        
        # The stack layout from your macro:
        # [rsp + 0]:       swapgs flag (0 or 1)
        # [rsp + 8]:      r15
        # [rsp + 16]:     r14
        # [rsp + 24]:     r13
        # [rsp + 32]:     r12
        # [rsp + 40]:      r11
        # [rsp + 48]:     r10
        # [rsp + 56]:     r9
        # [rsp + 64]:     r8
        # [rsp + 72]:     rbp
        # [rsp + 80]:     rdi
        # [rsp + 88]:      rsi
        # [rsp + 96]:     rdx
        # [rsp + 104]:    rcx
        # [rsp + 112]:    rbx
        # [rsp + 120]:    rax
        # [rsp + 128]:    error_code
        # [rsp + 136]:    rip (interrupt frame)
        # [rsp + 144]:    cs
        # [rsp + 152]:    rflags
        # [rsp + 160]:    rsp (original)
        # [rsp + 168]:    ss
        
        # Create frame ID using the stack pointer
        frame_id = FrameId(int(rsp), int(rip))
        
        # Create the unwind info
        unwind_info = pending_frame.create_unwind_info(frame_id)
        
        # Read saved registers from the stack
        # Helper function to read a value from stack
        def read_stack(offset):
            try:
                addr = int(rsp) + offset
                inferior = gdb.selected_inferior()
                data = inferior.read_memory(addr, 8)
                return int.from_bytes(data, byteorder='little')
            except:
                return None
        #de469
        # Restore all general purpose registers
        reg_offsets = {
            "r15": 8,
            "r14": 16,
            "r13":  24,
            "r12": 32,
            "r11": 40,
            "r10": 48,
            "r9": 56,
            "r8": 64,
            "rbp": 72,
            "rdi": 80,
            "rsi":  88,
            "rdx": 96,
            "rcx": 104,
            "rbx": 112,
            "rax": 120,
        }
        
        for reg_name, offset in reg_offsets.items():
            value = read_stack(offset)
            if value is not None: 
                unwind_info.add_saved_register(reg_name, gdb.Value(value))
        
        # Restore interrupt frame registers
        rip_value = read_stack(136)
        rsp_value = read_stack(160)
        
        if rip_value is not None: 
            unwind_info.add_saved_register("rip", gdb.Value(rip_value))
        
        if rsp_value is not None:
            # The original RSP is what we use for the caller's stack pointer
            unwind_info.add_saved_register("rsp", gdb.Value(rsp_value))
        
        return unwind_info


# Register the unwinder
gdb.unwinder.register_unwinder(None, InterruptHandlerUnwinder(), replace=True)

print("Interrupt handler unwinder registered for function 'test_func'")
