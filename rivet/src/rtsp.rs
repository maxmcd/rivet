use gst;
use gst::prelude::*;
use gst_rtsp_server;
use gst_rtsp_server::prelude::*;

pub fn start_server() {
    let server = gst_rtsp_server::RTSPServer::new();
    server.set_address("0.0.0.0");
    let factory = gst_rtsp_server::RTSPMediaFactory::new();
    let mounts = server.get_mount_points().unwrap();
    // videotestsrc pattern=ball ! video/x-raw,width=352,height=288,framerate=30/1 !
    // x264enc ! rtph264pay name=pay0 pt=96
    // audiotestsrc wave=2 ! audio/x-raw,rate=8000 !
    // alawenc ! rtppcmapay name=pay1 pt=97

    // it's likely important that these pt attributes are accurate
    // we can hopefully just link this to a pat providing RTP frames
    // with little fuss
    factory.set_launch(
        "(
        rtph264pay name=pay0 pt=97
        rtph264pay name=pay1 pt=96 
         )",
    );
    factory.set_shared(true);

    factory.connect_media_configure(|_, media| {
        println!("Hello!");
        let pipeline = media
            .get_element()
            .unwrap()
            .dynamic_cast::<gst::Bin>()
            .unwrap();
        println!("{:?}", pipeline.get_name());
        let rtph264pay = pipeline.get_by_name("pay0").unwrap();

        let replacement = gst::ElementFactory::make("rtph264pay", "pay99").unwrap();
        pipeline.add_many(&[&replacement]).unwrap();
        println!("{:?}", rtph264pay.get_name());
        //
        let rtp_src_pad = rtph264pay.get_static_pad("src").unwrap();
        let proxypad = rtp_src_pad.get_peer().unwrap();
        rtp_src_pad.unlink(&proxypad).unwrap();
        let ret = replacement.get_static_pad("src").unwrap().link(&proxypad);
        assert_eq!(ret, gst::PadLinkReturn::Ok);
        gst::debug_bin_to_dot_file_with_ts(
            &pipeline,
            gst::DebugGraphDetails::MEDIA_TYPE,
            "rtsp-server",
        );
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
