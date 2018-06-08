use glib::ObjectExt;
use gst;
use gst::prelude::*;
use gst_sdp;
use gst_webrtc;
use serde_json;
use webrtc::set_up_webrtc;
use ws;
use ws::{CloseCode, Handler, Handshake, Message, Result};
struct WsServer {
    out: ws::Sender,
    webrtc: Option<gst::Element>,
    main_pipeline: gst::Pipeline,
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
        let (webrtc, pipeline) = match set_up_webrtc(&self.out) {
            Ok(result) => result,
            Err(err) => {
                error!("Failed to set up webrtc {:?}, closing ws connection", err);
                self.out.close(CloseCode::Normal).unwrap();
                return Ok(());
            }
        };
        self.webrtc = Some(webrtc);
        match self.main_pipeline.add(&pipeline) {
            Ok(()) => (),
            Err(err) => {
                println!("{:?}", err);
            }
        }
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
        match code {
            CloseCode::Normal => debug!("The client is done with the connection."),
            CloseCode::Away => debug!("The client is leaving the site."),
            _ => debug!("The client encountered an error: {}", reason),
        }
    }
}

pub fn start_server(main_pipeline: &gst::Pipeline) {
    let host = "0.0.0.0:8883";
    info!("Ws Listening at {}", host);
    let ws = ws::WebSocket::new(|out| WsServer {
        out: out,
        webrtc: None,
        main_pipeline: main_pipeline.clone(),
    }).unwrap();
    // blocks
    ws.listen(host).unwrap();
}
