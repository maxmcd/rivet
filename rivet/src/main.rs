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

use std::thread;

fn main() {
    env_logger::init();
    gst::init().unwrap();
    thread::spawn(move || signalling::start_server());
    let main_loop = glib::MainLoop::new(None, false);
    rtsp::start_server();
    main_loop.run();
}
