/// Source = microphone etc. something that takes in audio
/// Source Output = application consuming that audio
///
/// Sink = headphones etc. something that plays out audio
/// Sink Input = application producing that audio
/// When you create a `SinkController`, you are working with audio playback devices and applications
/// if you want to manipulate recording devices such as microphone volume,
/// you'll need to use a `SourceController`. Both of these implement the same api, defined by
/// the traits DeviceControl and AppControl
use std::cell::RefCell;
use std::clone::Clone;
use std::rc::Rc;

use pulse::{
    callbacks::ListResult,
    context::introspect,
    volume::{ChannelVolumes, Volume},
};

use errors::{ControllerError, ControllerErrorType::*};
use types::{ApplicationInfo, DeviceInfo, ServerInfo};

use crate::Handler;

pub(crate) mod errors;
pub mod types;

pub trait DeviceControl<T> {
    fn get_default_device(&mut self) -> Result<T, ControllerError>;
    fn set_default_device(&mut self, name: &str) -> Result<bool, ControllerError>;

    fn list_devices(&mut self) -> Result<Vec<T>, ControllerError>;
    fn get_device_by_index(&mut self, index: u32) -> Result<T, ControllerError>;
    fn get_device_by_name(&mut self, name: &str) -> Result<T, ControllerError>;
    fn set_device_volume_by_index(&mut self, index: u32, volume: &ChannelVolumes);
    fn set_device_volume_by_name(&mut self, name: &str, volume: &ChannelVolumes);
    fn increase_device_volume_by_percent(&mut self, index: u32, delta: f64);
    fn decrease_device_volume_by_percent(&mut self, index: u32, delta: f64);
}

pub trait AppControl<T> {
    fn list_applications(&mut self) -> Result<Vec<T>, ControllerError>;

    fn get_app_by_index(&mut self, index: u32) -> Result<T, ControllerError>;
    fn increase_app_volume_by_percent(&mut self, index: u32, delta: f64);
    fn decrease_app_volume_by_percent(&mut self, index: u32, delta: f64);

    fn move_app_by_index(
        &mut self,
        stream_index: u32,
        device_index: u32,
    ) -> Result<bool, ControllerError>;
    fn move_app_by_name(
        &mut self,
        stream_index: u32,
        device_name: &str,
    ) -> Result<bool, ControllerError>;
    fn set_app_mute(&mut self, index: u32, mute: bool) -> Result<bool, ControllerError>;
}

fn volume_from_percent(volume: f64) -> f64 {
    (volume * 100.0) * (f64::from(pulse::volume::VOLUME_NORM.0) / 100.0)
}

pub struct SinkController {
    pub handler: Handler,
}

impl SinkController {
    pub fn create() -> Result<Self, ControllerError> {
        let handler = Handler::connect("SinkController")?;
        Ok(SinkController { handler })
    }

    pub fn get_server_info(&mut self) -> Result<ServerInfo, ControllerError> {
        let server = Rc::new(RefCell::new(Some(None)));
        let server_ref = server.clone();

        let op = self.handler.introspect.get_server_info(move |res| {
            server_ref
                .borrow_mut()
                .as_mut()
                .unwrap()
                .replace(res.into());
        });
        self.handler.wait_for_operation(op)?;
        let mut result = server.borrow_mut();
        result.take().unwrap().ok_or(ControllerError::new(
            GetInfoError,
            "Error getting information about the server",
        ))
    }
}

impl DeviceControl<DeviceInfo> for SinkController {
    fn get_default_device(&mut self) -> Result<DeviceInfo, ControllerError> {
        let server_info = self.get_server_info();
        match server_info {
            Ok(info) => self.get_device_by_name(info.default_sink_name.unwrap().as_ref()),
            Err(e) => Err(e),
        }
    }
    fn set_default_device(&mut self, name: &str) -> Result<bool, ControllerError> {
        let success = Rc::new(RefCell::new(false));
        let success_ref = success.clone();

        let op = self
            .handler
            .context
            .borrow_mut()
            .set_default_sink(name, move |res| success_ref.borrow_mut().clone_from(&res));
        self.handler.wait_for_operation(op)?;
        let result = success.borrow_mut().clone();
        Ok(result)
    }

