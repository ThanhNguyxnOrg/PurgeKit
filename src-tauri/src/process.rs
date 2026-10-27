use std::os::windows::io::AsRawHandle;
use std::process::{Command, ExitStatus};
use std::time::{Duration, Instant};

use windows_sys::Win32::Foundation::{CloseHandle, HANDLE};
use windows_sys::Win32::System::JobObjects::{
    AssignProcessToJobObject, CreateJobObjectW, JobObjectBasicAccountingInformation,
    JobObjectExtendedLimitInformation, QueryInformationJobObject, SetInformationJobObject,
    TerminateJobObject, JOBOBJECT_BASIC_ACCOUNTING_INFORMATION,
    JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
};

const TREE_WAIT_TIMEOUT: Duration = Duration::from_secs(30 * 60);

struct JobHandle(HANDLE);

impl Drop for JobHandle {
    fn drop(&mut self) {
        unsafe {
            CloseHandle(self.0);
        }
    }
}

fn create_job() -> Option<JobHandle> {
    unsafe {
        let job = CreateJobObjectW(std::ptr::null(), std::ptr::null());
        if job.is_null() {
            return None;
        }

        let mut info: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = std::mem::zeroed();
        info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        let ok = SetInformationJobObject(
            job,
            JobObjectExtendedLimitInformation,
            &info as *const _ as *const core::ffi::c_void,
            std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
        );
        if ok == 0 {
            CloseHandle(job);
            return None;
        }

        Some(JobHandle(job))
    }
}

/// Spawn a process and wait until it and every child it creates have exited.
/// Job-object assignment is best-effort; if the host already constrains the
/// process tree, fall back to waiting for the direct child only.
pub fn run_command_in_job(command: &mut Command) -> std::io::Result<ExitStatus> {
    let mut child = command.spawn()?;

    let job = create_job();
    let assigned = if let Some(job) = job.as_ref() {
        unsafe { AssignProcessToJobObject(job.0, child.as_raw_handle() as HANDLE) != 0 }
    } else {
        false
    };

    let status = child.wait()?;

    if let (true, Some(job)) = (assigned, job.as_ref()) {
        let deadline = Instant::now() + TREE_WAIT_TIMEOUT;
        loop {
            let mut info: JOBOBJECT_BASIC_ACCOUNTING_INFORMATION = unsafe { std::mem::zeroed() };
            let ok = unsafe {
                QueryInformationJobObject(
                    job.0,
                    JobObjectBasicAccountingInformation,
                    &mut info as *mut _ as *mut core::ffi::c_void,
                    std::mem::size_of::<JOBOBJECT_BASIC_ACCOUNTING_INFORMATION>() as u32,
                    std::ptr::null_mut(),
                )
            };
            if ok == 0 || info.ActiveProcesses == 0 {
                break;
            }
            if Instant::now() >= deadline {
                unsafe {
                    TerminateJobObject(job.0, 1);
                }
                break;
            }
            std::thread::sleep(Duration::from_millis(100));
        }
    }

    Ok(status)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn waits_for_a_direct_child() {
        let mut command = Command::new("cmd.exe");
        command.args(["/C", "exit", "0"]);
        let status = run_command_in_job(&mut command).expect("spawn cmd.exe");
        assert!(status.success());
    }
}
