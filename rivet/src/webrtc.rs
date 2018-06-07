use error::Error;
use glib;
use gst;
use gst::prelude::*;
use gst_webrtc;
use std::str::FromStr;
use ws;

fn on_incoming_rtpbin_stream(values: &[glib::Value], pipeline: &gst::Pipeline) {
    let pad = values[1].get::<gst::Pad>().expect("Invalid argument");
    let pad_name = pad.get_name();
    if pad_name.starts_with("recv_rtp_src") {
        let fakesink = gst::ElementFactory::make("fakesink", None).unwrap();
        pipeline.add_many(&[&fakesink]).unwrap();
        fakesink.sync_state_with_parent().unwrap();
        fakesink.set_property_from_str("dump", "true");
        let fakepad = fakesink.get_static_pad("sink").unwrap();
        let ret = pad.link(&fakepad);
        assert_eq!(ret, gst::PadLinkReturn::Ok);
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

pub fn set_up_webrtc(out: &ws::Sender) -> Result<gst::Element, Error> {
    let pipeline = gst::Pipeline::new("pipeline");
    let webrtc = gst::ElementFactory::make("webrtcbin", "webrtcsource").unwrap();
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
        on_incoming_rtpbin_stream(values, &pipeline_clone);
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
    pipeline.set_state(gst::State::Playing).into_result()?;
    Ok(webrtc.clone())
}
