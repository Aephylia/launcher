use anyhow::Result;
use windows::Win32::Foundation::CloseHandle;
use windows::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, Thread32First, Thread32Next, THREADENTRY32, TH32CS_SNAPTHREAD,
};
use windows::Win32::System::Threading::{
    OpenProcess, OpenThread, ResumeThread, SuspendThread, PROCESS_ALL_ACCESS, THREAD_SUSPEND_RESUME,
    TerminateProcess,
};

pub fn suspend_process(pid: u32) -> Result<()> {
    unsafe {
        let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPTHREAD, 0)?;
        let mut entry = THREADENTRY32 {
            dwSize: std::mem::size_of::<THREADENTRY32>() as u32,
            ..Default::default()
        };

        if Thread32First(snapshot, &mut entry).is_ok() {
            loop {
                if entry.th32OwnerProcessID == pid {
                    if let Ok(handle) = OpenThread(THREAD_SUSPEND_RESUME, false, entry.th32ThreadID) {
                        let _ = SuspendThread(handle);
                        let _ = CloseHandle(handle);
                    }
                }
                if Thread32Next(snapshot, &mut entry).is_err() {
                    break;
                }
            }
        }

        let _ = CloseHandle(snapshot);
    }
    Ok(())
}

pub fn resume_process(pid: u32) -> Result<()> {
    unsafe {
        let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPTHREAD, 0)?;
        let mut entry = THREADENTRY32 {
            dwSize: std::mem::size_of::<THREADENTRY32>() as u32,
            ..Default::default()
        };

        if Thread32First(snapshot, &mut entry).is_ok() {
            loop {
                if entry.th32OwnerProcessID == pid {
                    if let Ok(handle) = OpenThread(THREAD_SUSPEND_RESUME, false, entry.th32ThreadID) {
                        let _ = ResumeThread(handle);
                        let _ = CloseHandle(handle);
                    }
                }
                if Thread32Next(snapshot, &mut entry).is_err() {
                    break;
                }
            }
        }

        let _ = CloseHandle(snapshot);
    }
    Ok(())
}

pub fn kill_process_by_name(name: &str) -> Result<()> {
    let system = sysinfo::System::new_all();

    let name_without_ext = name.trim_end_matches(".exe").to_lowercase();
    let mut killed_count = 0;

    for (pid, process) in system.processes() {
        let process_name = process.name().to_string_lossy().to_lowercase();

        if process_name == name_without_ext || process_name == format!("{}.exe", name_without_ext) {
            unsafe {
                if let Ok(handle) = OpenProcess(PROCESS_ALL_ACCESS, false, pid.as_u32()) {
                    if TerminateProcess(handle, 1).is_ok() {
                        killed_count += 1;
                    }
                    let _ = CloseHandle(handle);
                }
            }
        }
    }

    if killed_count > 0 {
        println!("[+] Killed {} instance(s) of {}", killed_count, name);
    }

    Ok(())
}

pub fn is_process_running(name: &str) -> bool {
    let mut system = sysinfo::System::new_all();
    system.refresh_all();

    let name_without_ext = name.trim_end_matches(".exe").to_lowercase();

    system.processes().iter().any(|(_, process)| {
        let process_name = process.name().to_string_lossy().to_lowercase();
        process_name == name_without_ext || process_name == format!("{}.exe", name_without_ext)
    })
}
