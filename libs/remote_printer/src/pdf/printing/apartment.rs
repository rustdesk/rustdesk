use hbb_common::{
    libloading::os::windows::{Library, LOAD_LIBRARY_SEARCH_SYSTEM32},
    ResultType,
};
use once_cell::sync::OnceCell;
use std::{ffi::c_void, marker::PhantomData, path::Path, ptr::null_mut, rc::Rc};
use windows::{
    core::{Interface, RuntimeName, Type, GUID, HRESULT, HSTRING},
    Data::Pdf::{IPdfDocumentStatics, PdfDocument},
    Foundation::Metadata::{ApiInformation, IApiInformationStatics},
    Storage::{IStorageFile, IStorageFileStatics, StorageFile},
    Win32::System::WinRT::{RO_INIT_MULTITHREADED, RO_INIT_TYPE},
};
use windows_future::IAsyncOperation;

type GetFactory = unsafe extern "system" fn(*mut c_void, *const GUID, *mut *mut c_void) -> HRESULT;

struct Runtime {
    initialize: unsafe extern "system" fn(RO_INIT_TYPE) -> HRESULT,
    uninitialize: unsafe extern "system" fn(),
    get_factory: GetFactory,
    _library: Library,
}

impl Runtime {
    fn load() -> ResultType<Self> {
        type IncrementMtaUsage = unsafe extern "system" fn(*mut *mut c_void) -> HRESULT;
        unsafe {
            let library = Library::load_with_flags("combase.dll", LOAD_LIBRARY_SEARCH_SYSTEM32)?;
            let initialize = *library.get(b"RoInitialize\0")?;
            let uninitialize = *library.get(b"RoUninitialize\0")?;
            let get_factory = *library.get(b"RoGetActivationFactory\0")?;
            // Keep the MTA alive between PDF workers for the lifetime of the process.
            let ole32 = Library::load_with_flags("ole32.dll", LOAD_LIBRARY_SEARCH_SYSTEM32)?;
            let increment = ole32.get::<IncrementMtaUsage>(b"CoIncrementMTAUsage\0")?;
            let mut cookie = null_mut();
            increment(&mut cookie).ok()?;
            Ok(Self {
                initialize,
                uninitialize,
                get_factory,
                _library: library,
            })
        }
    }
}

pub(super) struct Apartment {
    runtime: &'static Runtime,
    // RoUninitialize must run on the thread that called RoInitialize.
    _thread: PhantomData<Rc<()>>,
}

impl Apartment {
    pub(super) fn initialize() -> ResultType<Self> {
        static RUNTIME: OnceCell<Runtime> = OnceCell::new();
        let runtime = RUNTIME.get_or_try_init(Runtime::load)?;
        unsafe { (runtime.initialize)(RO_INIT_MULTITHREADED).ok()? };
        Ok(Self {
            runtime,
            _thread: PhantomData,
        })
    }

    // The projected static methods import RoGetActivationFactory at process startup.
    fn factory<C: RuntimeName, I: Interface>(&self) -> ResultType<I> {
        let name = HSTRING::from(C::NAME);
        let mut factory = null_mut();
        unsafe {
            (self.runtime.get_factory)(hstring_handle(&name), &I::IID, &mut factory).ok()?;
            Ok(I::from_abi(factory)?)
        }
    }

    pub(super) fn pdf_type_present(&self) -> ResultType<bool> {
        let api = self.factory::<ApiInformation, IApiInformationStatics>()?;
        let name = HSTRING::from(PdfDocument::NAME);
        let mut present = false;
        unsafe {
            (api.vtable().IsTypePresent)(api.as_raw(), hstring_handle(&name), &mut present).ok()?;
        }
        Ok(present)
    }

    pub(super) fn load_pdf(&self, path: &Path) -> ResultType<PdfDocument> {
        let storage = self.factory::<StorageFile, IStorageFileStatics>()?;
        let path = HSTRING::from(path.as_os_str());
        let mut operation = null_mut();
        let file: IStorageFile = unsafe {
            (storage.vtable().GetFileFromPathAsync)(
                storage.as_raw(),
                hstring_handle(&path),
                &mut operation,
            )
            .ok()?;
            IAsyncOperation::<StorageFile>::from_abi(operation)?
        }
        .get()?
        .into();

        let pdf = self.factory::<PdfDocument, IPdfDocumentStatics>()?;
        let mut operation = null_mut();
        unsafe {
            (pdf.vtable().LoadFromFileAsync)(pdf.as_raw(), file.as_raw(), &mut operation).ok()?;
            Ok(IAsyncOperation::<PdfDocument>::from_abi(operation)?.get()?)
        }
    }
}

impl Drop for Apartment {
    fn drop(&mut self) {
        unsafe { (self.runtime.uninitialize)() };
    }
}

fn hstring_handle(value: &HSTRING) -> *mut c_void {
    // HSTRING is a transparent handle. The WinRT calls borrow it, including its header.
    unsafe { std::mem::transmute_copy(value) }
}
