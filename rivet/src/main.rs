extern crate glib;
extern crate gstreamer as gst;
extern crate gstreamer_rtsp;
extern crate gstreamer_rtsp_server as gst_rtsp_server;
extern crate gstreamer_sdp;
extern crate gstreamer_sdp_sys;
extern crate gstreamer_webrtc;
#[macro_use]
extern crate serde_json;
extern crate ws;
use glib::translate::from_glib_full;
use glib::translate::ToGlibPtr;
use gst_rtsp_server::prelude::*;
use std::sync::{Arc, Mutex};
use std::{thread, time};
// use std::io;

use ws::{listen, CloseCode, Handler, Handshake, Message, Result};

const STUN_SERVER: &'static str = "stun-server=stun://stun.l.google.com:19302 ";

struct WsServer {
    out: ws::Sender,
    webrtc: gst::Element,
}

fn ws_on_sdp(webrtc: &gst::Element, json_msg: &serde_json::Value) {
    println!("{:?}", json_msg);
    if !json_msg.get("type").is_some() {
        println!("ERROR: received SDP without 'type'");
        return;
    }
    let sdptype = &json_msg["type"];
    assert_eq!(sdptype, "answer");
    let text = &json_msg["sdp"];
    print!("Received answer:\n{}\n", text.as_str().unwrap());

    let ret = gstreamer_sdp::SDPMessage::parse_buffer(text.as_str().unwrap().as_bytes()).unwrap();
    let offer = gstreamer_webrtc::WebRTCSessionDescription::new(
        gstreamer_webrtc::WebRTCSDPType::Offer,
        ret,
    );
    webrtc
        .emit("set-remote-description", &[&offer, &None::<gst::Promise>])
        .unwrap();
}

impl Handler for WsServer {
    fn on_open(&mut self, hs: Handshake) -> Result<()> {
        println!("New connection from {}", hs.peer_addr.unwrap());

        Ok(())
    }
    fn on_message(&mut self, msg: Message) -> Result<()> {
        // Echo the message back
        println!("Got message {:?}", msg);
        let json_msg: serde_json::Value = serde_json::from_str(&msg.as_text().unwrap()).unwrap();
        if json_msg.get("sdp").is_some() {
            ws_on_sdp(&self.webrtc, &json_msg);
        };
        Ok(())
    }

    fn on_close(&mut self, code: CloseCode, reason: &str) {
        match code {
            CloseCode::Normal => println!("The client is done with the connection."),
            CloseCode::Away => println!("The client is leaving the site."),
            _ => println!("The client encountered an error: {}", reason),
        }
    }
}

fn webrtc_create_offer(promise: &gst::Promise, webrtc: gst::Element, out: ws::Sender) {
    println!("create-offer callback");
    let reply = promise.get_reply().unwrap();
    let offer = reply
        .get_value("offer")
        .unwrap()
        .get::<gstreamer_webrtc::WebRTCSessionDescription>()
        .expect("Invalid argument");
    println!("offer {}", sdp_message_as_text(offer.clone()).unwrap());
    webrtc
        .emit("set-local-description", &[&offer, &None::<gst::Promise>])
        .unwrap();
    let message = json!({
        "type": "offer",
        "sdp": sdp_message_as_text(offer).unwrap(),
    });
    out.send(message.to_string()).unwrap();
}

fn webrtc_on_ice_candidate(values: &[glib::Value], webrtc: &gst::Element, out: &ws::Sender) {
    println!("on-ice-candidate {:?}", values);
}

fn webrtc_on_negotiation_needed(values: &[glib::Value], out: &ws::Sender) {
    println!("on-negotiation-needed {:?}", values);
    let webrtc = values[0].get::<gst::Element>().expect("Invalid argument");
    let webrtc_clone = webrtc.clone();
    let out_clone = out.clone();
    let promise = gst::Promise::new_with_change_func(move |promise: &gst::Promise| {
        webrtc_create_offer(promise, webrtc_clone, out_clone)
    });
    let options = gst::Structure::new_empty("options");
    webrtc.emit("create-offer", &[&options, &promise]).unwrap();
}

