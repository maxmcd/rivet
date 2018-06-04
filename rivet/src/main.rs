extern crate glib;
extern crate gobject_sys;
extern crate gstreamer as gst;
extern crate gstreamer_rtsp;
extern crate gstreamer_rtsp_server as gst_rtsp_server;
extern crate gstreamer_sdp;
extern crate gstreamer_sdp_sys;
extern crate gstreamer_sys;
extern crate gstreamer_webrtc;
extern crate gstreamer_webrtc_sys;
#[macro_use]
extern crate serde_json;
extern crate ws;
use glib::translate::from_glib_full;
use glib::translate::ToGlibPtr;
use gst_rtsp_server::prelude::*;
use std::str::FromStr;
// use std::io;

use ws::{listen, CloseCode, Handler, Handshake, Message, Result};

const STUN_SERVER: &'static str = "stun-server=stun://stun.l.google.com:19302 ";
const RTP_CAPS_OPUS: &'static str = "application/x-rtp,media=audio,encoding-name=OPUS,payload=";
const RTP_CAPS_VP8: &'static str = "application/x-rtp,media=video,encoding-name=VP8,payload=";

struct WsServer {
    out: ws::Sender,
    webrtc: gst::Element,
}

fn ws_on_sdp(webrtc: &gst::Element, json_msg: &serde_json::Value) {
    if !json_msg.get("type").is_some() {
        println!("ERROR: received SDP without 'type'");
        return;
    }
    let sdptype = &json_msg["type"];
    assert_eq!(sdptype, "answer");
    let text = &json_msg["sdp"];
    print!("Received answer:\n{}\n", text.as_str().unwrap());

    let ret = gstreamer_sdp::SDPMessage::parse_buffer(text.as_str().unwrap().as_bytes()).unwrap();
    let answer = gstreamer_webrtc::WebRTCSessionDescription::new(
        gstreamer_webrtc::WebRTCSDPType::Answer,
        ret,
    );
    webrtc
        .emit("set-remote-description", &[&answer, &None::<gst::Promise>])
        .unwrap();
}

fn ws_on_ice(webrtc: &gst::Element, json_msg: &serde_json::Value) {
    let candidate = json_msg["ice"]["candidate"].as_str().unwrap();
    let sdpmlineindex = json_msg["ice"]["sdpMLineIndex"].as_u64().unwrap() as u32;
    webrtc
        .emit("add-ice-candidate", &[&sdpmlineindex, &candidate])
        .unwrap();
}

