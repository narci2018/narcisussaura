use std::os::windows::io::AsRawHandle;
use windows::Win32::Foundation::HANDLE;
use windows::Win32::System::JobObjects::{
    AssignProcessToJobObject, CreateJobObjectW, SetInformationJobObject,
    JobObjectExtendedLimitInformation, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
    JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
};

pub struct JobObjectGuard {
    handle: HANDLE,
}

unsafe impl Send for JobObjectGuard {}
unsafe impl Sync for JobObjectGuard {}

impl JobObjectGuard {
    pub fn new() -> Result<Self, String> {
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

            Ok(Self { handle })
        }
    }

    pub fn assign_process<T: AsRawHandle>(&self, process: &T) -> Result<(), String> {
        unsafe {
            let proc_handle = HANDLE(process.as_raw_handle());
            AssignProcessToJobObject(self.handle, proc_handle)
                .map_err(|e| format!("Failed to assign process to Job Object: {:?}", e))?;
            Ok(())
        }
    }
}