    fn list_devices(&mut self) -> Result<Vec<DeviceInfo>, ControllerError> {
        let list = Rc::new(RefCell::new(Some(Vec::new())));
        let list_ref = list.clone();

        let op = self.handler.introspect.get_sink_info_list(
            move |sink_list: ListResult<&introspect::SinkInfo>| {
                if let ListResult::Item(item) = sink_list {
                    list_ref.borrow_mut().as_mut().unwrap().push(item.into());
                }
            },
        );
        self.handler.wait_for_operation(op)?;
        let mut result = list.borrow_mut();
        result.take().ok_or(ControllerError::new(
            GetInfoError,
            "Error getting device list",
        ))
    }
    fn get_device_by_index(&mut self, index: u32) -> Result<DeviceInfo, ControllerError> {
        let device = Rc::new(RefCell::new(Some(None)));
        let dev_ref = device.clone();
        let op = self.handler.introspect.get_sink_info_by_index(
            index,
            move |sink_list: ListResult<&introspect::SinkInfo>| {
                if let ListResult::Item(item) = sink_list {
                    dev_ref.borrow_mut().as_mut().unwrap().replace(item.into());
                }
            },
        );
        self.handler.wait_for_operation(op)?;
        let mut result = device.borrow_mut();
        result.take().unwrap().ok_or(ControllerError::new(
            GetInfoError,
            "Error getting requested device",
        ))
    }
    fn get_device_by_name(&mut self, name: &str) -> Result<DeviceInfo, ControllerError> {
        let device = Rc::new(RefCell::new(Some(None)));
        let dev_ref = device.clone();
        let op = self.handler.introspect.get_sink_info_by_name(
            name,
            move |sink_list: ListResult<&introspect::SinkInfo>| {
                if let ListResult::Item(item) = sink_list {
                    dev_ref.borrow_mut().as_mut().unwrap().replace(item.into());
                }
            },
        );
        self.handler.wait_for_operation(op)?;
        let mut result = device.borrow_mut();
        result.take().unwrap().ok_or(ControllerError::new(
            GetInfoError,
            "Error getting requested device",
        ))
    }

    fn set_device_volume_by_index(&mut self, index: u32, volume: &ChannelVolumes) {
        let op = self
            .handler
            .introspect
            .set_sink_volume_by_index(index, volume, None);
        self.handler.wait_for_operation(op).ok();
    }
    fn set_device_volume_by_name(&mut self, name: &str, volume: &ChannelVolumes) {
        let op = self
            .handler
            .introspect
            .set_sink_volume_by_name(name, volume, None);
        self.handler.wait_for_operation(op).ok();
    }
    fn increase_device_volume_by_percent(&mut self, index: u32, delta: f64) {
        if let Ok(mut dev_ref) = self.get_device_by_index(index) {
            let new_vol = Volume::from(Volume(volume_from_percent(delta) as u32));
            if let Some(volumes) = dev_ref.volume.increase(new_vol) {
                let op = self
                    .handler
                    .introspect
                    .set_sink_volume_by_index(index, &volumes, None);
                self.handler.wait_for_operation(op).ok();
            }
        }
    }
    fn decrease_device_volume_by_percent(&mut self, index: u32, delta: f64) {
        if let Ok(mut dev_ref) = self.get_device_by_index(index) {
            let new_vol = Volume::from(Volume(volume_from_percent(delta) as u32));
            if let Some(volumes) = dev_ref.volume.decrease(new_vol) {
                let op = self
                    .handler
                    .introspect
                    .set_sink_volume_by_index(index, &volumes, None);
                self.handler.wait_for_operation(op).ok();
            }
        }
    }
}

impl AppControl<ApplicationInfo> for SinkController {
    fn list_applications(&mut self) -> Result<Vec<ApplicationInfo>, ControllerError> {
        let list = Rc::new(RefCell::new(Some(Vec::new())));
        let list_ref = list.clone();

        let op = self.handler.introspect.get_sink_input_info_list(
            move |sink_list: ListResult<&introspect::SinkInputInfo>| {
                if let ListResult::Item(item) = sink_list {
                    list_ref.borrow_mut().as_mut().unwrap().push(item.into());
                }
            },
        );
        self.handler.wait_for_operation(op)?;
        let mut result = list.borrow_mut();
        result.take().ok_or(ControllerError::new(
            GetInfoError,
            "Error getting application list",
        ))
    }

