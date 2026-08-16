//! C ABI for embedding simple-backups in mobile / other runtimes.
//!
//! Strings returned from this crate are heap-allocated UTF-8 C strings;
//! callers must free them with [`sb_string_free`].

use backups_core::{format_pair_payload, parse_pair_payload};
use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_int};
use std::path::Path;

#[cfg(feature = "jni")]
mod jni;

#[cfg(feature = "pqc")]
mod pqc_api;
#[cfg(feature = "pqc")]
mod runtime;

const OK: c_int = 0;
const ERR: c_int = 1;

fn cstr_to_str<'a>(p: *const c_char) -> Result<&'a str, String> {
    if p.is_null() {
        return Err("null string".into());
    }
    unsafe { CStr::from_ptr(p) }
        .to_str()
        .map_err(|e| e.to_string())
}

fn to_cstring(s: impl Into<String>) -> *mut c_char {
    CString::new(s.into())
        .unwrap_or_else(|_| CString::new("invalid utf-8").unwrap())
        .into_raw()
}

unsafe fn set_err(err_out: *mut *mut c_char, msg: impl Into<String>) {
    if !err_out.is_null() {
        *err_out = to_cstring(msg);
    }
}

/// Library version string. Caller frees with [`sb_string_free`].
#[no_mangle]
pub extern "C" fn sb_version() -> *mut c_char {
    to_cstring(env!("CARGO_PKG_VERSION"))
}

/// Free a string returned by this library. No-op on null.
///
/// # Safety
/// `s` must be null or a pointer previously returned by this crate.
#[no_mangle]
pub unsafe extern "C" fn sb_string_free(s: *mut c_char) {
    if s.is_null() {
        return;
    }
    drop(CString::from_raw(s));
}

/// Parse a pairing payload.
///
/// # Safety
/// Output pointers must be valid writable `*mut *mut c_char` or null.
#[no_mangle]
pub unsafe extern "C" fn sb_parse_pair_payload(
    payload: *const c_char,
    addr_out: *mut *mut c_char,
    code_out: *mut *mut c_char,
    err_out: *mut *mut c_char,
) -> c_int {
    let Ok(raw) = cstr_to_str(payload) else {
        set_err(err_out, "invalid payload pointer");
        return ERR;
    };
    match parse_pair_payload(raw) {
        Ok(p) => {
            if !addr_out.is_null() {
                *addr_out = to_cstring(p.addr);
            }
            if !code_out.is_null() {
                *code_out = to_cstring(p.code);
            }
            OK
        }
        Err(e) => {
            set_err(err_out, e.to_string());
            ERR
        }
    }
}

/// Format `simple-backups:v1:pair:<addr>:<code>`. Caller frees the result.
///
/// # Safety
/// `err_out` must be valid or null.
#[no_mangle]
pub unsafe extern "C" fn sb_format_pair_payload(
    addr: *const c_char,
    code: *const c_char,
    err_out: *mut *mut c_char,
) -> *mut c_char {
    let (Ok(a), Ok(c)) = (cstr_to_str(addr), cstr_to_str(code)) else {
        set_err(err_out, "invalid addr/code pointer");
        return std::ptr::null_mut();
    };
    to_cstring(format_pair_payload(a, c))
}

/// `1` if built with the `pqc` feature.
#[no_mangle]
pub extern "C" fn sb_pqc_enabled() -> c_int {
    i32::from(cfg!(feature = "pqc"))
}

/// Generate a one-time pairing code (hex). Requires `--features pqc`.
///
/// # Safety
/// `err_out` must be valid or null.
#[no_mangle]
pub unsafe extern "C" fn sb_generate_pairing_code(err_out: *mut *mut c_char) -> *mut c_char {
    #[cfg(feature = "pqc")]
    {
        match pqc_api::gen_code() {
            Ok(code) => to_cstring(code),
            Err(e) => {
                set_err(err_out, e.to_string());
                std::ptr::null_mut()
            }
        }
    }
    #[cfg(not(feature = "pqc"))]
    {
        set_err(err_out, "built without pqc feature");
        std::ptr::null_mut()
    }
}

