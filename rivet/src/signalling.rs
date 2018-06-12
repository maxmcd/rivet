use common::{StreamMap, WsConn};
use glib;
use glib::ObjectExt;
use gst;
use gst::prelude::*;
use gst_sdp;
use gst_webrtc;
use serde_json;
use webrtc::set_up_webrtc;
use ws;

use ws::{CloseCode, Handler, Handshake, Message, Result};

fn ws_on_sdp(webrtc: &gst::Element, json_msg: &serde_json::Value) {
    if !json_msg.get("type").is_some() {
        debug!("ERROR: received SDP without 'type'");
        return;
    }
    let sdptype = &json_msg["type"];
    assert_eq!(sdptype, "answer");
    let text = &json_msg["sdp"];
    debug!("Received answer:\n{}\n", text.as_str().unwrap());

    let ret = gst_sdp::SDPMessage::parse_buffer(text.as_str().unwrap().as_bytes()).unwrap();
    let answer = gst_webrtc::WebRTCSessionDescription::new(gst_webrtc::WebRTCSDPType::Answer, ret);
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

impl Handler for WsConn {
    fn on_open(&mut self, hs: Handshake) -> Result<()> {
        println!("{:?}", hs.request.resource());
        let mut ws_conn = self.0.lock().unwrap();
        ws_conn.path = hs.request.resource().to_string();
        {
            let stream_map = ws_conn.stream_map.0.lock().unwrap();
            if stream_map.contains_key(&ws_conn.path) {
                ws_conn
                    .sender
                    .send("{\"msg\":\"there is already an active stream at this endpoint\"}")
                    .unwrap();
                ws_conn.sender.close(CloseCode::Error).unwrap();
                return Ok(());
            }
        }

        match set_up_webrtc(&mut ws_conn) {
            Ok(()) => (()),
            Err(err) => {
                error!("Failed to set up webrtc {:?}, closing ws connection", err);
                ws_conn.sender.close(CloseCode::Normal).unwrap();
                return Ok(());
            }
        };

        // let pipeline_clone = ws_conn.pipeline.as_ref().unwrap().clone();
        // let bus = pipeline_clone.get_bus().unwrap();
        // bus.add_watch(move |_, msg| {
        //     use gst::MessageView;

        //     match msg.view() {
        //         MessageView::StateChanged(_) => {}
        //         MessageView::StreamStatus(_) => {}
        //         _ => println!("New bus message {:?}\r", msg),
        //     };
        //     // https://sdroege.github.io/rustdoc/gstreamer/gstreamer/message/enum.MessageView.html
        //     glib::Continue(true)
        // });
        // glib::source::timeout_add_seconds(5, move || {
        //     gst::debug_bin_to_dot_file_with_ts(
        //         &pipeline_clone,
        //         gst::DebugGraphDetails::ALL,
        //         "main-pipeline",
        //     );
        //     glib::Continue(true)
        // });

        info!("New connection from {}", hs.peer_addr.unwrap());
        Ok(())
    }
    fn on_message(&mut self, msg: Message) -> Result<()> {
        let json_msg: serde_json::Value = serde_json::from_str(&msg.as_text().unwrap()).unwrap();
        let ws_conn = self.0.lock().unwrap();
        if json_msg.get("sdp").is_some() {
            ws_on_sdp(ws_conn.webrtc.as_ref().unwrap(), &json_msg);
        };
        if json_msg.get("ice").is_some() {
            debug!("adding ice {}", json_msg);
            ws_on_ice(ws_conn.webrtc.as_ref().unwrap(), &json_msg);
        }
        Ok(())
    }

    fn on_close(&mut self, code: CloseCode, reason: &str) {
        // remove url from stream_map
        let ws_conn = self.0.lock().unwrap();
        let mut stream_map = ws_conn.stream_map.0.lock().unwrap();
        stream_map.remove(&ws_conn.path);
        match ws_conn.pipeline {
            Some(ref _pipeline) => {
                // TODO tear down
            }
            None => (),
        }
        match code {
            CloseCode::Normal => debug!("The client is done with the connection."),
            CloseCode::Away => debug!("The client is leaving the site."),
            _ => debug!("The client encountered an error: {}", reason),
        }
    }
}

pub fn start_server(stream_map: &StreamMap) {
    let host = "0.0.0.0:8883";
    info!("Ws Listening at {}", host);
    let ws = ws::WebSocket::new(move |sender| WsConn::new(sender, stream_map)).unwrap();
    // blocks
    ws.listen(host).unwrap();
}
