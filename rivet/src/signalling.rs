use glib::ObjectExt;
use gst;
use gst_sdp;
use gst_webrtc;
use serde_json;
use webrtc::set_up_webrtc;
use ws;
use ws::{listen, CloseCode, Handler, Handshake, Message, Result};

struct WsServer {
    _out: ws::Sender,
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
        println!("New connection from {}", hs.peer_addr.unwrap());

        Ok(())
    }
    fn on_message(&mut self, msg: Message) -> Result<()> {
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

pub fn start_server() {
    let host = "0.0.0.0:8883";
    println!("Ws Listening at {}", host);
    listen(host, |out| {
        let webrtc = set_up_webrtc(&out);
        WsServer {
            _out: out,
            webrtc: webrtc,
        }
    }).unwrap();
}
