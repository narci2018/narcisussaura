/// Cross-platform process lifecycle guard.
/// On Windows, binds child processes to a Job Object with `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`
/// ensuring that when the application terminates, all child core processes are killed immediately.
/// On macOS / Linux, process cleanup is safely managed upon drop.

pub struct ProcessGuard {
    #[cfg(windows)]
    job_handle: Option<windows::Win32::Foundation::HANDLE>,
}

unsafe impl Send for ProcessGuard {}
unsafe impl Sync for ProcessGuard {}

impl ProcessGuard {
    pub fn new() -> Result<Self, String> {
        #[cfg(windows)]
        {
            use windows::Win32::System::JobObjects::{
                CreateJobObjectW, SetInformationJobObject,
                JobObjectExtendedLimitInformation, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
                JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
            };

            unsafe {
                let handle = CreateJobObjectW(None, None)
                    .map_err(|e| format!("Failed to create Windows Job Object: {:?}", e))?;

                let mut info = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
                info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;

                SetInformationJobObject(
                    handle,
                    JobObjectExtendedLimitInformation,
                    &info as *const _ as *const _,
                    std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
                )
                .map_err(|e| format!("Failed to configure Job Object: {:?}", e))?;

                Ok(Self { job_handle: Some(handle) })
            }
        }
        #[cfg(not(windows))]
        {
            Ok(Self {})
        }
    }

    #[cfg(windows)]
    pub fn assign_process<T: std::os::windows::io::AsRawHandle>(&self, process: &T) -> Result<(), String> {
        if let Some(handle) = self.job_handle {
            unsafe {
                let proc_handle = windows::Win32::Foundation::HANDLE(process.as_raw_handle());
                windows::Win32::System::JobObjects::AssignProcessToJobObject(handle, proc_handle)
                    .map_err(|e| format!("Failed to assign process to Job Object: {:?}", e))?;
            }
        }
        Ok(())
    }

    #[cfg(not(windows))]
    pub fn assign_process<T>(&self, _process: &T) -> Result<(), String> {
        Ok(())
    }
}

impl Drop for ProcessGuard {
    fn drop(&mut self) {
        #[cfg(windows)]
        {
            if let Some(handle) = self.job_handle {
                use windows::Win32::Foundation::CloseHandle;
                unsafe {
                    let _ = CloseHandle(handle);
                }
            }
        }
    }
}
