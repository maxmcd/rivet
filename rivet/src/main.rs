extern crate glib;
extern crate gobject_sys;
extern crate gstreamer as gst;
extern crate gstreamer_rtsp as gst_rtsp;
extern crate gstreamer_rtsp_server as gst_rtsp_server;
extern crate gstreamer_sdp as gst_sdp;
extern crate gstreamer_sdp_sys as gst_sdp_sys;
extern crate gstreamer_sys as gst_sys;
extern crate gstreamer_webrtc as gst_webrtc;
extern crate gstreamer_webrtc_sys as gst_webrtc_sys;
#[macro_use]
extern crate serde_json;
extern crate ws;
#[macro_use]
extern crate log;
extern crate env_logger;

mod error;
mod rtsp;
mod signalling;
mod webrtc;

use gst::prelude::*;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::thread;

fn main() {
    env_logger::init();
    gst::init().unwrap();

    let stream_map: Arc<Mutex<HashMap<String, bool>>> = Arc::new(Mutex::new(HashMap::new()));
    let main_pipeline = gst::Pipeline::new("main");
    let bus = main_pipeline.get_bus().unwrap();
    bus.add_watch(move |_, msg| {
        use gst::MessageView;

        match msg.view() {
            MessageView::StateChanged(_) => {}
            MessageView::StreamStatus(_) => {}
            _ => println!("New bus message {:?}\r", msg),
        };
        // https://sdroege.github.io/rustdoc/gstreamer/gstreamer/message/enum.MessageView.html
        glib::Continue(true)
    });
    let main_pipeline_clone = main_pipeline.clone();
    let stream_map_clone = stream_map.clone();
    thread::spawn(move || signalling::start_server(&main_pipeline_clone, &stream_map_clone));
    let main_loop = glib::MainLoop::new(None, false);
    rtsp::start_server(&main_pipeline, &stream_map);
    main_loop.run();
}
