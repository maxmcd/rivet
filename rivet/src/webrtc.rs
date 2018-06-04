use glib;
use glib::translate::from_glib_full;
use glib::translate::ToGlibPtr;
use glib::Cast;
use gst;
use gst_rtsp_server::prelude::*;
use gst_sdp_sys;
use gst_webrtc;
use std::str::FromStr;
use ws;

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
        .get::<gst_webrtc::WebRTCSessionDescription>()
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

pub fn set_up_webrtc(out: &ws::Sender) -> gst::Element {
    let pipeline = gst::Pipeline::new("pipeline");
    let webrtc = gst::ElementFactory::make("webrtcbin", "webrtcsource").unwrap();
    webrtc.set_property_from_str("stun-server", "stun://stun.l.google.com:19302");
    pipeline.add_many(&[&webrtc]).unwrap();
    webrtc
        .emit(
            "add-transceiver",
            &[
                &gst_webrtc::WebRTCRTPTransceiverDirection::Recvonly,
                &gst::Caps::from_str(&[
                    "application/x-rtp",
                    "media=video",
                    "encoding-name=VP8",
                    "payload=96",
                    "clock-rate=90000",
                ].join(","))
                    .unwrap(),
            ],
        )
        .unwrap();
    webrtc
        .emit(
            "add-transceiver",
            &[
                &gst_webrtc::WebRTCRTPTransceiverDirection::Recvonly,
                &gst::Caps::from_str(&[
                    "application/x-rtp",
                    "media=audio",
                    "encoding-name=OPUS",
                    "payload=97",
                    "clock-rate=48000",
                    "encoding-params=(string)2",
                ].join(","))
                    .unwrap(),
            ],
        )
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
    webrtc.clone()
}

fn sdp_message_as_text(offer: gst_webrtc::WebRTCSessionDescription) -> Option<String> {
    unsafe {
        from_glib_full(gst_sdp_sys::gst_sdp_message_as_text(
            (*offer.to_glib_none().0).sdp,
        ))
    }
}
