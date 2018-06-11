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

use gst::prelude::*;
use std::thread;

fn main() {
    env_logger::init();
    gst::init().unwrap();

    let stream_map = common::StreamMap::new();
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
    glib::source::timeout_add_seconds(5, move || {
        gst::debug_bin_to_dot_file_with_ts(
            &main_pipeline_clone,
            gst::DebugGraphDetails::ALL,
            "main-pipeline",
        );
        glib::Continue(true)
    });
    let stream_map_clone = stream_map.clone();
    thread::spawn(move || signalling::start_server(&main_pipeline, &stream_map_clone));
    let main_loop = glib::MainLoop::new(None, false);
    rtsp::start_server(&stream_map);
    main_loop.run();
}
