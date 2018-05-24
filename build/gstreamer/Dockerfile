FROM alpine:edge

RUN apk add --no-cache \
    bison \
    build-base \
    flex \
    gettext-dev \
    # glib 2.56.1
    glib \
    libffi-dev \
    libmount \
    libnice-dev \
    linux-headers \
    pcre-dev \
    perl \
    python \
    x264-dev \
    zlib-dev

# http://www.linuxfromscratch.org/blfs/view/svn/multimedia/gstreamer10.html
RUN wget https://gstreamer.freedesktop.org/src/gstreamer/gstreamer-1.14.0.tar.xz \
    && tar xvfJ gstreamer-1.14.0.tar.xz > /dev/null \
    && cd gstreamer-1.14.0 \
    && ./configure --prefix=/usr \
    && make \
    && make install

RUN wget https://gstreamer.freedesktop.org/src/gst-plugins-base/gst-plugins-base-1.14.0.tar.xz \
    && tar xvfJ gst-plugins-base-1.14.0.tar.xz > /dev/null \
    && cd gst-plugins-base-1.14.0 \
    && ./configure --prefix=/usr \
    && make \
    && make install

RUN wget https://gstreamer.freedesktop.org/src/gst-plugins-good/gst-plugins-good-1.14.0.tar.xz \
    && tar xvfJ gst-plugins-good-1.14.0.tar.xz > /dev/null \
    && cd gst-plugins-good-1.14.0 \
    && ./configure --prefix=/usr \
    && make \
    && make install

RUN wget https://gstreamer.freedesktop.org/src/gst-plugins-bad/gst-plugins-bad-1.14.0.tar.xz \
    && tar xvfJ gst-plugins-bad-1.14.0.tar.xz > /dev/null \
    && cd gst-plugins-bad-1.14.0 \
    && ./configure --prefix=/usr \
    && make \
    && make install

RUN wget https://gstreamer.freedesktop.org/src/gst-plugins-ugly/gst-plugins-ugly-1.14.0.tar.xz \
    && tar xvfJ gst-plugins-ugly-1.14.0.tar.xz > /dev/null \
    && cd gst-plugins-ugly-1.14.0 \
    && ./configure --prefix=/usr \
    && make \
    && make install

RUN wget https://gstreamer.freedesktop.org/src/gst-rtsp-server/gst-rtsp-server-1.14.0.tar.xz \
    && tar xvfJ gst-rtsp-server-1.14.0.tar.xz > /dev/null \
    && cd gst-rtsp-server-1.14.0 \
    && ./configure --prefix=/usr \
    && make \
    && make install

RUN rm -rf gst*

RUN apk add tcpdump
COPY ./rtsp-launch.c .
RUN gcc rtsp-launch.c `pkg-config --cflags --libs gstreamer-1.0 gstreamer-rtsp-server-1.0` -o rtsp-launch

EXPOSE 1935

CMD ["./rtsp-launch"]
