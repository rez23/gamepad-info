#![no_std]
#![feature(
    //trait_alias,
    //lazy_type_alias,
    //associated_type_defaults,
    //min_specialization,
    //generic_const_exprs,
    //type_alias_impl_trait,
    negative_impls
    //impl_trait_in_assoc_type,
)]
extern crate alloc;

mod device;

use core::ops::Deref;

use swdk::bd::{
    WdfDevicePnpPowerSetup, WdfDriverConf, WdfDriverSetup,
    WdfObjAttrs,
};
use swdk::ctx::WdfCtxNoneDesc;
use swdk::ioctl::commands::IOCTL_HID_GET_COLLECTION_INFORMATION;
use swdk::ioctl::{IoCtlRequest, IoCtlResponse};
use swdk::op::{
    AsNtStatus, AsRaw, AsWdfOwned, AsWdfOwner, IntoInner,
    IntoRaw,
};
#[cfg(not(test))]
use swdk::rt::wdk_alloc::WdkAllocator;
use swdk::rt::wdk_sys::{
    HID_COLLECTION_INFORMATION, HID_DEVICE_ATTRIBUTES,
    NTSTATUS, PCUNICODE_STRING, PDRIVER_OBJECT,
    PWDFDEVICE_INIT, STATUS_SUCCESS, STATUS_UNSUCCESSFUL,
    WDF_POWER_DEVICE_STATE, WDFDEVICE, WDFDRIVER,
    WDFIOTARGET,
};
use swdk::vals::WdfIoTargetError::IoCtlTargetSendError;
use swdk::{
    Handle, HandleRef, debug, error,
    if_nterror_return_ntstatus, info, ioctl,
};

use crate::device::DeviceData;
use crate::device::models::GamepadModels;

#[cfg(not(test))]
#[global_allocator]
static GLOBAL_ALLOCATOR: WdkAllocator = WdkAllocator;

/// Main entry point for the KMDF driver.
///
/// # Panics
/// This function may panic if internal string conversions (e.g. `CString::new`)
/// fail due to invalid UTF-8 input. Such panics will trigger the kernel panic
/// handler provided by `wdk_panic`.
///
/// # Safety
/// This function is called directly by the Windows kernel. The pointers
/// `driver` and `registry_path` must be valid for the duration of the call.
/// The caller (the OS) guarantees these invariants. The function must not
/// assume any additional safety beyond what KMDF provides.
#[unsafe(export_name = "DriverEntry")]
pub unsafe extern "system" fn driver_entry(
    driver_obj: PDRIVER_OBJECT,
    registry_path: PCUNICODE_STRING,
) -> NTSTATUS {
    debug!("DriverEntry launched from WDF");
    if_nterror_return_ntstatus!(
        Handle::<WDFDRIVER>::from_owned_with_attrs(
            driver_obj,
            WdfDriverConf {
                setup: WdfDriverSetup {
                    on_driver_unload: Some(
                        on_driver_unload
                    ),
                    on_device_add: Some(
                        on_driver_device_add
                    ),
                    ..WdfDriverSetup::default()
                },
                registry_path,
            },
            Some(WdfObjAttrs::<WdfCtxNoneDesc>::default())
        )
    );
    STATUS_SUCCESS
}

unsafe extern "C" fn on_driver_unload(_driver: WDFDRIVER) {
    info!("Driver unload event triggered.");
}

#[unsafe(link_section = "PAGE")]
unsafe extern "C" fn on_driver_device_add(
    _driver: WDFDRIVER,
    device_init: PWDFDEVICE_INIT,
) -> NTSTATUS {
    debug!("Entering in function on_driver_device_add");
    if_nterror_return_ntstatus!(
        Handle::<WDFDEVICE>::from_owned(
            Handle::new(&device_init)
                .with_filter()
                .with_pnp_setup(WdfDevicePnpPowerSetup {
                    on_device_d0_entry: Some(
                        on_device_d0_entry
                    ),
                    ..WdfDevicePnpPowerSetup::default()
                })
                .raw(),
            Some(WdfObjAttrs::<DeviceData>::default())
        )
    );
    STATUS_SUCCESS
}

unsafe extern "C" fn on_device_d0_entry(
    device: WDFDEVICE,
    previous_state: WDF_POWER_DEVICE_STATE,
) -> NTSTATUS {
    debug!("D0Entry");

    let device_handle = Handle::<WDFDEVICE>::new(device);

    debug!("Getting device capabilities");
    let iot_handler = if_nterror_return_ntstatus!(
        Handle::<WDFIOTARGET>::from_owner(&device_handle)
    );

    // get device capabilities
    let device_info: IoCtlResponse<HID_COLLECTION_INFORMATION> = if_nterror_return_ntstatus!(
        iot_handler.send_ioctl_sync(IoCtlRequest::with_command(
            IOCTL_HID_GET_COLLECTION_INFORMATION
        )).map_err(|err| {
            match err {
                IoCtlTargetSendError(status) => {
                    let command = status.command;
                    let status_name = status.ntstatus.fmt_status();
                    let hex = status.ntstatus.fmt_hex();

                    error!(
                        "'WdfIoTargetSendIoctlSynchronously' \
                        failed for command \
                        '0x{:08X} with {} status ({})",
                        command,
                        status_name,
                        hex,
                    );

                    STATUS_UNSUCCESSFUL
                },
                err => {
                    error!("Failed to get device capabilities from IOCTL: {:?}", err);
                    STATUS_UNSUCCESSFUL
                }
            }
        }
    ));

    debug!(
        "VID={:04X} PID={:04X} VER={:04X}",
        device_info.VendorID,
        device_info.ProductID,
        device_info.VersionNumber,
    );

    let gamepad_model = GamepadModels::from_vid_and_pid(
        device_info.ProductID,
        device_info.VendorID,
    );

    info!("Device capabilities: {}", gamepad_model);
    STATUS_SUCCESS
}
