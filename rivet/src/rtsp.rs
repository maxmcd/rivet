use gst;
use gst::prelude::*;
use gst_app;
use gst_rtsp_server;
use gst_rtsp_server::prelude::*;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use webrtc;

// TODO: DRY
const WIDTH: usize = 320;
const HEIGHT: usize = 240;

fn link_appsrc_to_pad(pipeline: &gst::Bin, name: &str, caps: gst::GstRc<gst::CapsRef>) {
    let pay_element = pipeline.get_by_name(name).unwrap();
    let rtp_src_pad = pay_element.get_static_pad("src").unwrap();
    let proxypad = rtp_src_pad.get_peer().unwrap();
    rtp_src_pad.unlink(&proxypad).unwrap();
    let src = gst::ElementFactory::make("appsrc", None).unwrap();
    pipeline.add_many(&[&src]).unwrap();
    src.sync_state_with_parent().unwrap();
    let appsrc = src.dynamic_cast::<gst_app::AppSrc>()
        .expect("Source element is expected to be an appsrc!");
    appsrc.set_caps(&caps);
    let replacement_pad = appsrc.get_static_pad("src").unwrap();

    let mut i = 0;
    appsrc.set_callbacks(
        gst_app::AppSrcCallbacks::new()
            .need_data(move |appsrc, _| {
                if i == 100 {
                    let _ = appsrc.end_of_stream();
                    return;
                }

                println!("Producing frame {}", i);

                let r = if i % 2 == 0 { 0 } else { 255 };
                let g = if i % 3 == 0 { 0 } else { 255 };
                let b = if i % 5 == 0 { 0 } else { 255 };

                let mut buffer = gst::Buffer::with_size(WIDTH * HEIGHT * 4).unwrap();
                {
                    let buffer = buffer.get_mut().unwrap();
                    buffer.set_pts(i * 500 * gst::MSECOND);

                    let mut data = buffer.map_writable().unwrap();

                    for p in data.as_mut_slice().chunks_mut(4) {
                        assert_eq!(p.len(), 4);
                        p[0] = b;
                        p[1] = g;
                        p[2] = r;
                        p[3] = 0;
                    }
                }

                i += 1;

                // appsrc already handles the error here
                let _ = appsrc.push_buffer(buffer);
            })
            .build(),
    );

    let ret = replacement_pad.link(&proxypad);
    assert_eq!(ret, gst::PadLinkReturn::Ok);
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
    println!("done fuckin' this shit up");
}

pub fn start_server(
    _main_pipeline: &gst::Pipeline,
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
        appsrc name=pay0 pt=96
        appsrc name=pay1 pt=97
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
