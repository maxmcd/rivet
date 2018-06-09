use gst;
use gst::prelude::*;
use gst_rtsp_server;
use gst_rtsp_server::prelude::*;
use rand;
use rand::Rng;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

fn link_pads_to_tee(media: &gst_rtsp_server::RTSPMedia, main_pipeline: &gst::Pipeline) {
    println!("Hello!");
    let pipeline = media
        .get_element()
        .unwrap()
        .dynamic_cast::<gst::Bin>()
        .unwrap();

    let webrtc_pipeline = main_pipeline
        .get_by_name("pipeline/foo")
        .unwrap()
        .dynamic_cast::<gst::Bin>()
        .unwrap();
    let tee = main_pipeline.get_by_name("tee-video/foo").unwrap();
    println!("{:?}", tee);

    let queue = gst::ElementFactory::make("queue", None).unwrap();
    queue.set_property_from_str("leaky", "downstream");
    let vid_shmsink = gst::ElementFactory::make("shmsink", None).unwrap();
    let our_id = rand::thread_rng().gen_range(10, 10_000);
    vid_shmsink.set_property_from_str("socket-path", format!("/tmp/video{}", our_id).as_ref());
    webrtc_pipeline.add_many(&[&queue, &vid_shmsink]).unwrap();
    gst::Element::link_many(&[&tee, &queue, &vid_shmsink]).unwrap();
    vid_shmsink.sync_state_with_parent().unwrap();
    queue.sync_state_with_parent().unwrap();

    let replacement = gst::ElementFactory::make("shmsrc", "pay99").unwrap();
    replacement.set_property_from_str("socket-path", format!("/tmp/video{}", our_id).as_ref());
    let rtpbin = gst::ElementFactory::make("rtpbin", None).unwrap();
    pipeline.add_many(&[&replacement, &rtpbin]).unwrap();
    gst::Element::link_many(&[&replacement, &rtpbin]).unwrap();

    let rtph264pay = pipeline.get_by_name("pay0").unwrap();
    let rtp_src_pad = rtph264pay.get_static_pad("src").unwrap();
    let proxypad = rtp_src_pad.get_peer().unwrap();
    rtp_src_pad.unlink(&proxypad).unwrap();
    let replacement_pad = rtpbin.get_static_pad("src").unwrap();
    let ret = replacement_pad.link(&proxypad);
    assert_eq!(ret, gst::PadLinkReturn::Ok);
    // gst::debug_bin_to_dot_file_with_ts(
    //     &pipeline,
    //     gst::DebugGraphDetails::MEDIA_TYPE,
    //     "rtsp-server",
    // );
}

pub fn start_server(
    main_pipeline: &gst::Pipeline,
    _stream_map: &Arc<Mutex<HashMap<String, bool>>>,
) {
    let server = gst_rtsp_server::RTSPServer::new();
    server.set_address("0.0.0.0");
    let factory = gst_rtsp_server::RTSPMediaFactory::new();
    let mounts = server.get_mount_points().unwrap();
    // videotestsrc pattern=ball ! video/x-raw,width=352,height=288,framerate=30/1 !
    // x264enc ! rtph264pay name=pay0 pt=96
    // audiotestsrc wave=2 ! audio/x-raw,rate=8000 !
    // alawenc ! rtppcmapay name=pay1 pt=97

    // it's likely important that these pt attributes are accurate
    // we can hopefully just link this to a pt providing R TP frames
    // with little fuss
    factory.set_launch(
        "(
        rtph264pay name=pay0 pt=96
        audiotestsrc wave=2 ! audio/x-raw,rate=8000 !
        alawenc ! rtppcmapay name=pay1 pt=97
         )",
    );
    factory.set_shared(true);

    let main_pipeline_clone = main_pipeline.clone();
    factory.connect_media_configure(move |_, media| {
        link_pads_to_tee(&media, &main_pipeline_clone);
    });
    // factory.connect_media_constructed(|_, media| {
    //     println!("Hello!");
    //     println!("{:?}", media);
    // });

    mounts.add_factory("/test", &factory);
    server.attach(None);
    println!(
        "Stream ready at rtsp://127.0.0.1:{}/test",
        server.get_bound_port()
    );
}
