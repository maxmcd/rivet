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

mod signalling;
mod webrtc;

fn main() {
    gst::init().unwrap();
    signalling::start_server();

    let main_loop = glib::MainLoop::new(None, false);
    main_loop.run();

    // let server = gst_rtsp_server::RTSPServer::new();
    // server.set_address("127.0.0.1");
    // let factory = gst_rtsp_server::RTSPMediaFactory::new();
    // let mounts = server.get_mount_points().unwrap();
    // factory.set_launch(
    //     "(
    //     videotestsrc pattern=ball ! video/x-raw,width=352,height=288,framerate=30/1 !
    //     x264enc ! rtph264pay name=pay0 pt=96
    //     audiotestsrc wave=2 ! audio/x-raw,rate=8000 !
    //     alawenc ! rtppcmapay name=pay1 pt=97  )",
    // );
    // factory.set_shared(true);

    // // let elem = factory
    // //     .create_element(&gst_rtsp::RTSPUrl::parse("hi").1)
    // //     .unwrap();
    // // println!("{:?}", elem);
    // factory
    //     .connect("media-configure", false, move |values| {
    //         let two_seconds = time::Duration::from_millis(2_000);
    //         thread::sleep(two_seconds);
    //         println!("{:?}", values);
    //         None
    //     })
    //     .unwrap();
    // factory
    //     .connect("media-constructed", false, move |values| {
    //         println!("{:?}", values);
    //         None
    //     })
    //     .unwrap();
    // factory.connect_media_configure(|_, media| {
    //     println!("Hello!");
    //     println!("{:?}", media);
    // });
    // factory.connect_media_constructed(|_, media| {
    //     println!("Hello!");
    //     println!("{:?}", media);
    // });

    // mounts.add_factory("/test", &factory);
    // server.attach(None);
    // println!(
    //     "Stream ready at rtsp://127.0.0.1:{}/test",
    //     server.get_bound_port()
    // );
}