impl Handler for WsServer {
    fn on_open(&mut self, hs: Handshake) -> Result<()> {
        println!("New connection from {}", hs.peer_addr.unwrap());

        Ok(())
    }
    fn on_message(&mut self, msg: Message) -> Result<()> {
        // Echo the message back
        let json_msg: serde_json::Value = serde_json::from_str(&msg.as_text().unwrap()).unwrap();
        if json_msg.get("sdp").is_some() {
            ws_on_sdp(&self.webrtc, &json_msg);
        };
        if json_msg.get("ice").is_some() {
            println!("adding ice {}", json_msg);
            ws_on_ice(&self.webrtc, &json_msg);
        }
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

fn webrtc_on_incoming_decodebin_stream(values: &[glib::Value], pipe: &gst::Element) {
    let pad = values[1].get::<gst::Pad>().expect("Invalid argument");
    if !pad.has_current_caps() {
        println!("Pad {:?} has no caps, can't do anything, ignoring", pad);
    }

    let caps = pad.get_current_caps().unwrap();
    let name = caps.get_structure(0).unwrap().get_name();
    println!("{:?}", name);
    println!("{:?}", caps);
    let pipe_bin = pipe.clone().dynamic_cast::<gst::Bin>().unwrap();
    let q = gst::ElementFactory::make("identity", None).unwrap();
    // q.set_property_from_str("leaky", "downstream");
    q.set_property_from_str("dump", "true");
    pipe_bin.add_many(&[&q]).unwrap();
    let qpad = q.get_static_pad("sink").unwrap();
    let ret = pad.link(&qpad);
    assert_eq!(ret, gst::PadLinkReturn::Ok);
    // if name.starts_with("video") {
    //     // handle_media_stream(&pad, &pipe, "rtpvp8depay", "rtpvp8pay");
    // } else if name.starts_with("audio") {
    //     // handle_media_stream(&pad, &pipe, "audioconvert", "autoaudiosink");
    // } else {
    //     println!("Unknown pad {:?}, ignoring", pad);
    // }
}

fn webrtc_on_incoming_stream(values: &[glib::Value], pipe: &gst::Element) {
    println!("pad-added");
    let webrtc = values[0].get::<gst::Element>().expect("Invalid argument");
    let decodebin = gst::ElementFactory::make("decodebin", None).unwrap();
    let pipe_clone = pipe.clone();
    decodebin
        .connect("pad-added", false, move |values| {
            webrtc_on_incoming_decodebin_stream(values, &pipe_clone);
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
}

fn webrtc_on_offer_created(promise: &gst::Promise, webrtc: gst::Element, out: ws::Sender) {
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

fn webrtc_send_ice_candidate_message(values: &[glib::Value], out: &ws::Sender) {
    let mlineindex = values[1].get::<u32>().expect("Invalid argument");
    let candidate = values[2].get::<String>().expect("Invalid argument");
    let message = json!({
        "ice": {
            "candidate": candidate,
            "sdpMLineIndex": mlineindex,
        }
    });
    println!("Sending {}", message.to_string());
    out.send(message.to_string()).unwrap();
}

fn webrtc_on_negotiation_needed(values: &[glib::Value], out: &ws::Sender) {
    println!("on-negotiation-needed {:?}", values);
    let webrtc = values[0].get::<gst::Element>().expect("Invalid argument");
    let webrtc_clone = webrtc.clone();
    let out_clone = out.clone();
    let promise = gst::Promise::new_with_change_func(move |promise: &gst::Promise| {
        webrtc_on_offer_created(promise, webrtc_clone, out_clone)
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

    let opus_caps = gst::Caps::from_str(&rtp_caps_opus).unwrap();
    let vp8_caps = gst::Caps::from_str(&rtp_caps_vp8).unwrap();
    // TODO: figure out how to intenfully add caps
    let _pipeline = gst::parse_launch(&format!(
        "webrtcbin name=webrtcsource {}
         identity ! {} ! webrtcsource.
         identity ! {} ! webrtcsource.
        ",
        STUN_SERVER, rtp_caps_vp8, rtp_caps_opus
    )).unwrap();
    let pipeline = gst::Pipeline::new("pipeline");
    let webrtc = gst::ElementFactory::make("webrtcbin", "webrtcsource").unwrap();
    pipeline.add_many(&[&webrtc]).unwrap();
    // emit_add_transceiver(&webrtc, &opus_caps);
    webrtc
        .emit(
            "add-transceiver",
            &[
                &gstreamer_webrtc::WebRTCRTPTransceiverDirection::Recvonly,
                &opus_caps,
            ],
        )
        .unwrap();
    webrtc
        .emit(
            "add-transceiver",
            &[
                &gstreamer_webrtc::WebRTCRTPTransceiverDirection::Recvonly,
                &vp8_caps,
            ],
        )
        .unwrap();
    // let pipeline = gst::parse_launch("webrtcbin name=webrtcsource").unwrap();

    // let pipeline = gst::parse_launch(&format!(
    //     "webrtcbin name=webrtcsource {}
    //     videotestsrc pattern=ball ! videoconvert ! queue ! vp8enc deadline=1 ! rtpvp8pay !
    //     queue ! {}96 ! webrtcsource.
    //     audiotestsrc wave=red-noise ! audioconvert ! audioresample ! queue ! opusenc ! rtpopuspay !
    //     queue ! {}97 ! webrtcsource.
    //     ",
    //     STUN_SERVER, RTP_CAPS_VP8, RTP_CAPS_OPUS
    // )).unwrap();
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
    webrtc
        .connect("on-ice-candidate", false, move |values| {
            webrtc_send_ice_candidate_message(values, &out_clone);
            None
        })
        .unwrap();
    let pipeline_clone = pipeline.clone().dynamic_cast::<gst::Element>().unwrap();
    webrtc
        .connect("pad-added", false, move |values| {
            webrtc_on_incoming_stream(values, &pipeline_clone);
            None
        })
        .unwrap();
    pipeline
        .set_state(gst::State::Playing)
        .into_result()
        .unwrap();
    webrtc.clone().dynamic_cast::<gst::Element>().unwrap()
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
    // //     .create_element(&gstreamer_rtsp::RTSPUrl::parse("hi").1)
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