    fn get_app_by_index(&mut self, index: u32) -> Result<ApplicationInfo, ControllerError> {
        let app = Rc::new(RefCell::new(Some(None)));
        let app_ref = app.clone();
        let op = self.handler.introspect.get_sink_input_info(
            index,
            move |sink_list: ListResult<&introspect::SinkInputInfo>| {
                if let ListResult::Item(item) = sink_list {
                    app_ref.borrow_mut().as_mut().unwrap().replace(item.into());
                }
            },
        );
        self.handler.wait_for_operation(op)?;
        let mut result = app.borrow_mut();
        result.take().unwrap().ok_or(ControllerError::new(
            GetInfoError,
            "Error getting requested app",
        ))
    }

    fn increase_app_volume_by_percent(&mut self, index: u32, delta: f64) {
        if let Ok(mut app_ref) = self.get_app_by_index(index) {
            let new_vol = Volume::from(Volume(volume_from_percent(delta) as u32));
            if let Some(volumes) = app_ref.volume.increase(new_vol) {
                let op = self
                    .handler
                    .introspect
                    .set_sink_input_volume(index, &volumes, None);
                self.handler.wait_for_operation(op).ok();
            }
        }
    }

    fn decrease_app_volume_by_percent(&mut self, index: u32, delta: f64) {
        if let Ok(mut app_ref) = self.get_app_by_index(index) {
            let new_vol = Volume::from(Volume(volume_from_percent(delta) as u32));
            if let Some(volumes) = app_ref.volume.decrease(new_vol) {
                let op = self
                    .handler
                    .introspect
                    .set_sink_input_volume(index, &volumes, None);
                self.handler.wait_for_operation(op).ok();
            }
        }
    }

    fn move_app_by_index(
        &mut self,
        stream_index: u32,
        device_index: u32,
    ) -> Result<bool, ControllerError> {
        let success = Rc::new(RefCell::new(false));
        let success_ref = success.clone();
        let op = self.handler.introspect.move_sink_input_by_index(
            stream_index,
            device_index,
            Some(Box::new(move |res| {
                success_ref.borrow_mut().clone_from(&res)
            })),
        );
        self.handler.wait_for_operation(op)?;
        let result = success.borrow_mut().clone();
        Ok(result)
    }

    fn move_app_by_name(
        &mut self,
        stream_index: u32,
        device_name: &str,
    ) -> Result<bool, ControllerError> {
        let success = Rc::new(RefCell::new(false));
        let success_ref = success.clone();
        let op = self.handler.introspect.move_sink_input_by_name(
            stream_index,
            device_name,
            Some(Box::new(move |res| {
                success_ref.borrow_mut().clone_from(&res)
            })),
        );
        self.handler.wait_for_operation(op)?;
        let result = success.borrow_mut().clone();
        Ok(result)
    }

    fn set_app_mute(&mut self, index: u32, mute: bool) -> Result<bool, ControllerError> {
        let success = Rc::new(RefCell::new(false));
        let success_ref = success.clone();
        let op = self.handler.introspect.set_sink_input_mute(
            index,
            mute,
            Some(Box::new(move |res| {
                success_ref.borrow_mut().clone_from(&res)
            })),
        );
        self.handler.wait_for_operation(op)?;
        let result = success.borrow_mut().clone();
        Ok(result)
    }
}

pub struct SourceController {
    pub handler: Handler,
}

impl SourceController {
    pub fn create() -> Result<Self, ControllerError> {
        let handler = Handler::connect("SourceController")?;
        Ok(SourceController { handler })
    }

    pub fn get_server_info(&mut self) -> Result<ServerInfo, ControllerError> {
        let server = Rc::new(RefCell::new(Some(None)));
        let server_ref = server.clone();

        let op = self.handler.introspect.get_server_info(move |res| {
            server_ref
                .borrow_mut()
                .as_mut()
                .unwrap()
                .replace(res.into());
        });
        self.handler.wait_for_operation(op)?;
        let mut result = server.borrow_mut();
        result.take().unwrap().ok_or(ControllerError::new(
            GetInfoError,
            "Error getting application list",
        ))
    }
}

