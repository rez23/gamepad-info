use alloc::vec::Vec;
use swdk::rt::wdk_sys;
use swdk_macros::CtxDescriptor;
use crate::device::models::GamepadModels;

pub mod models;
pub mod policy;

#[derive(Default, CtxDescriptor)]
pub struct DeviceData {
    pub release: bool,
    pub model: GamepadModels,
    pub allowed_pid: Vec<u16>,
}