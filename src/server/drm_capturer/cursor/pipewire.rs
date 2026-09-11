use super::{ffi, metadata, DrmCursorData};
use hbb_common::{anyhow::anyhow, bail, ResultType};
use std::{
    cell::UnsafeCell,
    ffi::{c_char, c_int, CStr},
    ptr, slice,
};

struct State {
    api: &'static ffi::Api,
    stream: ffi::Handle,
    publish: Box<dyn FnMut(DrmCursorData) + Send>,
    error: Option<String>,
    received: bool,
    cursor: metadata::CursorState,
}

pub struct Stream {
    api: &'static ffi::Api,
    thread: ffi::Handle,
    // PipeWire callbacks own this state while holding the thread-loop lock.
    state: Box<UnsafeCell<State>>,
    started: bool,
}

impl Stream {
    pub fn new(node: u32, publish: impl FnMut(DrmCursorData) + Send + 'static) -> ResultType<Self> {
        let api = ffi::Api::get()?;
        let thread = unsafe {
            (api.pw_thread_loop_new)(b"rustdesk-cursor\0".as_ptr().cast(), ptr::null_mut())
        };
        if thread.is_null() {
            bail!("Could not create the PipeWire cursor loop");
        }
        let mut stream = Self {
            api,
            thread,
            started: false,
            state: Box::new(UnsafeCell::new(State {
                api,
                stream: ptr::null_mut(),
                publish: Box::new(publish),
                error: None,
                received: false,
                cursor: metadata::CursorState::default(),
            })),
        };
        stream.connect(node)?;
        Ok(stream)
    }

    fn connect(&mut self, node: u32) -> ResultType<()> {
        // The loop has not started; callbacks during setup run synchronously on this thread.
        unsafe {
            let properties = (self.api.pw_properties_new_string)(
                b"media.type=Video media.category=Capture media.role=Screen\0"
                    .as_ptr()
                    .cast(),
            );
            if properties.is_null() {
                bail!("Could not create PipeWire cursor properties");
            }
            let stream = (self.api.pw_stream_new_simple)(
                (self.api.pw_thread_loop_get_loop)(self.thread),
                b"RustDesk cursor\0".as_ptr().cast(),
                properties,
                &EVENTS,
                self.state.get().cast(),
            );
            (*self.state.get()).stream = stream;
            if stream.is_null() {
                bail!("Could not create the PipeWire cursor stream");
            }
            let format = ffi::video_format();
            let params = [&format.pod as *const _];
            check((self.api.pw_stream_connect)(
                stream,
                ffi::DIRECTION_INPUT,
                node,
                ffi::AUTOCONNECT | ffi::DONT_RECONNECT,
                params.as_ptr(),
                params.len() as u32,
            ))?;
            check((self.api.pw_thread_loop_start)(self.thread))?;
            self.started = true;
        }
        Ok(())
    }

    pub fn received(&self) -> ResultType<bool> {
        unsafe {
            (self.api.pw_thread_loop_lock)(self.thread);
            let state = &*self.state.get();
            let result = match &state.error {
                Some(error) => Err(anyhow!("PipeWire cursor: {error}")),
                None => Ok(state.received),
            };
            (self.api.pw_thread_loop_unlock)(self.thread);
            result
        }
    }
}

impl Drop for Stream {
    fn drop(&mut self) {
        unsafe {
            if self.started {
                (self.api.pw_thread_loop_stop)(self.thread);
            }
            let stream = (*self.state.get()).stream;
            if !stream.is_null() {
                (self.api.pw_stream_destroy)(stream);
            }
            (self.api.pw_thread_loop_destroy)(self.thread);
        }
    }
}

fn check(result: c_int) -> ResultType<()> {
    if result < 0 {
        bail!("{}", std::io::Error::from_raw_os_error(-result));
    }
    Ok(())
}

unsafe extern "C" fn state_changed(
    data: ffi::Handle,
    old: c_int,
    state: c_int,
    error: *const c_char,
) {
    let context = &mut *data.cast::<State>();
    if state == ffi::STREAM_ERROR || (state == ffi::STREAM_UNCONNECTED && old != state) {
        context.error = Some(if error.is_null() {
            "Cursor stream disconnected".to_owned()
        } else {
            CStr::from_ptr(error).to_string_lossy().into_owned()
        });
    }
}

unsafe extern "C" fn param_changed(data: ffi::Handle, id: u32, param: *const ffi::Pod) {
    if id != ffi::PARAM_FORMAT || param.is_null() {
        return;
    }
    let context = data.cast::<State>();
    let meta = ffi::cursor_meta();
    let params = [&meta.pod as *const _];
    if let Err(error) = check(((*context).api.pw_stream_update_params)(
        (*context).stream,
        params.as_ptr(),
        params.len() as u32,
    )) {
        (*context).error = Some(error.to_string());
    }
}

unsafe fn cursor(
    decoder: &mut metadata::CursorState,
    buffer: *const ffi::Buffer,
) -> ResultType<Option<DrmCursorData>> {
    let buffer = buffer
        .as_ref()
        .ok_or_else(|| anyhow!("Missing PipeWire buffer"))?;
    if buffer.metas.is_null() {
        bail!("PipeWire did not negotiate cursor metadata");
    }
    let metas = slice::from_raw_parts(buffer.metas, buffer.n_metas as usize);
    let meta = metas
        .iter()
        .find(|meta| meta.kind == ffi::META_CURSOR)
        .ok_or_else(|| anyhow!("PipeWire did not negotiate cursor metadata"))?;
    if meta.data.is_null() {
        bail!("Missing PipeWire cursor metadata payload");
    }
    Ok(decoder.update(slice::from_raw_parts(meta.data.cast(), meta.size as usize))?)
}

unsafe extern "C" fn process(data: ffi::Handle) {
    let state = data.cast::<State>();
    let buffer = ((*state).api.pw_stream_dequeue_buffer)((*state).stream);
    if buffer.is_null() {
        return;
    }
    let result = cursor(&mut (*state).cursor, (*buffer).buffer);
    let queued = check(((*state).api.pw_stream_queue_buffer)(
        (*state).stream,
        buffer,
    ));
    let context = &mut *state;
    if context.error.is_some() {
        return;
    }
    match result.and_then(|cursor| queued.map(|_| cursor)) {
        Ok(cursor) => {
            context.received = true;
            if let Some(cursor) = cursor {
                (context.publish)(cursor);
            }
        }
        Err(error) => context.error = Some(error.to_string()),
    }
}

static EVENTS: ffi::Events = ffi::Events {
    version: 2,
    destroy: None,
    state_changed: Some(state_changed),
    control_info: None,
    io_changed: None,
    param_changed: Some(param_changed),
    add_buffer: None,
    remove_buffer: None,
    process: Some(process),
    drained: None,
    command: None,
    trigger_done: None,
};
