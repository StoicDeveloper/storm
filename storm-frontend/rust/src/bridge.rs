#![allow(
    non_camel_case_types,
    unused,
    clippy::redundant_closure,
    clippy::useless_conversion,
    clippy::unit_arg,
    clippy::double_parens,
    non_snake_case
)]
// AUTO GENERATED FILE, DO NOT EDIT.
// Generated by `flutter_rust_bridge`.

use crate::lib::*;
use flutter_rust_bridge::*;

// Section: imports

// Section: wire functions

#[no_mangle]
pub extern "C" fn wire_start(port_: i64) {
    FLUTTER_RUST_BRIDGE_HANDLER.wrap(
        WrapInfo {
            debug_name: "start",
            port: Some(port_),
            mode: FfiCallMode::Normal,
        },
        move || move |task_callback| Ok(start()),
    )
}

#[no_mangle]
pub extern "C" fn wire_createAccount(port_: i64, num1: u8, num2: u8) {
    FLUTTER_RUST_BRIDGE_HANDLER.wrap(
        WrapInfo {
            debug_name: "createAccount",
            port: Some(port_),
            mode: FfiCallMode::Normal,
        },
        move || {
            let api_num1 = num1.wire2api();
            let api_num2 = num2.wire2api();
            move |task_callback| Ok(createAccount(api_num1, api_num2))
        },
    )
}

#[no_mangle]
pub extern "C" fn wire_createAlias(port_: i64) {
    FLUTTER_RUST_BRIDGE_HANDLER.wrap(
        WrapInfo {
            debug_name: "createAlias",
            port: Some(port_),
            mode: FfiCallMode::Normal,
        },
        move || move |task_callback| Ok(createAlias()),
    )
}

#[no_mangle]
pub extern "C" fn wire_login(
    port_: i64,
    username: *mut wire_uint_8_list,
    password: *mut wire_uint_8_list,
) {
    FLUTTER_RUST_BRIDGE_HANDLER.wrap(
        WrapInfo {
            debug_name: "login",
            port: Some(port_),
            mode: FfiCallMode::Normal,
        },
        move || {
            let api_username = username.wire2api();
            let api_password = password.wire2api();
            move |task_callback| Ok(login(api_username, api_password))
        },
    )
}

#[no_mangle]
pub extern "C" fn wire_getGroups(port_: i64, alias: u16) {
    FLUTTER_RUST_BRIDGE_HANDLER.wrap(
        WrapInfo {
            debug_name: "getGroups",
            port: Some(port_),
            mode: FfiCallMode::Normal,
        },
        move || {
            let api_alias = alias.wire2api();
            move |task_callback| Ok(getGroups(api_alias))
        },
    )
}

#[no_mangle]
pub extern "C" fn wire_getGroupMessages(port_: i64, group: u16) {
    FLUTTER_RUST_BRIDGE_HANDLER.wrap(
        WrapInfo {
            debug_name: "getGroupMessages",
            port: Some(port_),
            mode: FfiCallMode::Normal,
        },
        move || {
            let api_group = group.wire2api();
            move |task_callback| Ok(getGroupMessages(api_group))
        },
    )
}

#[no_mangle]
pub extern "C" fn wire_sendGroupMessage(port_: i64, group: u16, msg: *mut wire_uint_8_list) {
    FLUTTER_RUST_BRIDGE_HANDLER.wrap(
        WrapInfo {
            debug_name: "sendGroupMessage",
            port: Some(port_),
            mode: FfiCallMode::Normal,
        },
        move || {
            let api_group = group.wire2api();
            let api_msg = msg.wire2api();
            move |task_callback| Ok(sendGroupMessage(api_group, api_msg))
        },
    )
}

#[no_mangle]
pub extern "C" fn wire_setGroupSettings(port_: i64, group: u16, msg: *mut wire_uint_8_list) {
    FLUTTER_RUST_BRIDGE_HANDLER.wrap(
        WrapInfo {
            debug_name: "setGroupSettings",
            port: Some(port_),
            mode: FfiCallMode::Normal,
        },
        move || {
            let api_group = group.wire2api();
            let api_msg = msg.wire2api();
            move |task_callback| Ok(setGroupSettings(api_group, api_msg))
        },
    )
}

// Section: wire structs

#[repr(C)]
#[derive(Clone)]
pub struct wire_uint_8_list {
    ptr: *mut u8,
    len: i32,
}

// Section: wrapper structs

// Section: static checks

// Section: allocate functions

#[no_mangle]
pub extern "C" fn new_uint_8_list_0(len: i32) -> *mut wire_uint_8_list {
    let ans = wire_uint_8_list {
        ptr: support::new_leak_vec_ptr(Default::default(), len),
        len,
    };
    support::new_leak_box_ptr(ans)
}

// Section: impl Wire2Api

pub trait Wire2Api<T> {
    fn wire2api(self) -> T;
}

impl<T, S> Wire2Api<Option<T>> for *mut S
where
    *mut S: Wire2Api<T>,
{
    fn wire2api(self) -> Option<T> {
        if self.is_null() {
            None
        } else {
            Some(self.wire2api())
        }
    }
}

impl Wire2Api<String> for *mut wire_uint_8_list {
    fn wire2api(self) -> String {
        let vec: Vec<u8> = self.wire2api();
        String::from_utf8_lossy(&vec).into_owned()
    }
}

impl Wire2Api<u16> for u16 {
    fn wire2api(self) -> u16 {
        self
    }
}

impl Wire2Api<u8> for u8 {
    fn wire2api(self) -> u8 {
        self
    }
}

impl Wire2Api<Vec<u8>> for *mut wire_uint_8_list {
    fn wire2api(self) -> Vec<u8> {
        unsafe {
            let wrap = support::box_from_leak_ptr(self);
            support::vec_from_leak_ptr(wrap.ptr, wrap.len)
        }
    }
}

// Section: impl NewWithNullPtr

pub trait NewWithNullPtr {
    fn new_with_null_ptr() -> Self;
}

impl<T> NewWithNullPtr for *mut T {
    fn new_with_null_ptr() -> Self {
        std::ptr::null_mut()
    }
}

// Section: impl IntoDart

// Section: executor

support::lazy_static! {
    pub static ref FLUTTER_RUST_BRIDGE_HANDLER: support::DefaultHandler = Default::default();
}

// Section: sync execution mode utility

#[no_mangle]
pub extern "C" fn free_WireSyncReturnStruct(val: support::WireSyncReturnStruct) {
    unsafe {
        let _ = support::vec_from_leak_ptr(val.ptr, val.len);
    }
}