impl DeviceControl<DeviceInfo> for SourceController {
    fn get_default_device(&mut self) -> Result<DeviceInfo, ControllerError> {
        let server_info = self.get_server_info();
        match server_info {
            Ok(info) => self.get_device_by_name(info.default_sink_name.unwrap().as_ref()),
            Err(e) => Err(e),
        }
    }
    fn set_default_device(&mut self, name: &str) -> Result<bool, ControllerError> {
        let success = Rc::new(RefCell::new(false));
        let success_ref = success.clone();

        let op = self
            .handler
            .context
            .borrow_mut()
            .set_default_source(name, move |res| success_ref.borrow_mut().clone_from(&res));
        self.handler.wait_for_operation(op)?;
        let result = success.borrow_mut().clone();
        Ok(result)
    }

    fn list_devices(&mut self) -> Result<Vec<DeviceInfo>, ControllerError> {
        let list = Rc::new(RefCell::new(Some(Vec::new())));
        let list_ref = list.clone();

        let op = self.handler.introspect.get_source_info_list(
            move |sink_list: ListResult<&introspect::SourceInfo>| {
                if let ListResult::Item(item) = sink_list {
                    list_ref.borrow_mut().as_mut().unwrap().push(item.into());
                }
            },
        );
        self.handler.wait_for_operation(op)?;
        let mut result = list.borrow_mut();
        result.take().ok_or(ControllerError::new(
            GetInfoError,
            "Error getting application list",
        ))
    }
    fn get_device_by_index(&mut self, index: u32) -> Result<DeviceInfo, ControllerError> {
        let device = Rc::new(RefCell::new(Some(None)));
        let dev_ref = device.clone();
        let op = self.handler.introspect.get_source_info_by_index(
            index,
            move |sink_list: ListResult<&introspect::SourceInfo>| {
                if let ListResult::Item(item) = sink_list {
                    dev_ref.borrow_mut().as_mut().unwrap().replace(item.into());
                }
            },
        );
        self.handler.wait_for_operation(op)?;
        let mut result = device.borrow_mut();
        result.take().unwrap().ok_or(ControllerError::new(
            GetInfoError,
            "Error getting application list",
        ))
    }
    fn get_device_by_name(&mut self, name: &str) -> Result<DeviceInfo, ControllerError> {
        let device = Rc::new(RefCell::new(Some(None)));
        let dev_ref = device.clone();
        let op = self.handler.introspect.get_source_info_by_name(
            name,
            move |sink_list: ListResult<&introspect::SourceInfo>| {
                if let ListResult::Item(item) = sink_list {
                    dev_ref.borrow_mut().as_mut().unwrap().replace(item.into());
                }
            },
        );
        self.handler.wait_for_operation(op)?;
        let mut result = device.borrow_mut();
        result.take().unwrap().ok_or(ControllerError::new(
            GetInfoError,
            "Error getting application list",
        ))
    }

    fn set_device_volume_by_index(&mut self, index: u32, volume: &ChannelVolumes) {
        let op = self
            .handler
            .introspect
            .set_source_volume_by_index(index, volume, None);
        self.handler.wait_for_operation(op).ok();
    }
    fn set_device_volume_by_name(&mut self, name: &str, volume: &ChannelVolumes) {
        let op = self
            .handler
            .introspect
            .set_source_volume_by_name(name, volume, None);
        self.handler.wait_for_operation(op).ok();
    }
    fn increase_device_volume_by_percent(&mut self, index: u32, delta: f64) {
        if let Ok(mut dev_ref) = self.get_device_by_index(index) {
            let new_vol = Volume::from(Volume(volume_from_percent(delta) as u32));
            if let Some(volumes) = dev_ref.volume.increase(new_vol) {
                let op = self
                    .handler
                    .introspect
                    .set_source_volume_by_index(index, &volumes, None);
                self.handler.wait_for_operation(op).ok();
            }
        }
    }
    fn decrease_device_volume_by_percent(&mut self, index: u32, delta: f64) {
        if let Ok(mut dev_ref) = self.get_device_by_index(index) {
            let new_vol = Volume::from(Volume(volume_from_percent(delta) as u32));
            if let Some(volumes) = dev_ref.volume.decrease(new_vol) {
                let op = self
                    .handler
                    .introspect
                    .set_source_volume_by_index(index, &volumes, None);
                self.handler.wait_for_operation(op).ok();
            }
        }
    }
}

