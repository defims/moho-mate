/* version.h - Generated for Moho FFmpeg 7.0 */
#ifndef AVUTIL_VERSION_H
#define AVUTIL_VERSION_H

#include "macros.h"

#define AV_VERSION_INT(a, b, c) ((a)<<16 | (b)<<8 | (c))
#define AV_VERSION_DOT(a, b, c) a ##.## b ##.## c
#define AV_VERSION(a, b, c) AV_VERSION_DOT(a, b, c)

#define LIBAVUTIL_VERSION_MAJOR  59
#define LIBAVUTIL_VERSION_MINOR  8
#define LIBAVUTIL_VERSION_MICRO 100

#define LIBAVUTIL_VERSION_INT  AV_VERSION_INT(LIBAVUTIL_VERSION_MAJOR, \
                                               LIBAVUTIL_VERSION_MINOR, \
                                               LIBAVUTIL_VERSION_MICRO)
#define LIBAVUTIL_VERSION      AV_VERSION(LIBAVUTIL_VERSION_MAJOR, \
                                          LIBAVUTIL_VERSION_MINOR, \
                                          LIBAVUTIL_VERSION_MICRO)
#define LIBAVUTIL_BUILD        LIBAVUTIL_VERSION_INT

#define LIBAVUTIL_IDENT        "Lavu" AV_STRINGIFY(LIBAVUTIL_VERSION)

#define FF_API_SSE42           (LIBAVUTIL_VERSION_MAJOR > 59)
#define FF_API_FRAME_PICTURE_NUMBER (LIBAVUTIL_VERSION_MAJOR > 59)

#endif /* AVUTIL_VERSION_H */
