use anyhow::{Context, Result};
use std::ffi::CString;
use std::path::Path;
use windows::core::PCSTR;
use windows::Win32::Foundation::CloseHandle;
use windows::Win32::System::Diagnostics::Debug::WriteProcessMemory;
use windows::Win32::System::LibraryLoader::{GetModuleHandleA, GetProcAddress};
use windows::Win32::System::Memory::{
    VirtualAllocEx, VirtualFreeEx, MEM_COMMIT, MEM_RELEASE, MEM_RESERVE, PAGE_READWRITE,
};
use windows::Win32::System::Threading::{
    CreateRemoteThread, GetExitCodeThread, OpenProcess, WaitForSingleObject, PROCESS_ALL_ACCESS,
};

const INFINITE: u32 = 0xFFFFFFFF;

pub fn inject_dll(pid: u32, dll_path: &str) -> Result<()> {
    if !Path::new(dll_path).exists() {
        anyhow::bail!("DLL not found: {}", dll_path);
    }

    let dll_path_abs = std::fs::canonicalize(dll_path)
        .context("Failed to get absolute path")?
        .to_string_lossy()
        .to_string();

    unsafe {
        let process_handle = OpenProcess(PROCESS_ALL_ACCESS, false, pid)
            .context("Failed to open target process")?;

        let cpath = CString::new(dll_path_abs.clone()).context("CString::new failed")?;
        let bytes_with_nul = cpath.as_bytes_with_nul();
        let alloc_size = bytes_with_nul.len();

        let remote_memory = VirtualAllocEx(
            process_handle,
            None,
            alloc_size,
            MEM_COMMIT | MEM_RESERVE,
            PAGE_READWRITE,
        );

        if remote_memory.is_null() {
            CloseHandle(process_handle)?;
            anyhow::bail!("Failed to allocate memory in target process");
        }

        let write_result = WriteProcessMemory(
            process_handle,
            remote_memory,
            bytes_with_nul.as_ptr() as *const _,
            alloc_size,
            None, 
        );

        if write_result.is_err() {
            VirtualFreeEx(process_handle, remote_memory, 0, MEM_RELEASE)?;
            CloseHandle(process_handle)?;
            anyhow::bail!("WriteProcessMemory failed: {:?}", write_result.err());
        }

        let kernel32_name = CString::new("kernel32.dll").unwrap();
        let loadlibrary_name = CString::new("LoadLibraryA").unwrap();

        let kernel32_handle = GetModuleHandleA(PCSTR(kernel32_name.as_ptr() as *const u8))
            .context("Failed to get kernel32.dll handle")?;

        let loadlibrary_addr = GetProcAddress(
            kernel32_handle,
            PCSTR(loadlibrary_name.as_ptr() as *const u8),
        )
        .context("Failed to get LoadLibraryA address")?;

        let thread_handle = CreateRemoteThread(
            process_handle,
            None,
            0,
            Some(std::mem::transmute(loadlibrary_addr)),
            Some(remote_memory),
            0,
            None,
        )
        .context("Failed to create remote thread")?;

        WaitForSingleObject(thread_handle, INFINITE);

        let mut exit_code: u32 = 0;
        let get_exit = GetExitCodeThread(thread_handle, &mut exit_code);
        if get_exit.is_err() {
            CloseHandle(thread_handle)?;
            VirtualFreeEx(process_handle, remote_memory, 0, MEM_RELEASE)?;
            CloseHandle(process_handle)?;
            anyhow::bail!("GetExitCodeThread failed: {:?}", get_exit.err());
        }

        if exit_code == 0 {
            CloseHandle(thread_handle)?;
            VirtualFreeEx(process_handle, remote_memory, 0, MEM_RELEASE)?;
            CloseHandle(process_handle)?;
            anyhow::bail!("Remote LoadLibraryA returned NULL (failed to load DLL)");
        } else {
            println!("DLL loaded successfully, remote HMODULE = 0x{:x}", exit_code);
        }

        CloseHandle(thread_handle)?;
        VirtualFreeEx(process_handle, remote_memory, 0, MEM_RELEASE)?;
        CloseHandle(process_handle)?;
    }

    Ok(())
}
