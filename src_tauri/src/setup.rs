


use crate::{
    core_main,
    ui,
};

pub fn setup(app: &tauri::AppHandle) {
    
    if let Some(args) = core_main::core_main().as_mut(){
        ui::start(app, args);
    }
}