use gst;
use gst::prelude::*;
use gst_app;
use gst_rtsp_server;
use gst_rtsp_server::prelude::*;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use webrtc;
use bus;

fn link_appsrc_to_pad(pipeline: &gst::Bin, name: &str, caps: gst::GstRc<gst::CapsRef>) {
    let src = pipeline.get_by_name(name).unwrap();
    let appsrc = src.dynamic_cast::<gst_app::AppSrc>()
        .expect("Source element is expected to be an appsrc!");
    appsrc.set_caps(&caps);
    appsrc.set_callbacks(
        gst_app::AppSrcCallbacks::new()
            .need_data(move |_appsrc, _| {
                println!("need frame");
            })
            .build(),
    );
}

fn configure_media(media: &gst_rtsp_server::RTSPMedia) {
    println!("Hello!");
    let pipeline = media
        .get_element()
        .unwrap()
        .dynamic_cast::<gst::Bin>()
        .unwrap();

    link_appsrc_to_pad(&pipeline, "pay0", webrtc::video_caps());
    link_appsrc_to_pad(&pipeline, "pay1", webrtc::audio_caps());
    println!("done");
}

pub fn start_server(_stream_map: &Arc<Mutex<HashMap<String, bus::Bus<gst::Buffer>>>>) {
    let server = gst_rtsp_server::RTSPServer::new();
    server.set_address("0.0.0.0");
    let factory = gst_rtsp_server::RTSPMediaFactory::new();
    let mounts = server.get_mount_points().unwrap();
    // videotestsrc pattern=ball ! video/x-raw,width=352,height=288,framerate=30/1 !
    // x264enc ! rtph264pay name=pay0 pt=96
    // audiotestsrc wave=2 ! audio/x-raw,rate=8000 !
    // alawenc ! rtppcmapay name=pay1 pt=97

    factory.set_launch(
        "(
        appsrc name=pay0 
        appsrc name=pay1 
         )",
    );
    factory.set_shared(true);

    factory.connect_media_configure(move |_, media| {
        configure_media(&media);
    });
    // factory.connect_media_constructed(|_, media| {
    // });

    mounts.add_factory("/test", &factory);
    server.attach(None);
    println!(
        "Stream ready at rtsp://127.0.0.1:{}/test",
        server.get_bound_port()
    );
}
