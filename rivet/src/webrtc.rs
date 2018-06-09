use error::Error;
use glib;
use gst;
use gst::prelude::*;
use gst_app;
use gst_webrtc;
use std::str::FromStr;
use ws;

pub fn video_caps() -> gst::GstRc<gst::CapsRef> {
    gst::Caps::new_simple(
        "application/x-rtp",
        &[
            ("media", &"video"),
            ("encoding-name", &"VP8"),
            ("payload", &(96i32)),
            ("clock-rate", &(90_000i32)),
        ],
    )
}
pub fn audio_caps() -> gst::GstRc<gst::CapsRef> {
    gst::Caps::new_simple(
        "application/x-rtp",
        &[
            ("media", &"audio"),
            ("encoding-name", &"OPUS"),
            ("payload", &(97i32)),
            ("clock-rate", &(48_000i32)),
            ("encoding-params", &"2"),
        ],
    )
}

fn on_offer_created(promise: &gst::Promise, webrtc: gst::Element, out: ws::Sender) {
    debug!("create-offer callback");
    let reply = promise.get_reply().unwrap();
    let offer = reply
        .get_value("offer")
        .unwrap()
        .get::<gst_webrtc::WebRTCSessionDescription>()
        .expect("Invalid argument");
    let sdp_text = offer.get_sdp().as_text().unwrap();
    debug!("offer {}", sdp_text);
    webrtc
        .emit("set-local-description", &[&offer, &None::<gst::Promise>])
        .unwrap();
    let message = json!({
        "type": "offer",
        "sdp": sdp_text,
    });
    out.send(message.to_string()).unwrap();
}

fn send_ice_candidate_message(values: &[glib::Value], out: &ws::Sender) {
    let mlineindex = values[1].get::<u32>().expect("Invalid argument");
    let candidate = values[2].get::<String>().expect("Invalid argument");
    let message = json!({
        "ice": {
            "candidate": candidate,
            "sdpMLineIndex": mlineindex,
        }
    });
    debug!("Sending {}", message.to_string());
    out.send(message.to_string()).unwrap();
}

fn on_negotiation_needed(values: &[glib::Value], out: &ws::Sender) {
    debug!("on-negotiation-needed {:?}", values);
    let webrtc = values[0].get::<gst::Element>().expect("Invalid argument");
    let clone = webrtc.clone();
    let out_clone = out.clone();
    let promise = gst::Promise::new_with_change_func(move |promise: &gst::Promise| {
        on_offer_created(promise, clone, out_clone)
    });
    let options = gst::Structure::new_empty("options");
    webrtc.emit("create-offer", &[&options, &promise]).unwrap();
}

pub fn set_up_webrtc(
    out: &ws::Sender,
    name: String,
) -> Result<(gst::Element, gst::Pipeline), Error> {
    let pipeline = gst::Pipeline::new(format!("pipeline{}", name).as_ref());
    let webrtc = gst::ElementFactory::make("webrtcbin", "webrtcsource").unwrap();
    webrtc.set_property_from_str("stun-server", "stun://stun.l.google.com:19302");
    pipeline.add_many(&[&webrtc]).unwrap();
    webrtc
        .emit(
            "add-transceiver",
            &[
                &gst_webrtc::WebRTCRTPTransceiverDirection::Recvonly,
                &video_caps(),
            ],
        )
        .unwrap();
    webrtc
        .emit(
            "add-transceiver",
            &[
                &gst_webrtc::WebRTCRTPTransceiverDirection::Recvonly,
                &audio_caps(),
            ],
        )
        .unwrap();
    let out_clone = out.clone();
    webrtc.connect("on-negotiation-needed", false, move |values| {
        on_negotiation_needed(values, &out_clone);
        None
    })?;
    let out_clone = out.clone();
    webrtc.connect("on-ice-candidate", false, move |values| {
        send_ice_candidate_message(values, &out_clone);
        None
    })?;
    let pipeline_clone = pipeline.clone();
    webrtc.connect_pad_added(move |_, pad| {
        let pad_name = pad.get_name();
        println!("pad thing {:?}", pad_name);
        let caps = if pad_name == "src_0" {
            video_caps()
        } else if pad_name == "src_1" {
            audio_caps()
        } else {
            unreachable!()
        };
        let sink = gst::ElementFactory::make("appsink", None).unwrap();
        let appsink = sink.clone()
            .dynamic_cast::<gst_app::AppSink>()
            .expect("Sink element is expected to be an appsink!");
        pipeline_clone.add_many(&[&appsink]).unwrap();
        // tee.link(&appsink).unwrap();
        appsink.sync_state_with_parent().unwrap();
        appsink.set_caps(&caps);
        appsink.set_callbacks(
            gst_app::AppSinkCallbacks::new()
                .new_sample(|appsink| {
                    println!("got buffer");
                    let sample = match appsink.pull_sample() {
                        None => return gst::FlowReturn::Eos,
                        Some(sample) => sample,
                    };

                    let _buffer = if let Some(buffer) = sample.get_buffer() {
                        buffer
                    } else {
                        println!("Failed to get buffer from appsink");

                        return gst::FlowReturn::Error;
                    };
                    println!("got buffer");
                    gst::FlowReturn::Ok
                })
                .build(),
        );
        let appsink_pad = appsink.get_static_pad("sink").unwrap();
        let ret = pad.link(&appsink_pad);
        assert_eq!(ret, gst::PadLinkReturn::Ok);
    });
    pipeline.set_state(gst::State::Playing).into_result()?;
    Ok((webrtc, pipeline))
}
