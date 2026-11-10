//! Inspect native handle security descriptors; do not shell out to icacls.
use crate::error;
use scanner_types::error::Result;
use std::{
    ffi::c_void,
    fs::File,
    os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle},
};
use windows_sys::Win32::{
    Foundation::LocalFree,
    Security::{
        ACCESS_ALLOWED_ACE, ACE_HEADER, ACL,
        Authorization::{GetSecurityInfo, SE_FILE_OBJECT},
        DACL_SECURITY_INFORMATION, EqualSid, GetAce, GetTokenInformation, IsValidAcl, IsValidSid,
        IsWellKnownSid, OWNER_SECURITY_INFORMATION, TOKEN_QUERY, TOKEN_USER, TokenUser,
        WinBuiltinAdministratorsSid, WinLocalSystemSid,
    },
    Storage::FileSystem::{
        BY_HANDLE_FILE_INFORMATION, FILE_ATTRIBUTE_REPARSE_POINT, GetFileInformationByHandle,
    },
    System::Threading::{GetCurrentProcess, OpenProcessToken},
};
struct Descriptor(*mut c_void);
impl Drop for Descriptor {
    fn drop(&mut self) {
        // SAFETY: GetSecurityInfo allocates this descriptor with LocalAlloc.
        unsafe {
            LocalFree(self.0);
        }
    }
}
pub(crate) fn validate(file: &File, directory: bool) -> Result<()> {
    let handle = file.as_raw_handle();
    let mut info = BY_HANDLE_FILE_INFORMATION::default();
    // SAFETY: file owns a live handle; info is a correctly sized writable output.
    if unsafe { GetFileInformationByHandle(handle, &mut info) } == 0 {
        return Err(error(std::io::Error::last_os_error()));
    }
    if info.dwFileAttributes & FILE_ATTRIBUTE_REPARSE_POINT != 0
        || !directory && info.nNumberOfLinks != 1
    {
        return Err(error(
            "Private state cannot be reparse points or hard links.",
        ));
    }
    let user = current_user()?;
    // SAFETY: current_user returns aligned, initialized TOKEN_USER storage and
    // its referenced SID in the same allocation, retained until validation ends.
    let sid = unsafe { (*(user.as_ptr().cast::<TOKEN_USER>())).User.Sid };
    let (mut owner, mut acl, mut descriptor) = (
        std::ptr::null_mut(),
        std::ptr::null_mut(),
        std::ptr::null_mut(),
    );
    // SAFETY: outputs are valid pointers; descriptor owns owner/ACL storage.
    let status = unsafe {
        GetSecurityInfo(
            handle,
            SE_FILE_OBJECT,
            OWNER_SECURITY_INFORMATION | DACL_SECURITY_INFORMATION,
            &mut owner,
            std::ptr::null_mut(),
            &mut acl,
            std::ptr::null_mut(),
            &mut descriptor,
        )
    };
    if status != 0 {
        return Err(error(std::io::Error::from_raw_os_error(status as i32)));
    }
    let _descriptor = Descriptor(descriptor);
    // SAFETY: pointers are checked before use; all SID/ACL buffers came from the OS.
    if owner.is_null()
        || acl.is_null()
        || unsafe { IsValidSid(owner) } == 0
        || !trusted_identity(owner, sid)
        || unsafe { IsValidAcl(acl) } == 0
    {
        return Err(error(
            "Private state must have a trusted owner and a non-null DACL.",
        ));
    }
    validate_acl(acl, sid)
}
pub(super) fn current_user() -> Result<Vec<usize>> {
    let mut token = std::ptr::null_mut();
    // SAFETY: GetCurrentProcess returns a valid pseudo-handle; token is writable.
    if unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) } == 0 {
        return Err(error(std::io::Error::last_os_error()));
    }
    // SAFETY: successful OpenProcessToken transfers ownership of this handle.
    let token = unsafe { OwnedHandle::from_raw_handle(token) };
    let mut length = 0;
    // SAFETY: a zero-length query obtains the required buffer size.
    unsafe {
        GetTokenInformation(
            token.as_raw_handle(),
            TokenUser,
            std::ptr::null_mut(),
            0,
            &mut length,
        );
    }
    if length < std::mem::size_of::<TOKEN_USER>() as u32 || length > 65536 {
        return Err(error("Invalid process token size."));
    }
    let mut storage = vec![0usize; (length as usize).div_ceil(std::mem::size_of::<usize>())];
    // SAFETY: usize storage provides TOKEN_USER alignment and sufficient bytes.
    if unsafe {
        GetTokenInformation(
            token.as_raw_handle(),
            TokenUser,
            storage.as_mut_ptr().cast(),
            length,
            &mut length,
        )
    } == 0
    {
        return Err(error(std::io::Error::last_os_error()));
    }
    Ok(storage)
}
fn trusted_identity(sid: *mut c_void, user: *mut c_void) -> bool {
    // Elevated Windows processes may create SQLite sidecars owned by the local
    // Administrators group. Both privileged identities already have full access.
    // The DACL must still grant access only to this user, SYSTEM or Administrators.
    unsafe {
        EqualSid(sid, user) != 0
            || IsWellKnownSid(sid, WinLocalSystemSid) != 0
            || IsWellKnownSid(sid, WinBuiltinAdministratorsSid) != 0
    }
}
fn validate_acl(acl: *const ACL, user: *mut c_void) -> Result<()> {
    // SAFETY: validate checked the OS-allocated ACL and keeps its descriptor alive.
    for index in 0..unsafe { (*acl).AceCount } {
        let mut ace = std::ptr::null_mut();
        if unsafe { GetAce(acl, u32::from(index), &mut ace) } == 0 || ace.is_null() {
            return Err(error("Cannot inspect local state ACL."));
        }
        let header = unsafe { &*ace.cast::<ACE_HEADER>() };
        // ACCESS_DENIED_ACE cannot widen access. Other special ACE types are
        // rejected conservatively instead of guessing their effective rights.
        if header.AceType == 1 {
            continue;
        }
        if header.AceType != 0
            || usize::from(header.AceSize) < std::mem::size_of::<ACCESS_ALLOWED_ACE>()
        {
            return Err(error("Unsupported ACE in private local state."));
        }
        let allowed = ace.cast::<ACCESS_ALLOWED_ACE>();
        let sid = unsafe { std::ptr::addr_of!((*allowed).SidStart) }
            .cast_mut()
            .cast();
        // SAFETY: IsValidAcl/GetAce ensure an in-bounds ordinary ACE/SID. Only
        // this user and privileged OS/admin identities may have granted access.
        let private = unsafe { IsValidSid(sid) != 0 && trusted_identity(sid, user) };
        if !private {
            return Err(error(
                "Local state ACL grants access to another user or group.",
            ));
        }
    }
    Ok(())
}