/// Ensure a vault exists and contains an ML-DSA identity. Returns verifying-key hex.
///
/// # Safety
/// Pointers must be valid C strings / out-params.
#[no_mangle]
pub unsafe extern "C" fn sb_identity_ensure(
    vault_path: *const c_char,
    password: *const c_char,
    err_out: *mut *mut c_char,
) -> *mut c_char {
    #[cfg(feature = "pqc")]
    {
        let (Ok(v), Ok(p)) = (cstr_to_str(vault_path), cstr_to_str(password)) else {
            set_err(err_out, "invalid vault/password pointer");
            return std::ptr::null_mut();
        };
        match pqc_api::identity_ensure(Path::new(v), p) {
            Ok(vk) => to_cstring(vk),
            Err(e) => {
                set_err(err_out, e.to_string());
                std::ptr::null_mut()
            }
        }
    }
    #[cfg(not(feature = "pqc"))]
    {
        let _ = (vault_path, password, Path::new("."));
        set_err(err_out, "built without pqc feature");
        std::ptr::null_mut()
    }
}

/// Pair with a peer. `listen != 0` listens on `addr`; otherwise connects.
/// Returns pinned peer verifying-key hex.
///
/// # Safety
/// Pointers must be valid C strings / out-params.
#[no_mangle]
pub unsafe extern "C" fn sb_pair(
    vault_path: *const c_char,
    password: *const c_char,
    peer: *const c_char,
    addr: *const c_char,
    code: *const c_char,
    listen: c_int,
    err_out: *mut *mut c_char,
) -> *mut c_char {
    #[cfg(feature = "pqc")]
    {
        let (Ok(v), Ok(pw), Ok(peer), Ok(addr), Ok(code)) = (
            cstr_to_str(vault_path),
            cstr_to_str(password),
            cstr_to_str(peer),
            cstr_to_str(addr),
            cstr_to_str(code),
        ) else {
            set_err(err_out, "invalid pair argument pointer");
            return std::ptr::null_mut();
        };
        match pqc_api::pair_blocking(Path::new(v), pw, peer, addr, code, listen != 0) {
            Ok(vk) => to_cstring(vk),
            Err(e) => {
                set_err(err_out, e.to_string());
                std::ptr::null_mut()
            }
        }
    }
    #[cfg(not(feature = "pqc"))]
    {
        let _ = (
            vault_path,
            password,
            peer,
            addr,
            code,
            listen,
            Path::new("."),
        );
        set_err(err_out, "built without pqc feature");
        std::ptr::null_mut()
    }
}

/// `1` if peer is pinned in the vault.
///
/// # Safety
/// Pointers must be valid C strings / out-params.
#[no_mangle]
pub unsafe extern "C" fn sb_has_peer(
    vault_path: *const c_char,
    password: *const c_char,
    peer: *const c_char,
    err_out: *mut *mut c_char,
) -> c_int {
    #[cfg(feature = "pqc")]
    {
        let (Ok(v), Ok(pw), Ok(peer)) = (
            cstr_to_str(vault_path),
            cstr_to_str(password),
            cstr_to_str(peer),
        ) else {
            set_err(err_out, "invalid argument pointer");
            return ERR;
        };
        match pqc_api::has_pinned_peer(Path::new(v), pw, peer) {
            Ok(true) => 1,
            Ok(false) => 0,
            Err(e) => {
                set_err(err_out, e.to_string());
                ERR
            }
        }
    }
    #[cfg(not(feature = "pqc"))]
    {
        let _ = (vault_path, password, peer, Path::new("."));
        set_err(err_out, "built without pqc feature");
        ERR
    }
}

