//! Read emitted ELF symbol boundaries and disassemble a bounded copy of loaded code.
use std::{
    collections::HashMap,
    ffi::{CStr, CString, c_char, c_int, c_void},
    fmt::Write,
    ptr,
    sync::Mutex,
};
type Handle = *mut c_void;
const MAX_CODE: usize = 8 * 1024 * 1024;
#[derive(Default)]
pub(super) struct Capture {
    pub sizes: HashMap<String, usize>,
    pub error: Option<String>,
}
#[derive(Clone, Debug)]
pub struct MachineCode {
    pub address: usize,
    pub bytes: Vec<u8>,
}
unsafe extern "C" {
    fn LLVMGetBufferStart(buffer: Handle) -> *const c_char;
    fn LLVMGetBufferSize(buffer: Handle) -> usize;
    fn LLVMCreateDisasm(
        triple: *const c_char,
        info: Handle,
        tag: c_int,
        operand_info: Handle,
        symbol_lookup: Handle,
    ) -> Handle;
    fn LLVMSetDisasmOptions(context: Handle, options: u64) -> c_int;
    fn LLVMDisasmInstruction(
        context: Handle,
        bytes: *mut u8,
        len: u64,
        pc: u64,
        output: *mut c_char,
        capacity: usize,
    ) -> usize;
    fn LLVMDisasmDispose(context: Handle);
}

pub(super) unsafe extern "C" fn capture_object(context: Handle, buffer: *mut Handle) -> Handle {
    let result = std::panic::catch_unwind(|| {
        if context.is_null() || buffer.is_null() {
            return;
        }
        let capture = unsafe { &*(context as *const Mutex<Capture>) };
        let object = unsafe { *buffer };
        if object.is_null() {
            return;
        }
        let size = unsafe { LLVMGetBufferSize(object) };
        let data = unsafe { LLVMGetBufferStart(object) };
        if data.is_null() || size > 64 * 1024 * 1024 {
            return;
        }
        let bytes = unsafe { std::slice::from_raw_parts(data.cast::<u8>(), size) };
        let result = elf_function_sizes(bytes);
        if let Ok(mut state) = capture.lock() {
            match result {
                Ok(symbols) => state.sizes.extend(symbols),
                Err(e) => state.error = Some(e),
            }
        }
    });
    if result.is_err() {
        if let Some(capture) = unsafe { (context as *const Mutex<Capture>).as_ref() } {
            if let Ok(mut state) = capture.lock() {
                state.error = Some("object metadata callback failed".into());
            }
        }
    }
    // ORC retains ownership of the unchanged memory buffer.
    ptr::null_mut()
}
pub(super) fn copy_loaded(address: usize, size: usize) -> Result<MachineCode, String> {
    if address == 0 || size == 0 || size > MAX_CODE || address.checked_add(size).is_none() {
        return Err("invalid native symbol bounds".into());
    }
    // Only called with the live LLJIT symbol address and the matching ELF function size.
    let bytes = unsafe { std::slice::from_raw_parts(address as *const u8, size) }.to_vec();
    Ok(MachineCode { address, bytes })
}
impl MachineCode {
    pub fn hex(&self) -> String {
        let mut text = String::with_capacity(self.bytes.len().saturating_mul(3));
        for (index, byte) in self.bytes.iter().enumerate() {
            if index != 0 {
                text.push(' ');
            }
            let _ = write!(text, "{byte:02x}");
        }
        text
    }
    pub fn disassemble(&self, triple: &str) -> Result<String, String> {
        let triple = CString::new(triple).map_err(|_| "invalid target triple")?;
        let context = unsafe {
            LLVMCreateDisasm(
                triple.as_ptr(),
                ptr::null_mut(),
                0,
                ptr::null_mut(),
                ptr::null_mut(),
            )
        };
        if context.is_null() {
            return Err("LLVM disassembler is unavailable for this target".into());
        }
        struct Guard(Handle);
        impl Drop for Guard {
            fn drop(&mut self) {
                unsafe { LLVMDisasmDispose(self.0) }
            }
        }
        let guard = Guard(context);
        unsafe {
            LLVMSetDisasmOptions(guard.0, 2 | 4);
        }
        let mut bytes = self.bytes.clone();
        let mut result = String::new();
        let mut offset = 0;
        while offset < bytes.len() {
            let mut text = [0 as c_char; 512];
            let length = unsafe {
                LLVMDisasmInstruction(
                    guard.0,
                    bytes.as_mut_ptr().add(offset),
                    (bytes.len() - offset) as u64,
                    (self.address + offset) as u64,
                    text.as_mut_ptr(),
                    text.len(),
                )
            };
            if length == 0 || length > bytes.len() - offset {
                return Err(format!(
                    "cannot decode instruction at native offset 0x{offset:x}"
                ));
            }
            let assembly = unsafe { CStr::from_ptr(text.as_ptr()) }.to_string_lossy();
            let hex = bytes[offset..offset + length]
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect::<Vec<_>>()
                .join(" ");
            result.push_str(&format!(
                "{:016x}  {:<44} {}\n",
                self.address + offset,
                hex,
                assembly.trim()
            ));
            offset += length;
            if result.len() > 16 * 1024 * 1024 {
                return Err("disassembly exceeds 16 MiB".into());
            }
        }
        Ok(result)
    }
}

