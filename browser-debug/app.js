var pc, local_stream_promise;
var rtc_configuration = {
    iceServers: [
        { urls: "stun:stun.services.mozilla.com" },
        { urls: "stun:stun.l.google.com:19302" }
    ]
};


function onRemoteStreamAdded(event) {
    videoTracks = event.stream.getVideoTracks();
    audioTracks = event.stream.getAudioTracks();
}

ws_conn = new WebSocket("ws://localhost:8883");
ws_conn.addEventListener("open", event => {
    console.log("ws connection open");

    pc = new RTCPeerConnection(rtc_configuration);
    pc.onaddstream = onRemoteStreamAdded;

    local_stream_promise = navigator.mediaDevices
        .getUserMedia({ video: true, audio: true })
        .then(stream => {
            console.log("Adding local stream");
            pc.addStream(stream);
            return stream;
        })
        .catch(e => {
            console.log(`Error! ${e}`);
        });
    console.log(local_stream_promise);

    pc.onicecandidate = event => {
        console.log("ice candidate");
        if (event.candidate == null) {
            console.log("ICE Candidate was null, done");
            return;
        }
        ws_conn.send(JSON.stringify({ ice: event.candidate }));
    };
});
ws_conn.addEventListener("error", e => {
    console.log("error", e);
});
ws_conn.addEventListener("message", e => {
    let msg = JSON.parse(event.data);
    console.log("got message: ", msg);
    if (msg.sdp) {
        const sdp = msg;
        pc
            .setRemoteDescription(sdp)
            .then(() => {
                console.log("Remote SDP set");
                if (sdp.type != "offer") return;
                console.log("Got SDP offer");
                console.log(local_stream_promise);
                local_stream_promise
                    .then(stream => {
                        console.log("Got local stream, creating answer");
                        pc
                            .createAnswer()
                            .then(onLocalDescription)
                            .catch(e => {
                                console.log(`Error! ${e}`);
                            });
                    })
                    .catch(e => {
                        console.log(`Error! ${e}`);
                    });
            })
            .catch(e => {
                console.log(`Error! ${e}`);
            });
    }
    if (msg.ice) {
        var candidate = new RTCIceCandidate(msg.ice);
        pc.addIceCandidate(candidate).catch(e => {
            console.log(`Error! ${e}`);
        });
    }
});

// Local description was set, send it to peer
const onLocalDescription = desc => {
    console.log("Got local description: " + JSON.stringify(desc));
    pc.setLocalDescription(desc).then(function() {
        console.log("Sending SDP answer");
        ws_conn.send(JSON.stringify(pc.localDescription));
    });
};

ws_conn.addEventListener("close", e => {
    console.log("close", e);
});
