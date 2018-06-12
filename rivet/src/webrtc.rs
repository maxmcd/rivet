use common::{audio_caps, video_caps, VideoType, WsConnInner};
use error::Error;
use glib;
use gst;
use gst::prelude::*;
use gst_app;
use gst_webrtc;
use ws;

fn on_offer_created(promise: &gst::Promise, webrtc: gst::Element, sender: ws::Sender) {
    debug!("create-offer callback");
    let reply = promise.get_reply().unwrap();
    let offer = reply
        .get_value("offer")
        .unwrap()
        .get::<gst_webrtc::WebRTCSessionDescription>()
        .expect("Invalid argument");
    let mut sdp = offer.get_sdp();
    sdp.add_attribute("fmtp", "96 profile-level-id=42e00a;packetization-mode=1")
        .unwrap();
    let sdp_text = sdp.as_text().unwrap();

    debug!("offer {}", sdp_text);
    webrtc
        .emit("set-local-description", &[&offer, &None::<gst::Promise>])
        .unwrap();
    let message = json!({
        "type": "offer",
        "sdp": sdp_text,
    });
    sender.send(message.to_string()).unwrap();
}

fn send_ice_candidate_message(values: &[glib::Value], sender: &ws::Sender) {
    let mlineindex = values[1].get::<u32>().expect("Invalid argument");
    let candidate = values[2].get::<String>().expect("Invalid argument");
    let message = json!({
        "ice": {
            "candidate": candidate,
            "sdpMLineIndex": mlineindex,
        }
    });
    debug!("Sending {}", message.to_string());
    sender.send(message.to_string()).unwrap();
}

fn on_negotiation_needed(values: &[glib::Value], sender: &ws::Sender) {
    debug!("on-negotiation-needed {:?}", values);
    let webrtc = values[0].get::<gst::Element>().expect("Invalid argument");
    let clone = webrtc.clone();
    let sender_clone = sender.clone();
    let promise = gst::Promise::new_with_change_func(move |promise: &gst::Promise| {
        on_offer_created(promise, clone, sender_clone)
    });
    let options = gst::Structure::new_empty("options");
    webrtc.emit("create-offer", &[&options, &promise]).unwrap();
}

// TODO: only pass around WsConnInner
pub fn set_up_webrtc(ws_conn: &mut WsConnInner) -> Result<(), Error> {
    let pipeline = gst::Pipeline::new(format!("pipeline{}", ws_conn.path).as_ref());
    let webrtc = gst::ElementFactory::make("webrtcbin", "webrtcsource").unwrap();
    webrtc.set_property_from_str("stun-server", "stun://stun.l.google.com:19302");
    pipeline.add_many(&[&webrtc]).unwrap();
    webrtc
        .emit(
            "add-transceiver",
            &[
                &gst_webrtc::WebRTCRTPTransceiverDirection::Recvonly,
                &video_caps(VideoType::H264),
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
    let sender_clone = ws_conn.sender.clone();
    webrtc.connect("on-negotiation-needed", false, move |values| {
        on_negotiation_needed(values, &sender_clone);
        None
    })?;
    let sender_clone = ws_conn.sender.clone();
    webrtc.connect("on-ice-candidate", false, move |values| {
        send_ice_candidate_message(values, &sender_clone);
        None
    })?;
    let pipeline_clone = pipeline.clone();
    let av_bus = ws_conn.add_conn();
    webrtc.connect_pad_added(move |_, pad| {
        let pad_name = pad.get_name();
        let (caps, bus) = if pad_name == "src_0" {
            (video_caps(VideoType::H264), av_bus.video.clone())
        } else if pad_name == "src_1" {
            (audio_caps(), av_bus.audio.clone())
        } else {
            unreachable!()
        };
        let sink = gst::ElementFactory::make("appsink", None).unwrap();
        let queue = gst::ElementFactory::make("queue", None).unwrap();
        let appsink = sink.clone()
            .dynamic_cast::<gst_app::AppSink>()
            .expect("Sink element is expected to be an appsink!");
        pipeline_clone.add_many(&[&queue, &sink]).unwrap();
        gst::Element::link_many(&[&queue, &sink]).unwrap();
        // tee.link(&appsink).unwrap();
        queue.sync_state_with_parent().unwrap();
        appsink.sync_state_with_parent().unwrap();
        appsink.set_property_from_str("sync", "true");
        appsink.set_caps(&caps);
        appsink
            .set_state(gst::State::Playing)
            .into_result()
            .unwrap();
        appsink.set_callbacks(
            gst_app::AppSinkCallbacks::new()
                .new_sample(move |appsink| {
                    debug!("got buffer {}", pad_name);
                    let sample = match appsink.pull_sample() {
                        None => return gst::FlowReturn::Eos,
                        Some(sample) => sample,
                    };
                    bus.lock().unwrap().broadcast(sample);
                    gst::FlowReturn::Ok
                })
                .build(),
        );
        let appsink_pad = queue.get_static_pad("sink").unwrap();
        let ret = pad.link(&appsink_pad);
        assert_eq!(ret, gst::PadLinkReturn::Ok);
    });
    pipeline.set_state(gst::State::Playing).into_result()?;
    ws_conn.webrtc = Some(webrtc);
    ws_conn.pipeline = Some(pipeline);

    Ok(())
}
