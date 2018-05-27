FROM maxmcd/gstreamer:latest

COPY ./rtsp-launch.c .
ENV GST_DEBUG=2
RUN echo `pkg-config --cflags --libs gstreamer-1.0 gstreamer-rtsp-server-1.0`
RUN gcc rtsp-launch.c `pkg-config --cflags --libs gstreamer-1.0 gstreamer-rtsp-server-1.0` -o rtsp-launch

EXPOSE 1935

CMD ["./rtsp-launch"]
