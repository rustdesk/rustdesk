use super::{CodecInfo, HwCodecConfig, CONFIG, CONFIG_SET_BY_IPC};
use crate::{
    codec::enable_vram_option,
    vram::{VRamDecoder, VRamEncoder},
    AdapterDevice, CodecFormat,
};
use base::config::keys::{OPTION_ALLOW_D3D_RENDER, OPTION_ENABLE_HWCODEC};
use hbb_common::config::{OVERWRITE_LOCAL_SETTINGS, OVERWRITE_SETTINGS};
use hwcodec::{
    common::{DataFormat, Driver},
    vram::{DecodeContext, FeatureContext},
};
use std::{collections::HashMap, mem};

struct RestoreConfig {
    config: Option<HwCodecConfig>,
    set_by_ipc: bool,
    options: HashMap<String, String>,
    local_options: HashMap<String, String>,
}

impl Drop for RestoreConfig {
    fn drop(&mut self) {
        *CONFIG.lock().unwrap() = self.config.take();
        *CONFIG_SET_BY_IPC.lock().unwrap() = self.set_by_ipc;
        *OVERWRITE_SETTINGS.write().unwrap() = mem::take(&mut self.options);
        *OVERWRITE_LOCAL_SETTINGS.write().unwrap() = mem::take(&mut self.local_options);
    }
}

fn add_backend(config: &mut HwCodecConfig, driver: Driver, vendor: Driver, format: DataFormat) {
    let luid = match vendor {
        Driver::NV => 1,
        Driver::MFX => 2,
        Driver::AMF => 3,
        _ => unreachable!(),
    };
    config.vram_encode.push(FeatureContext {
        driver: driver.clone(),
        vendor: vendor.clone(),
        luid,
        data_format: format,
    });
    config.vram_decode.push(DecodeContext {
        device: None,
        driver,
        vendor,
        luid,
        data_format: format,
    });
}

#[test]
fn cached_sdk_entries_are_ignored_before_check_completes() {
    let _restore = RestoreConfig {
        config: CONFIG.lock().unwrap().take(),
        set_by_ipc: mem::replace(&mut *CONFIG_SET_BY_IPC.lock().unwrap(), false),
        options: mem::take(&mut *OVERWRITE_SETTINGS.write().unwrap()),
        local_options: mem::take(&mut *OVERWRITE_LOCAL_SETTINGS.write().unwrap()),
    };
    OVERWRITE_SETTINGS
        .write()
        .unwrap()
        .insert(OPTION_ENABLE_HWCODEC.into(), "Y".into());
    OVERWRITE_LOCAL_SETTINGS
        .write()
        .unwrap()
        .insert(OPTION_ALLOW_D3D_RENDER.into(), "Y".into());
    assert!(enable_vram_option(false));

    let formats = [
        (CodecFormat::H264, DataFormat::H264),
        (CodecFormat::H265, DataFormat::H265),
    ];
    let mut cached = HwCodecConfig::default();
    for (_, format) in formats {
        // A RAM fallback keeps this selection test independent of attached displays.
        cached.ram_encode.push(CodecInfo {
            format,
            ..Default::default()
        });
        for vendor in [Driver::NV, Driver::MFX, Driver::AMF] {
            add_backend(&mut cached, vendor.clone(), vendor, format);
        }
    }
    *CONFIG.lock().unwrap() = Some(cached.clone());
    assert!(!HwCodecConfig::already_set());
    for (format, _) in formats {
        assert!(VRamEncoder::available(format).is_empty());
        for luid in 1..=3 {
            assert!(VRamDecoder::try_get(format, Some(luid)).is_none());
        }
    }
    assert_eq!(
        VRamDecoder::possible_available_without_check(),
        (false, false)
    );

    for (index, &(format, data_format)) in formats.iter().enumerate() {
        for vendor in [Driver::NV, Driver::MFX, Driver::AMF] {
            add_backend(&mut cached, Driver::FFMPEG, vendor, data_format);
        }
        *CONFIG.lock().unwrap() = Some(cached.clone());
        let encoders = VRamEncoder::available(format);
        assert_eq!(encoders.len(), 3);
        for encoder in encoders {
            assert_eq!(encoder.driver, Driver::FFMPEG);
            let device = AdapterDevice {
                luid: encoder.luid,
                ..Default::default()
            };
            assert_eq!(VRamEncoder::try_get(&device, format), Some(encoder.clone()));
            let decoder = VRamDecoder::try_get(format, Some(encoder.luid)).unwrap();
            assert_eq!(decoder.driver, Driver::FFMPEG);
            assert_eq!(decoder.vendor, encoder.vendor);
            assert_eq!(decoder.data_format, data_format);
        }
        assert!(VRamDecoder::available(format, Some(99)).is_empty());
        assert!(VRamDecoder::available(format, None).is_empty());
        assert_eq!(
            VRamDecoder::possible_available_without_check(),
            (true, index == 1)
        );
    }
    assert!(!HwCodecConfig::already_set());
    assert_eq!(
        HwCodecConfig::get().vram_encode.len(),
        cached.vram_encode.len()
    );
}
