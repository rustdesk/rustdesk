use crate::Error;
use std;

pub struct Window {}

impl Window {
    pub fn new() -> Result<Window, Error> {
        Err(Error::NotImplementedError)
    }
    pub fn quit(&self) {
        unimplemented!()
    }
    pub fn set_tooltip(&self, _: &str) -> Result<(), Error> {
        unimplemented!()
    }
    pub fn add_menu_item<F>(&self, _: &str, _: F) -> Result<u32, Error>
    where
        F: std::ops::Fn(&Window) -> () + 'static,
    {
        unimplemented!()
    }
    pub fn wait_for_message(&mut self) {
        unimplemented!()
    }
    pub fn set_icon_from_buffer(&self, _: &[u8], _: u32, _: u32) -> Result<(), Error> {
        unimplemented!()
    }
}
