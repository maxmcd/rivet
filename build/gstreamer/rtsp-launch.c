/* GStreamer
 * Copyright (C) 2008 Wim Taymans <wim.taymans at gmail.com>
 *
 * This library is free software; you can redistribute it and/or
 * modify it under the terms of the GNU Library General Public
 * License as published by the Free Software Foundation; either
 * version 2 of the License, or (at your option) any later version.
 *
 * This library is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the GNU
 * Library General Public License for more details.
 *
 * You should have received a copy of the GNU Library General Public
 * License along with this library; if not, write to the
 * Free Software Foundation, Inc., 51 Franklin St, Fifth Floor,
 * Boston, MA 02110-1301, USA.
 */

#include <stdio.h>
#include <gst/gst.h>

#include <gst/rtsp-server/rtsp-server.h>

int
main (int argc, char *argv[])
{
  GMainLoop *loop;
  GstRTSPServer *server;
  GstRTSPMountPoints *mounts;
  GstRTSPMediaFactory *factory;
  GstRTSPAddressPool *pool;
  GstClock *rt_time;

  gst_init (&argc, &argv);

  loop = g_main_loop_new (NULL, FALSE);

  /* create a server instance */
  server = gst_rtsp_server_new ();

  /* get the mount points for this server, every server has a default object
   * that be used to map uri mount points to media factories */
  mounts = gst_rtsp_server_get_mount_points (server);

  /* make a media factory for a test stream. The default media factory can use
   * gst-launch syntax to create pipelines.
   * any launch line works as long as it contains elements named pay%d. Each
   * element with pay%d names will be a stream */
  factory = gst_rtsp_media_factory_new ();
  pool = gst_rtsp_address_pool_new ();
  gst_rtsp_address_pool_add_range (pool,
      GST_RTSP_ADDRESS_POOL_ANY_IPV4, 
      GST_RTSP_ADDRESS_POOL_ANY_IPV4, 
      32768+200, 32768+500, 0);
  gst_rtsp_media_factory_set_address_pool (factory, pool);

  // gst_rtsp_media_factory_set_buffer_size (factory, 1);
  // gst_rtsp_media_factory_set_latency (factory, 1000);

  // gst_rtsp_media_factory_set_protocols (factory, GST_RTSP_LOWER_TRANS_TCP);
  /* store up to 0.4 seconds of retransmission data */
  gst_rtsp_media_factory_set_retransmission_time (factory, 400 * GST_MSECOND);

  // gst_rtsp_media_factory_set_launch (factory, argv[1]);
  gst_rtsp_media_factory_set_launch (factory, "( "
        "videotestsrc pattern=ball ! video/x-raw,width=352,height=288,framerate=30/1 ! "
        "x264enc ! rtph264pay name=pay0 pt=96 "
        "audiotestsrc wave=2 ! audio/x-raw,rate=8000 ! "
        "alawenc ! rtppcmapay name=pay1 pt=97 " ")");
  
  /* attach the test factory to the /test url */
  gst_rtsp_mount_points_add_factory (mounts, "/test", factory);

  /* don't need the ref to the mapper anymore */
  g_object_unref (mounts);

  g_object_unref (pool);

  /* attach the server to the default maincontext */
  gst_rtsp_server_attach (server, NULL);

  /* start serving */
  g_print ("stream ready at rtsp://127.0.0.1:8554/test\n");
  g_main_loop_run (loop);

  return 0;
}