/// Snapshot `source` into `repo` (creating repo if needed) and push latest to peer.
///
/// # Safety
/// Pointers must be valid C strings / out-params.
#[no_mangle]
pub unsafe extern "C" fn sb_snapshot_and_push(
    repo: *const c_char,
    source: *const c_char,
    vault_path: *const c_char,
    password: *const c_char,
    peer: *const c_char,
    addr: *const c_char,
    message: *const c_char,
    err_out: *mut *mut c_char,
) -> *mut c_char {
    #[cfg(feature = "pqc")]
    {
        let (Ok(repo), Ok(source), Ok(vault), Ok(pw), Ok(peer), Ok(addr)) = (
            cstr_to_str(repo),
            cstr_to_str(source),
            cstr_to_str(vault_path),
            cstr_to_str(password),
            cstr_to_str(peer),
            cstr_to_str(addr),
        ) else {
            set_err(err_out, "invalid argument pointer");
            return std::ptr::null_mut();
        };
        let msg = if message.is_null() {
            None
        } else {
            cstr_to_str(message).ok()
        };
        match pqc_api::snapshot_and_push(
            Path::new(repo),
            Path::new(source),
            Path::new(vault),
            pw,
            peer,
            addr,
            msg,
        ) {
            Ok(s) => to_cstring(s),
            Err(e) => {
                set_err(err_out, e.to_string());
                std::ptr::null_mut()
            }
        }
    }
    #[cfg(not(feature = "pqc"))]
    {
        let _ = (
            repo,
            source,
            vault_path,
            password,
            peer,
            addr,
            message,
            Path::new("."),
        );
        set_err(err_out, "built without pqc feature");
        std::ptr::null_mut()
    }
}

/// Push an existing repo to a paired peer.
///
/// # Safety
/// Pointers must be valid C strings / out-params.
#[no_mangle]
pub unsafe extern "C" fn sb_push(
    repo: *const c_char,
    vault_path: *const c_char,
    password: *const c_char,
    peer: *const c_char,
    addr: *const c_char,
    latest_only: c_int,
    err_out: *mut *mut c_char,
) -> *mut c_char {
    #[cfg(feature = "pqc")]
    {
        let (Ok(repo), Ok(vault), Ok(pw), Ok(peer), Ok(addr)) = (
            cstr_to_str(repo),
            cstr_to_str(vault_path),
            cstr_to_str(password),
            cstr_to_str(peer),
            cstr_to_str(addr),
        ) else {
            set_err(err_out, "invalid argument pointer");
            return std::ptr::null_mut();
        };
        match pqc_api::push_blocking(
            Path::new(repo),
            Path::new(vault),
            pw,
            peer,
            addr,
            latest_only != 0,
        ) {
            Ok(s) => to_cstring(s),
            Err(e) => {
                set_err(err_out, e.to_string());
                std::ptr::null_mut()
            }
        }
    }
    #[cfg(not(feature = "pqc"))]
    {
        let _ = (
            repo,
            vault_path,
            password,
            peer,
            addr,
            latest_only,
            Path::new("."),
        );
        set_err(err_out, "built without pqc feature");
        std::ptr::null_mut()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::CString;

    #[test]
    fn parse_via_c_abi() {
        let payload = CString::new("simple-backups:v1:pair:10.0.0.1:9876:abcd").unwrap();
        let mut addr: *mut c_char = std::ptr::null_mut();
        let mut code: *mut c_char = std::ptr::null_mut();
        let mut err: *mut c_char = std::ptr::null_mut();
        let rc = unsafe { sb_parse_pair_payload(payload.as_ptr(), &mut addr, &mut code, &mut err) };
        assert_eq!(rc, OK);
        assert!(err.is_null());
        let a = unsafe { CStr::from_ptr(addr) }.to_str().unwrap();
        let c = unsafe { CStr::from_ptr(code) }.to_str().unwrap();
        assert_eq!(a, "10.0.0.1:9876");
        assert_eq!(c, "abcd");
        unsafe {
            sb_string_free(addr);
            sb_string_free(code);
        }
    }

    #[cfg(feature = "pqc")]
    #[test]
    fn identity_ensure_via_c_abi() {
        let dir = tempfile::tempdir().unwrap();
        let vault = dir.path().join("vault.bin");
        let vault_c = CString::new(vault.to_str().unwrap()).unwrap();
        let pass = CString::new("test-passphrase-not-for-prod").unwrap();
        let mut err: *mut c_char = std::ptr::null_mut();
        let vk = unsafe { sb_identity_ensure(vault_c.as_ptr(), pass.as_ptr(), &mut err) };
        assert!(err.is_null(), "err set");
        assert!(!vk.is_null());
        let hex = unsafe { CStr::from_ptr(vk) }.to_str().unwrap();
        assert!(hex.len() > 32);
        unsafe { sb_string_free(vk) };
        assert!(vault.exists());
    }
}
