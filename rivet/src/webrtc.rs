use error::Error;
use glib;
use gst;
use gst::prelude::*;
use gst_app;
use gst_webrtc;
use std::str::FromStr;
use ws;

fn add_fakesink_to_tee(
    tee: &gst::Element,
    pipeline: &gst::Pipeline,
    name: &String,
    pad: &gst::Pad,
    media_type: &str,
) {
    let fakesink = gst::ElementFactory::make(
        "fakesink",
        format!("fakesink-{}{}", media_type, name).as_ref(),
    ).unwrap();
    let queue = gst::ElementFactory::make("queue", None).unwrap();
    pipeline.add_many(&[tee, &queue, &fakesink]).unwrap();
    gst::Element::link_many(&[tee, &queue, &fakesink]).unwrap();
    tee.sync_state_with_parent().unwrap();
    queue.sync_state_with_parent().unwrap();
    fakesink.sync_state_with_parent().unwrap();
    fakesink.set_property_from_str("dump", "false");
    let teepad = tee.get_static_pad("sink").unwrap();
    let ret = pad.link(&teepad);
    assert_eq!(ret, gst::PadLinkReturn::Ok);
}

fn add_appsink_to_tee(
    tee: &gst::Element,
    pipeline: &gst::Pipeline,
    _name: &String,
    pad: &gst::Pad,
    _media_type: &str,
) {
    let sink = gst::ElementFactory::make("appsink", None).unwrap();
    let appsink = sink.clone()
        .dynamic_cast::<gst_app::AppSink>()
        .expect("Sink element is expected to be an appsink!");
    pipeline.add_many(&[&appsink]).unwrap();
    tee.link(&appsink).unwrap();
    appsink.sync_state_with_parent().unwrap();
    appsink.set_caps(&pad.get_current_caps().unwrap());
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
}

fn just_appsink(
    tee: &gst::Element,
    pipeline: &gst::Pipeline,
    _name: &String,
    pad: &gst::Pad,
    _media_type: &str,
) {
    let sink = gst::ElementFactory::make("appsink", None).unwrap();
    let appsink = sink.clone()
        .dynamic_cast::<gst_app::AppSink>()
        .expect("Sink element is expected to be an appsink!");
    pipeline.add_many(&[&appsink]).unwrap();
    // tee.link(&appsink).unwrap();
    appsink.sync_state_with_parent().unwrap();
    appsink.set_caps(&pad.get_current_caps().unwrap());
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
}

fn on_incoming_rtpbin_stream(values: &[glib::Value], pipeline: &gst::Pipeline, name: &String) {
    let pad = values[1].get::<gst::Pad>().expect("Invalid argument");
    let pad_name = pad.get_name();
    println!("{:?}", pad_name);
    if pad_name.starts_with("recv_rtp_src") {
        println!("{:?}", pad.get_current_caps().unwrap());
        let media_type = if pad_name.ends_with("96") {
            "video"
        } else if pad_name.ends_with("97") {
            "audio"
        } else {
            unreachable!()
        };
        let tee = gst::ElementFactory::make("tee", format!("tee-{}{}", media_type, name).as_ref())
            .unwrap();

        just_appsink(&tee, pipeline, name, &pad, media_type);
        // add_fakesink_to_tee(&tee, pipeline, name, &pad, media_type);
        // add_appsink_to_tee(&tee, pipeline, name, &pad, media_type);
    }
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
    // TODO: we're duplicating a structure here that is internal to the webrtc
    // element. maybe we can just hook into the pads of the underlying webrtc
    // element instead of creating a new one
    let rtpbin = gst::ElementFactory::make("rtpbin", None).unwrap();
    webrtc.set_property_from_str("stun-server", "stun://stun.l.google.com:19302");
    pipeline.add_many(&[&webrtc, &rtpbin]).unwrap();

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
                ].join(","))?,
            ],
        )
        .unwrap();
    // TODO don't use stringly typed caps
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
                ].join(","))?,
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
    rtpbin.connect("pad-added", false, move |values| {
        on_incoming_rtpbin_stream(values, &pipeline_clone, &name);
        None
    })?;
    let rtpbin_clone = rtpbin.clone();
    let webrtc_clone = webrtc.clone();
    webrtc.connect("pad-added", false, move |_values| {
        // figure out why this is needed now
        // doesn't work if we link earlier
        webrtc_clone.link(&rtpbin_clone).unwrap();
        None
    })?;
    // webrtc.connect_pad_added(|_, pad| {
    //     println!("pad thing {:?}", pad.get_name());
    //     println!("caps {:?}", pad.has_current_caps());
    // });
    pipeline.set_state(gst::State::Playing).into_result()?;
    Ok((webrtc, pipeline))
}
