use bus;
use common::{audio_caps, video_caps, StreamMap, VideoType};
use gst;
use gst::prelude::*;
use gst_app;
use gst_rtsp;
use gst_rtsp_server;
use gst_rtsp_server::prelude::*;
use std::sync::{Arc, Mutex};

fn link_appsrc_to_pad(
    pipeline: &gst::Bin,
    name: &str,
    caps: gst::GstRc<gst::CapsRef>,
    rx: bus::BusReader<gst::Sample>,
) {
    let src = pipeline.get_by_name(name).unwrap();
    let appsrc = src.dynamic_cast::<gst_app::AppSrc>()
        .expect("Source element is expected to be an appsrc!");
    appsrc.set_caps(&caps);
    appsrc.set_property_format(gst::Format::Time);

    let rx_mutex = Arc::new(Mutex::new(rx));
    appsrc.set_callbacks(
        gst_app::AppSrcCallbacks::new()
            .need_data(move |appsrc, thing| {
                debug!("need data {}", thing);
                let rx_mutex = rx_mutex.clone();
                let sample = rx_mutex.lock().unwrap().recv().unwrap();
                // println!("got sample {:?}", sample.get_segment().unwrap());
                let _ = appsrc.push_sample(&sample);
            })
            .build(),
    );
    // appsrc.set_callbacks(
    //     gst_app::AppSrcCallbacks::new()
    //         .seek_data(move |_appsrc, _| {
    //             println!("seek data");
    //             true
    //         })
    //         .build(),
    // );
}

fn configure_media(media: &gst_rtsp_server::RTSPMedia, stream_map: StreamMap) {
    println!("Hello!");
    let pipeline = media
        .get_element()
        .unwrap()
        .dynamic_cast::<gst::Bin>()
        .unwrap();

    // media.connect_new_state(|_media, _id| {
    //     println!("connect_new_state {:?}", _id);
    // });
    // media.connect_new_stream(|_media, _stream| {
    //     println!("connect_new_stream {:?}", _stream);
    // });
    // media.connect_prepared(|_media| {
    //     println!("connect_prepared");
    // });
    // media.connect_removed_stream(|_media, _stream| {
    //     println!("connect_removed_stream {:?}", _stream);
    // });
    // media.connect_target_state(|_media, _num| {
    //     println!("connect_target_state {:?}", _num);
    //     if _num == 1 {}
    // });
    // media.connect_unprepared(|_media| {
    //     println!("connect_unprepared");
    // });

    let stream_map = stream_map.0.lock().unwrap();
    let av_bus = stream_map.get(&String::from("/foo")).unwrap();
    let video_rx = av_bus.video.lock().unwrap().add_rx();
    let audio_rx = av_bus.audio.lock().unwrap().add_rx();
    link_appsrc_to_pad(&pipeline, "pay0", video_caps(VideoType::H264), video_rx);
    link_appsrc_to_pad(&pipeline, "pay1", audio_caps(), audio_rx);
    println!("done");
}

pub fn start_server(stream_map: &StreamMap) {
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
    let stream_map = stream_map.clone();
    factory.connect_media_configure(move |_, media| {
        configure_media(&media, stream_map.clone());
    });
    factory.connect_media_constructed(|_, _media| {
        println!("factory media constructed");
    });
    factory.set_protocols(gst_rtsp::RTSPLowerTrans::TCP);
    mounts.add_factory("/test", &factory);
    server.attach(None);
    println!(
        "Stream ready at rtsp://127.0.0.1:{}/test",
        server.get_bound_port()
    );
}
