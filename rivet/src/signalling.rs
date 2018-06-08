use glib::ObjectExt;
use gst;
use gst::prelude::*;
use gst_sdp;
use gst_webrtc;
use serde_json;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use webrtc::set_up_webrtc;
use ws;
use ws::{CloseCode, Handler, Handshake, Message, Result};

struct WsServer {
    out: ws::Sender,
    webrtc: Option<gst::Element>,
    pipeline: Option<gst::Pipeline>,
    main_pipeline: gst::Pipeline,
    path: String,
    stream_map: Arc<Mutex<HashMap<String, bool>>>,
}

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

impl Handler for WsServer {
    fn on_open(&mut self, hs: Handshake) -> Result<()> {
        println!("{:?}", hs.request.resource());
        let mut stream_map = self.stream_map.lock().unwrap();
        let path = hs.request.resource();
        if stream_map.contains_key(path) {
            self.out
                .send("{\"msg\":\"there is already an active stream at this endpoint\"}")
                .unwrap();
            self.out.close(CloseCode::Error).unwrap();
            return Ok(());
        }
        stream_map.insert(path.to_string(), true);

        let (webrtc, pipeline) = match set_up_webrtc(&self.out, path.to_string()) {
            Ok(result) => result,
            Err(err) => {
                error!("Failed to set up webrtc {:?}, closing ws connection", err);
                self.out.close(CloseCode::Normal).unwrap();
                return Ok(());
            }
        };
        match self.main_pipeline.add(&pipeline) {
            Ok(()) => (),
            Err(err) => {
                println!("{:?}", err);
            }
        }
        self.pipeline = Some(pipeline);
        self.webrtc = Some(webrtc);
        self.path = path.to_string();
        info!("New connection from {}", hs.peer_addr.unwrap());
        Ok(())
    }
    fn on_message(&mut self, msg: Message) -> Result<()> {
        let json_msg: serde_json::Value = serde_json::from_str(&msg.as_text().unwrap()).unwrap();
        if json_msg.get("sdp").is_some() {
            ws_on_sdp(self.webrtc.as_ref().unwrap(), &json_msg);
        };
        if json_msg.get("ice").is_some() {
            debug!("adding ice {}", json_msg);
            ws_on_ice(self.webrtc.as_ref().unwrap(), &json_msg);
        }
        Ok(())
    }

    fn on_close(&mut self, code: CloseCode, reason: &str) {
        // remove url from stream_map
        let mut stream_map = self.stream_map.lock().unwrap();
        stream_map.remove(&self.path);
        match self.pipeline {
            Some(ref pipeline) => {
                let plc = pipeline.clone();
                self.main_pipeline
                    .remove(&plc.dynamic_cast::<gst::Element>().unwrap())
                    .unwrap();
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

pub fn start_server(main_pipeline: &gst::Pipeline, stream_map: &Arc<Mutex<HashMap<String, bool>>>) {
    let host = "0.0.0.0:8883";
    info!("Ws Listening at {}", host);
    let ws = ws::WebSocket::new(move |out| WsServer {
        out: out,
        webrtc: None,
        pipeline: None,
        path: String::new(),
        main_pipeline: main_pipeline.clone(),
        stream_map: stream_map.clone(),
    }).unwrap();
    // blocks
    ws.listen(host).unwrap();
}
