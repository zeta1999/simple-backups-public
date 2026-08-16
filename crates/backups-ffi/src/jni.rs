//! JNI bindings for Android (`libbackups_ffi.so`).

use backups_core::{format_pair_payload, parse_pair_payload};
use jni::objects::{JClass, JObject, JObjectArray, JString};
use jni::sys::{jboolean, jint, jobjectArray};
use jni::JNIEnv;
use std::path::Path;

fn jstr(env: &mut JNIEnv, s: &JString<'_>) -> Result<String, String> {
    env.get_string(s)
        .map(|js| js.into())
        .map_err(|e| e.to_string())
}

fn jerr(env: &mut JNIEnv, msg: impl AsRef<str>) {
    let _ = env.throw_new("java/lang/RuntimeException", msg.as_ref());
}

#[no_mangle]
pub extern "system" fn Java_com_simpletools_backups_NativeFfi_nativeVersion<'local>(
    env: JNIEnv<'local>,
    _class: JClass<'local>,
) -> JString<'local> {
    env.new_string(env!("CARGO_PKG_VERSION"))
        .unwrap_or_else(|_| env.new_string("").unwrap())
}

#[no_mangle]
pub extern "system" fn Java_com_simpletools_backups_NativeFfi_nativeParsePairPayload<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    payload: JString<'local>,
) -> jobjectArray {
    let Ok(raw) = jstr(&mut env, &payload) else {
        return std::ptr::null_mut();
    };
    let Ok(parsed) = parse_pair_payload(&raw) else {
        return std::ptr::null_mut();
    };
    let string_class = match env.find_class("java/lang/String") {
        Ok(c) => c,
        Err(_) => return std::ptr::null_mut(),
    };
    let arr: JObjectArray = match env.new_object_array(2, &string_class, JObject::null()) {
        Ok(a) => a,
        Err(_) => return std::ptr::null_mut(),
    };
    let Ok(a) = env.new_string(&parsed.addr) else {
        return std::ptr::null_mut();
    };
    let Ok(c) = env.new_string(&parsed.code) else {
        return std::ptr::null_mut();
    };
    if env.set_object_array_element(&arr, 0, a).is_err()
        || env.set_object_array_element(&arr, 1, c).is_err()
    {
        return std::ptr::null_mut();
    }
    arr.into_raw()
}

#[no_mangle]
pub extern "system" fn Java_com_simpletools_backups_NativeFfi_nativeFormatPairPayload<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    addr: JString<'local>,
    code: JString<'local>,
) -> JString<'local> {
    let (Ok(a), Ok(c)) = (jstr(&mut env, &addr), jstr(&mut env, &code)) else {
        return env.new_string("").unwrap();
    };
    env.new_string(format_pair_payload(&a, &c))
        .unwrap_or_else(|_| env.new_string("").unwrap())
}

#[no_mangle]
pub extern "system" fn Java_com_simpletools_backups_NativeFfi_nativePqcEnabled(
    _env: JNIEnv,
    _class: JClass,
) -> jboolean {
    u8::from(cfg!(feature = "pqc"))
}

#[no_mangle]
pub extern "system" fn Java_com_simpletools_backups_NativeFfi_nativeGeneratePairingCode<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
) -> JString<'local> {
    #[cfg(feature = "pqc")]
    {
        match crate::pqc_api::gen_code() {
            Ok(code) => env
                .new_string(code)
                .unwrap_or_else(|_| env.new_string("").unwrap()),
            Err(e) => {
                jerr(&mut env, e.to_string());
                env.new_string("").unwrap()
            }
        }
    }
    #[cfg(not(feature = "pqc"))]
    {
        jerr(&mut env, "built without pqc feature");
        env.new_string("").unwrap()
    }
}

#[no_mangle]
pub extern "system" fn Java_com_simpletools_backups_NativeFfi_nativeIdentityEnsure<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    vault: JString<'local>,
    password: JString<'local>,
) -> JString<'local> {
    #[cfg(feature = "pqc")]
    {
        let (Ok(v), Ok(p)) = (jstr(&mut env, &vault), jstr(&mut env, &password)) else {
            jerr(&mut env, "bad args");
            return env.new_string("").unwrap();
        };
        match crate::pqc_api::identity_ensure(Path::new(&v), &p) {
            Ok(vk) => env
                .new_string(vk)
                .unwrap_or_else(|_| env.new_string("").unwrap()),
            Err(e) => {
                jerr(&mut env, e.to_string());
                env.new_string("").unwrap()
            }
        }
    }
    #[cfg(not(feature = "pqc"))]
    {
        let _ = (vault, password, Path::new("."));
        jerr(&mut env, "built without pqc feature");
        env.new_string("").unwrap()
    }
}