fn range(bytes: &[u8], offset: usize, len: usize) -> Result<&[u8], String> {
    bytes
        .get(offset..offset.checked_add(len).ok_or("ELF offset overflow")?)
        .ok_or_else(|| "truncated ELF object".into())
}
fn u16_at(b: &[u8], o: usize) -> Result<u16, String> {
    Ok(u16::from_le_bytes(range(b, o, 2)?.try_into().unwrap()))
}
fn u32_at(b: &[u8], o: usize) -> Result<u32, String> {
    Ok(u32::from_le_bytes(range(b, o, 4)?.try_into().unwrap()))
}
fn u64_at(b: &[u8], o: usize) -> Result<u64, String> {
    Ok(u64::from_le_bytes(range(b, o, 8)?.try_into().unwrap()))
}
fn number(n: u64) -> Result<usize, String> {
    usize::try_from(n).map_err(|_| "ELF size exceeds address space".into())
}
fn elf_function_sizes(bytes: &[u8]) -> Result<HashMap<String, usize>, String> {
    if range(bytes, 0, 6)? != b"\x7fELF\x02\x01"
        || u16_at(bytes, 16)? != 1
        || u16_at(bytes, 18)? != 62
    {
        return Err(
            "native metadata requires a little-endian ELF64 x86_64 relocatable object".into(),
        );
    }
    let offset = number(u64_at(bytes, 40)?)?;
    let stride = u16_at(bytes, 58)? as usize;
    let count = u16_at(bytes, 60)? as usize;
    if stride < 64 || count == 0 {
        return Err("unsupported ELF section table".into());
    }
    range(
        bytes,
        offset,
        stride.checked_mul(count).ok_or("ELF section overflow")?,
    )?;
    let section = |i: usize| -> Result<usize, String> {
        if i >= count {
            Err("invalid ELF section index".into())
        } else {
            Ok(offset + i * stride)
        }
    };
    let mut symbols = HashMap::new();
    for i in 0..count {
        let h = section(i)?;
        if u32_at(bytes, h + 4)? != 2 {
            continue;
        }
        let start = number(u64_at(bytes, h + 24)?)?;
        let size = number(u64_at(bytes, h + 32)?)?;
        let entry = number(u64_at(bytes, h + 56)?)?;
        if entry < 24 || size % entry != 0 {
            return Err("invalid ELF symbol table".into());
        }
        range(bytes, start, size)?;
        let strings = section(u32_at(bytes, h + 40)? as usize)?;
        let table = range(
            bytes,
            number(u64_at(bytes, strings + 24)?)?,
            number(u64_at(bytes, strings + 32)?)?,
        )?;
        for sym in (start..start + size).step_by(entry) {
            if bytes[sym + 4] & 15 != 2 {
                continue;
            }
            let section_index = u16_at(bytes, sym + 6)? as usize;
            if section_index == 0 || section_index >= count {
                continue;
            }
            let length = number(u64_at(bytes, sym + 16)?)?;
            if length == 0 {
                continue;
            }
            if length > MAX_CODE {
                return Err("native function exceeds inspection limit".into());
            }
            let code = section(section_index)?;
            if u64_at(bytes, code + 8)? & 4 == 0 {
                continue;
            }
            let value = number(u64_at(bytes, sym + 8)?)?;
            let section_size = number(u64_at(bytes, code + 32)?)?;
            if value
                .checked_add(length)
                .is_none_or(|end| end > section_size)
            {
                return Err("ELF function exceeds its executable section".into());
            }
            let file_start = number(u64_at(bytes, code + 24)?)?;
            range(
                bytes,
                file_start.checked_add(value).ok_or("ELF offset overflow")?,
                length,
            )?;
            let name_start = u32_at(bytes, sym)? as usize;
            let tail = table.get(name_start..).ok_or("invalid ELF symbol name")?;
            let end = tail
                .iter()
                .position(|b| *b == 0)
                .ok_or("unterminated ELF symbol name")?;
            let name = std::str::from_utf8(&tail[..end]).map_err(|_| "non-UTF8 ELF symbol")?;
            symbols.insert(name.to_owned(), length);
        }
    }
    Ok(symbols)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_unbounded_or_truncated_metadata() {
        for bytes in [
            vec![],
            b"not ELF".to_vec(),
            b"\x7fELF\x02\x01".to_vec(),
            vec![0u8; 64],
        ] {
            assert!(elf_function_sizes(&bytes).is_err());
        }
    }
}
