use std::{
    cell::Cell,
    rc::Rc,
    thread::{self, JoinHandle},
};

use winsafe::{co, gui, prelude::*};

pub struct AppCloseHandler {
    wnd: gui::WindowMain,
}

impl AppCloseHandler {
    pub fn new() -> Self {
        let wnd = gui::WindowMain::new(gui::WindowMainOpts {
            style: co::WS::OVERLAPPED, //required for processing wm_close and wm_endsession message
            ..Default::default()
        });
        Self { wnd }
    }

    pub fn on_app_close<F>(self, handler: F) -> JoinHandle<()>
    where
        F: FnOnce() + Send + 'static,
    {
        thread::spawn(move || {
            let handler = Rc::new(Cell::new(Some(handler)));
            let handler_1 = Rc::clone(&handler);
            self.wnd.on().wm_close(move || {
                if let Some(handler) = handler.take() {
                    handler();
                }
                Ok(())
            });
            self.wnd.on().wm_end_session(move |_| {
                if let Some(handler) = handler_1.take() {
                    handler();
                }
                Ok(())
            });
            self.wnd.run_main(Some(co::SW::HIDE)).unwrap();
        })
    }
}
