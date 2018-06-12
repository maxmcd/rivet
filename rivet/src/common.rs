use bus;
use glib;
use gst;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use ws;

#[derive(Clone)]
pub struct AVBus {
    pub audio: Arc<Mutex<bus::Bus<gst::Sample>>>,
    pub video: Arc<Mutex<bus::Bus<gst::Sample>>>,
}

#[derive(Clone)]
pub struct StreamMap(pub Arc<Mutex<HashMap<String, AVBus>>>);
impl StreamMap {
    pub fn new() -> Self {
        StreamMap(Arc::new(Mutex::new(HashMap::new())))
    }
    pub fn add_conn(&self, path: String) -> AVBus {
        let av_bus = AVBus {
            audio: Arc::new(Mutex::new(bus::Bus::new(10))),
            video: Arc::new(Mutex::new(bus::Bus::new(10))),
        };
        self.0.lock().unwrap().insert(path, av_bus.clone());
        av_bus
    }
}

#[derive(Clone)]
pub struct WsConn(pub Arc<Mutex<WsConnInner>>);
pub struct WsConnInner {
    pub sender: ws::Sender,
    pub webrtc: Option<gst::Element>,
    pub pipeline: Option<gst::Pipeline>,
    pub path: String,
    pub stream_map: StreamMap,
}

impl WsConn {
    pub fn new(sender: ws::Sender, stream_map: &StreamMap) -> Self {
        return WsConn(Arc::new(Mutex::new(WsConnInner {
            sender: sender,
            webrtc: None,
            pipeline: None,
            path: String::new(),
            stream_map: stream_map.clone(),
        })));
    }
}

impl WsConnInner {
    pub fn add_conn(&self) -> AVBus {
        self.stream_map.add_conn(self.path.clone())
    }
}

pub enum VideoType {
    VP9,
    H264,
}

pub fn video_caps(video_type: VideoType) -> gst::GstRc<gst::CapsRef> {
    let values: &[(&str, &glib::ToSendValue)] = match video_type {
        VideoType::VP9 => &[
            ("media", &"video"),
            ("encoding-name", &"VP8"),
            ("payload", &(96i32)),
            ("clock-rate", &(90_000i32)),
        ],
        VideoType::H264 => &[
            ("media", &"video"),
            ("encoding-name", &"H264"),
            ("payload", &(96i32)),
            ("clock-rate", &(90_000i32)),
        ],
    };
    gst::Caps::new_simple("application/x-rtp", values)
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
