extern crate gstreamer as gst;
extern crate gstreamer_rtsp_server as gst_rtsp_server;
extern crate glib;

use gst_rtsp_server::prelude::*;
// use std::io;

fn main() {
    gst::init().unwrap();

    let main_loop = glib::MainLoop::new(None, false);
    let server = gst_rtsp_server::RTSPServer::new();
    server.set_address("0.0.0.0");
    let factory = gst_rtsp_server::RTSPMediaFactory::new();
    let mounts = server.get_mount_points().unwrap();
    factory.set_launch("( 
        videotestsrc pattern=ball ! video/x-raw,width=352,height=288,framerate=30/1 ! 
        x264enc ! rtph264pay name=pay0 pt=96 
        audiotestsrc wave=2 ! audio/x-raw,rate=8000 ! 
        alawenc ! rtppcmapay name=pay1 pt=97  )"
    );
    factory.set_shared(true);
    mounts.add_factory("/test", &factory);
    server.attach(None);
    println!(
        "Stream ready at rtsp://127.0.0.1:{}/test",
        server.get_bound_port()
    );
    main_loop.run();
}