impl AppControl<ApplicationInfo> for SourceController {
    fn list_applications(&mut self) -> Result<Vec<ApplicationInfo>, ControllerError> {
        let list = Rc::new(RefCell::new(Some(Vec::new())));
        let list_ref = list.clone();

        let op = self.handler.introspect.get_source_output_info_list(
            move |sink_list: ListResult<&introspect::SourceOutputInfo>| {
                if let ListResult::Item(item) = sink_list {
                    list_ref.borrow_mut().as_mut().unwrap().push(item.into());
                }
            },
        );
        self.handler.wait_for_operation(op)?;
        let mut result = list.borrow_mut();
        result.take().ok_or(ControllerError::new(
            GetInfoError,
            "Error getting application list",
        ))
    }

    fn get_app_by_index(&mut self, index: u32) -> Result<ApplicationInfo, ControllerError> {
        let app = Rc::new(RefCell::new(Some(None)));
        let app_ref = app.clone();
        let op = self.handler.introspect.get_source_output_info(
            index,
            move |sink_list: ListResult<&introspect::SourceOutputInfo>| {
                if let ListResult::Item(item) = sink_list {
                    app_ref.borrow_mut().as_mut().unwrap().replace(item.into());
                }
            },
        );
        self.handler.wait_for_operation(op)?;
        let mut result = app.borrow_mut();
        result.take().unwrap().ok_or(ControllerError::new(
            GetInfoError,
            "Error getting application list",
        ))
    }

    fn increase_app_volume_by_percent(&mut self, index: u32, delta: f64) {
        if let Ok(mut app_ref) = self.get_app_by_index(index) {
            let new_vol = Volume::from(Volume(volume_from_percent(delta) as u32));
            if let Some(volumes) = app_ref.volume.increase(new_vol) {
                let op = self
                    .handler
                    .introspect
                    .set_source_output_volume(index, &volumes, None);
                self.handler.wait_for_operation(op).ok();
            }
        }
    }

    fn decrease_app_volume_by_percent(&mut self, index: u32, delta: f64) {
        if let Ok(mut app_ref) = self.get_app_by_index(index) {
            let new_vol = Volume::from(Volume(volume_from_percent(delta) as u32));
            if let Some(volumes) = app_ref.volume.decrease(new_vol) {
                let op = self
                    .handler
                    .introspect
                    .set_source_output_volume(index, &volumes, None);
                self.handler.wait_for_operation(op).ok();
            }
        }
    }

    fn move_app_by_index(
        &mut self,
        stream_index: u32,
        device_index: u32,
    ) -> Result<bool, ControllerError> {
        let success = Rc::new(RefCell::new(false));
        let success_ref = success.clone();
        let op = self.handler.introspect.move_source_output_by_index(
            stream_index,
            device_index,
            Some(Box::new(move |res| {
                success_ref.borrow_mut().clone_from(&res)
            })),
        );
        self.handler.wait_for_operation(op)?;
        let result = success.borrow_mut().clone();
        Ok(result)
    }

    fn move_app_by_name(
        &mut self,
        stream_index: u32,
        device_name: &str,
    ) -> Result<bool, ControllerError> {
        let success = Rc::new(RefCell::new(false));
        let success_ref = success.clone();
        let op = self.handler.introspect.move_source_output_by_name(
            stream_index,
            device_name,
            Some(Box::new(move |res| {
                success_ref.borrow_mut().clone_from(&res)
            })),
        );
        self.handler.wait_for_operation(op)?;
        let result = success.borrow_mut().clone();
        Ok(result)
    }

    fn set_app_mute(&mut self, index: u32, mute: bool) -> Result<bool, ControllerError> {
        let success = Rc::new(RefCell::new(false));
        let success_ref = success.clone();
        let op = self.handler.introspect.set_source_mute_by_index(
            index,
            mute,
            Some(Box::new(move |res| {
                success_ref.borrow_mut().clone_from(&res)
            })),
        );
        self.handler.wait_for_operation(op)?;
        let result = success.borrow_mut().clone();
        Ok(result)
    }
}