fn set_up_webrtc(out: &ws::Sender) -> gst::Element {
    let rtp_caps_vp8 = [
        "application/x-rtp",
        "media=video",
        "encoding-name=VP8",
        "payload=96",
        "clock-rate=90000",
    ].join(",");
    let rtp_caps_opus = [
        "application/x-rtp",
        "media=audio",
        "encoding-name=OPUS",
        "payload=97",
        "clock-rate=48000",
        "encoding-params=(string)2",
    ].join(",");
    let pipeline = gst::parse_launch(&format!(
        "webrtcbin name=webrtcsource {} 
         queue ! {} ! webrtcsource.
         queue ! {} ! webrtcsource.
        ",
        STUN_SERVER, rtp_caps_vp8, rtp_caps_opus
    )).unwrap();
    let webrtc = pipeline
        .clone()
        .dynamic_cast::<gst::Bin>()
        .unwrap()
        .get_by_name("webrtcsource")
        .unwrap();
    let out_clone = out.clone();
    webrtc
        .connect("on-negotiation-needed", false, move |values| {
            webrtc_on_negotiation_needed(values, &out_clone);
            None
        })
        .unwrap();
    let out_clone = out.clone();
    let webrtc_clone = webrtc.clone();
    webrtc
        .connect("on-ice-candidate", false, move |values| {
            webrtc_on_ice_candidate(values, &webrtc_clone, &out_clone);
            None
        })
        .unwrap();
    let pipeline_clone = pipeline.clone();
    webrtc
        .connect("pad-added", false, move |values| {
            on_incoming_stream(values, &pipeline_clone)
        })
        .unwrap();
    pipeline
        .set_state(gst::State::Playing)
        .into_result()
        .unwrap();
    webrtc.clone().dynamic_cast::<gst::Element>().unwrap()
}

fn on_incoming_stream(values: &[glib::Value], pipe: &gst::Element) -> Option<glib::Value> {
    println!("pad-added");
    let webrtc = values[0].get::<gst::Element>().expect("Invalid argument");
    let decodebin = gst::ElementFactory::make("decodebin", None).unwrap();
    decodebin
        .connect("pad-added", false, move |_values| {
            println!("decodebin pad added");
            None
        })
        .unwrap();
    pipe.clone()
        .dynamic_cast::<gst::Bin>()
        .unwrap()
        .add(&decodebin)
        .unwrap();
    decodebin.sync_state_with_parent().unwrap();
    webrtc.link(&decodebin).unwrap();
    None
}

fn sdp_message_as_text(offer: gstreamer_webrtc::WebRTCSessionDescription) -> Option<String> {
    unsafe {
        from_glib_full(gstreamer_sdp_sys::gst_sdp_message_as_text(
            (*offer.to_glib_none().0).sdp,
        ))
    }
}

pub fn start_server() {
    let host = "0.0.0.0:8883";
    println!("Ws Listening at {}", host);
    listen(host, |out| {
        let webrtc = set_up_webrtc(&out);
        WsServer {
            out: out,
            webrtc: webrtc,
        }
    }).unwrap();
}

fn main() {
    gst::init().unwrap();
    start_server();

    // TODO
    unreachable!();

    let main_loop = glib::MainLoop::new(None, false);
    let server = gst_rtsp_server::RTSPServer::new();
    server.set_address("127.0.0.1");
    let factory = gst_rtsp_server::RTSPMediaFactory::new();
    let mounts = server.get_mount_points().unwrap();
    factory.set_launch(
        "( 
        videotestsrc pattern=ball ! video/x-raw,width=352,height=288,framerate=30/1 ! 
        x264enc ! rtph264pay name=pay0 pt=96 
        audiotestsrc wave=2 ! audio/x-raw,rate=8000 ! 
        alawenc ! rtppcmapay name=pay1 pt=97  )",
    );
    factory.set_shared(true);

    // let elem = factory
    //     .create_element(&gstreamer_rtsp::RTSPUrl::parse("hi").1)
    //     .unwrap();
    // println!("{:?}", elem);
    factory
        .connect("media-configure", false, move |values| {
            let two_seconds = time::Duration::from_millis(2_000);
            thread::sleep(two_seconds);
            println!("{:?}", values);
            None
        })
        .unwrap();
    factory
        .connect("media-constructed", false, move |values| {
            println!("{:?}", values);
            None
        })
        .unwrap();
    factory.connect_media_configure(|_, media| {
        println!("Hello!");
        println!("{:?}", media);
    });
    factory.connect_media_constructed(|_, media| {
        println!("Hello!");
        println!("{:?}", media);
    });

    mounts.add_factory("/test", &factory);
    server.attach(None);
    println!(
        "Stream ready at rtsp://127.0.0.1:{}/test",
        server.get_bound_port()
    );
    main_loop.run();
    println!("hello");
}