#[no_mangle]
pub extern "system" fn Java_com_simpletools_backups_NativeFfi_nativePair<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    vault: JString<'local>,
    password: JString<'local>,
    peer: JString<'local>,
    addr: JString<'local>,
    code: JString<'local>,
    listen: jint,
) -> JString<'local> {
    #[cfg(feature = "pqc")]
    {
        let (Ok(v), Ok(pw), Ok(peer), Ok(addr), Ok(code)) = (
            jstr(&mut env, &vault),
            jstr(&mut env, &password),
            jstr(&mut env, &peer),
            jstr(&mut env, &addr),
            jstr(&mut env, &code),
        ) else {
            jerr(&mut env, "bad args");
            return env.new_string("").unwrap();
        };
        match crate::pqc_api::pair_blocking(Path::new(&v), &pw, &peer, &addr, &code, listen != 0) {
            Ok(vk) => env
                .new_string(vk)
                .unwrap_or_else(|_| env.new_string("").unwrap()),
            Err(e) => {
                jerr(&mut env, e.to_string());
                env.new_string("").unwrap()
            }
        }
    }
    #[cfg(not(feature = "pqc"))]
    {
        let _ = (vault, password, peer, addr, code, listen, Path::new("."));
        jerr(&mut env, "built without pqc feature");
        env.new_string("").unwrap()
    }
}

#[no_mangle]
pub extern "system" fn Java_com_simpletools_backups_NativeFfi_nativeHasPeer(
    mut env: JNIEnv,
    _class: JClass,
    vault: JString,
    password: JString,
    peer: JString,
) -> jboolean {
    #[cfg(feature = "pqc")]
    {
        let (Ok(v), Ok(pw), Ok(peer)) = (
            jstr(&mut env, &vault),
            jstr(&mut env, &password),
            jstr(&mut env, &peer),
        ) else {
            return 0;
        };
        match crate::pqc_api::has_pinned_peer(Path::new(&v), &pw, &peer) {
            Ok(true) => 1,
            _ => 0,
        }
    }
    #[cfg(not(feature = "pqc"))]
    {
        let _ = (vault, password, peer, &mut env, Path::new("."));
        0
    }
}

#[no_mangle]
pub extern "system" fn Java_com_simpletools_backups_NativeFfi_nativeSnapshotAndPush<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    repo: JString<'local>,
    source: JString<'local>,
    vault: JString<'local>,
    password: JString<'local>,
    peer: JString<'local>,
    addr: JString<'local>,
    message: JString<'local>,
) -> JString<'local> {
    #[cfg(feature = "pqc")]
    {
        let (Ok(repo), Ok(source), Ok(vault), Ok(pw), Ok(peer), Ok(addr), Ok(message)) = (
            jstr(&mut env, &repo),
            jstr(&mut env, &source),
            jstr(&mut env, &vault),
            jstr(&mut env, &password),
            jstr(&mut env, &peer),
            jstr(&mut env, &addr),
            jstr(&mut env, &message),
        ) else {
            jerr(&mut env, "bad args");
            return env.new_string("").unwrap();
        };
        let msg = if message.is_empty() {
            None
        } else {
            Some(message.as_str())
        };
        match crate::pqc_api::snapshot_and_push(
            Path::new(&repo),
            Path::new(&source),
            Path::new(&vault),
            &pw,
            &peer,
            &addr,
            msg,
        ) {
            Ok(s) => env
                .new_string(s)
                .unwrap_or_else(|_| env.new_string("").unwrap()),
            Err(e) => {
                jerr(&mut env, e.to_string());
                env.new_string("").unwrap()
            }
        }
    }
    #[cfg(not(feature = "pqc"))]
    {
        let _ = (
            repo,
            source,
            vault,
            password,
            peer,
            addr,
            message,
            Path::new("."),
        );
        jerr(&mut env, "built without pqc feature");
        env.new_string("").unwrap()
    }
}
