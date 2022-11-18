use jni::objects::JByteBuffer;
use jni::objects::JString;
use jni::objects::JValue;
use jni::sys::jboolean;
use jni::JNIEnv;
use jni::{
    objects::{GlobalRef, JClass, JObject},
    JavaVM,
};

use jni::errors::{Error as JniError, Result as JniResult};
use lazy_static::lazy_static;
use std::ops::Not;
use std::sync::atomic::{AtomicPtr, Ordering::SeqCst};
use std::sync::{Mutex, RwLock};
use std::time::{Duration, Instant};
lazy_static! {
    static ref JVM: RwLock<Option<JavaVM>> = RwLock::new(None);
    static ref MAIN_SERVICE_CTX: RwLock<Option<GlobalRef>> = RwLock::new(None); // MainService -> video service / audio service / info
    static ref VIDEO_RAW: Mutex<FrameRaw> = Mutex::new(FrameRaw::new("video", MAX_VIDEO_FRAME_TIMEOUT));
    static ref AUDIO_RAW: Mutex<FrameRaw> = Mutex::new(FrameRaw::new("audio", MAX_AUDIO_FRAME_TIMEOUT));
}

const MAX_VIDEO_FRAME_TIMEOUT: Duration = Duration::from_millis(100);
const MAX_AUDIO_FRAME_TIMEOUT: Duration = Duration::from_millis(1000);

struct FrameRaw {
    name: &'static str,
    ptr: AtomicPtr<u8>,
    len: usize,
    last_update: Instant,
    timeout: Duration,
    enable: bool,
}

impl FrameRaw {
    fn new(name: &'static str, timeout: Duration) -> Self {
        FrameRaw {
            name,
            ptr: AtomicPtr::default(),
            len: 0,
            last_update: Instant::now(),
            timeout,
            enable: false,
        }
    }

    fn set_enable(&mut self, value: bool) {
        self.enable = value;
    }

    fn update(&mut self, data: &mut [u8]) {
        if self.enable.not() {
            return;
        }
        self.len = data.len();
        self.ptr.store(data.as_mut_ptr(), SeqCst);
        self.last_update = Instant::now();
    }

    // take inner data as slice
    // release when success
    fn take<'a>(&mut self) -> Option<&'a [u8]> {
        if self.enable.not() {
            return None;
        }
        let ptr = self.ptr.load(SeqCst);
        if ptr.is_null() || self.len == 0 {
            None
        } else {
            if self.last_update.elapsed() > self.timeout {
                log::trace!("Failed to take {} raw,timeout!", self.name);
                return None;
            }
            let slice = unsafe { std::slice::from_raw_parts(ptr, self.len) };
            self.release();
            Some(slice)
        }
    }

    fn release(&mut self) {
        self.len = 0;
        self.ptr.store(std::ptr::null_mut(), SeqCst);
    }
}

pub fn get_video_raw<'a>() -> Option<&'a [u8]> {
    VIDEO_RAW.lock().ok()?.take()
}

pub fn get_audio_raw<'a>() -> Option<&'a [u8]> {
    AUDIO_RAW.lock().ok()?.take()
}

#[no_mangle]
pub extern "system" fn Java_com_carriez_flutter_1hbb_MainService_onVideoFrameUpdate(
    env: JNIEnv,
    _class: JClass,
    buffer: JObject,
) {
    let jb = JByteBuffer::from(buffer);
    let slice = env.get_direct_buffer_address(jb).unwrap();
    VIDEO_RAW.lock().unwrap().update(slice);
}

#[no_mangle]
pub extern "system" fn Java_com_carriez_flutter_1hbb_MainService_onAudioFrameUpdate(
    env: JNIEnv,
    _class: JClass,
    buffer: JObject,
) {
    let jb = JByteBuffer::from(buffer);
    let slice = env.get_direct_buffer_address(jb).unwrap();
    AUDIO_RAW.lock().unwrap().update(slice);
}

#[no_mangle]
pub extern "system" fn Java_com_carriez_flutter_1hbb_MainService_setFrameRawEnable(
    env: JNIEnv,
    _class: JClass,
    name: JString,
    value: jboolean,
) {
    if let Ok(name) = env.get_string(name) {
        let name: String = name.into();
        let value = value.eq(&1);
        if name.eq("video") {
            VIDEO_RAW.lock().unwrap().set_enable(value);
        } else if name.eq("audio") {
            AUDIO_RAW.lock().unwrap().set_enable(value);
        }
    };
}

#[no_mangle]
pub extern "system" fn Java_com_carriez_flutter_1hbb_MainService_init(
    env: JNIEnv,
    _class: JClass,
    ctx: JObject,
) {
    log::debug!("MainService init from java");
    let jvm = env.get_java_vm().unwrap();

    *JVM.write().unwrap() = Some(jvm);

    let context = env.new_global_ref(ctx).unwrap();
    *MAIN_SERVICE_CTX.write().unwrap() = Some(context);
}

pub fn call_main_service_mouse_input(mask: i32, x: i32, y: i32) -> JniResult<()> {
    if let (Some(jvm), Some(ctx)) = (
        JVM.read().unwrap().as_ref(),
        MAIN_SERVICE_CTX.read().unwrap().as_ref(),
    ) {
        let env = jvm.attach_current_thread_as_daemon()?;
        env.call_method(
            ctx,
            "rustMouseInput",
            "(III)V",
            &[JValue::Int(mask), JValue::Int(x), JValue::Int(y)],
        )?;
        return Ok(());
    } else {
        return Err(JniError::ThrowFailed(-1));
    }
}

pub fn call_main_service_get_by_name(name: &str) -> JniResult<String> {
    if let (Some(jvm), Some(ctx)) = (
        JVM.read().unwrap().as_ref(),
        MAIN_SERVICE_CTX.read().unwrap().as_ref(),
    ) {
        let env = jvm.attach_current_thread_as_daemon()?;
        let name = env.new_string(name)?;
        let res = env
            .call_method(
                ctx,
                "rustGetByName",
                "(Ljava/lang/String;)Ljava/lang/String;",
                &[JValue::Object(name.into())],
            )?
            .l()?;
        let res = env.get_string(res.into())?;
        let res = res.to_string_lossy().to_string();
        return Ok(res);
    } else {
        return Err(JniError::ThrowFailed(-1));
    }
}

pub fn call_main_service_set_by_name(
    name: &str,
    arg1: Option<&str>,
    arg2: Option<&str>,
) -> JniResult<()> {
    if let (Some(jvm), Some(ctx)) = (
        JVM.read().unwrap().as_ref(),
        MAIN_SERVICE_CTX.read().unwrap().as_ref(),
    ) {
        let env = jvm.attach_current_thread_as_daemon()?;
        let name = env.new_string(name)?;
        let arg1 = env.new_string(arg1.unwrap_or(""))?;
        let arg2 = env.new_string(arg2.unwrap_or(""))?;

        env.call_method(
            ctx,
            "rustSetByName",
            "(Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;)V",
            &[
                JValue::Object(name.into()),
                JValue::Object(arg1.into()),
                JValue::Object(arg2.into()),
            ],
        )?;
        return Ok(());
    } else {
        return Err(JniError::ThrowFailed(-1));
    }
}
