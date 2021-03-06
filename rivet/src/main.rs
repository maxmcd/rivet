extern crate glib;
extern crate gobject_sys;
extern crate gstreamer as gst;
extern crate gstreamer_app as gst_app;
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
extern crate bus;
extern crate byte_slice_cast;
extern crate env_logger;
extern crate rand;

mod common;
mod error;
mod rtsp;
mod signalling;
mod webrtc;

use std::thread;

fn main() {
    env_logger::init();
    gst::init().unwrap();

    let stream_map = common::StreamMap::new();
    let stream_map_clone = stream_map.clone();
    thread::spawn(move || signalling::start_server(&stream_map_clone));
    let main_loop = glib::MainLoop::new(None, false);
    rtsp::start_server(&stream_map);
    main_loop.run();
}
