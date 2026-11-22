use super::pasteboard_context::{PasteObserverInfo, TEMP_FILE_PREFIX};
use objc2::{
    declare_class, msg_send_id, mutability,
    rc::Id,
    runtime::{NSObject, NSObjectProtocol},
    ClassType, DeclaredClass,
};
use objc2_app_kit::{
    NSPasteboard, NSPasteboardItem, NSPasteboardItemDataProvider, NSPasteboardType,
    NSPasteboardTypeFileURL,
};
use objc2_foundation::NSString;
use std::{io::Result, sync::mpsc::Sender};

pub(super) struct Ivars {
    task_info: PasteObserverInfo,
    tx: Sender<Result<PasteObserverInfo>>,
}

declare_class!(
    pub(super) struct PasteboardFileUrlProvider;

    unsafe impl ClassType for PasteboardFileUrlProvider {
        type Super = NSObject;
        type Mutability = mutability::InteriorMutable;
        const NAME: &'static str = "PasteboardFileUrlProvider";
    }

    impl DeclaredClass for PasteboardFileUrlProvider {
        type Ivars = Ivars;
    }

    unsafe impl NSObjectProtocol for PasteboardFileUrlProvider {}

    unsafe impl NSPasteboardItemDataProvider for PasteboardFileUrlProvider {
        #[method(pasteboard:item:provideDataForType:)]
        #[allow(non_snake_case)]
        unsafe fn pasteboard_item_provideDataForType(
            &self,
            _pasteboard: Option<&NSPasteboard>,
            item: &NSPasteboardItem,
            r#type: &NSPasteboardType,
        ) {
            if r#type == NSPasteboardTypeFileURL {
                let path = format!("/tmp/{}{}", TEMP_FILE_PREFIX, uuid::Uuid::new_v4().to_string());
                match std::fs::File::create(&path) {
                    Ok(_) => {
                        let url = format!("file:///{}", &path);
                            item.setString_forType(&NSString::from_str(&url), &NSPasteboardTypeFileURL);
                        let mut task_info = self.ivars().task_info.clone();
                        task_info.source_path = path;
                        self.ivars().tx.send(Ok(task_info)).ok();
                    }
                    Err(e) => {
                        self.ivars().tx.send(Err(e)).ok();
                    }
                }
            }
        }

        // #[method(pasteboardFinishedWithDataProvider:)]
        // unsafe fn pasteboardFinishedWithDataProvider(&self, pasteboard: &NSPasteboard) {
        // }
    }

    unsafe impl PasteboardFileUrlProvider {}
);

pub(super) fn create_pasteboard_file_url_provider(
    task_info: PasteObserverInfo,
    tx: Sender<Result<PasteObserverInfo>>,
) -> Id<PasteboardFileUrlProvider> {
    let provider = PasteboardFileUrlProvider::alloc();
    let provider = provider.set_ivars(Ivars { task_info, tx });
    let provider: Id<PasteboardFileUrlProvider> = unsafe { msg_send_id![super(provider), init] };
    provider
}
