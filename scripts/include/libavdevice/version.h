/*
 * version.h - FFmpeg device library version (stub for Moho)
 */

#ifndef AVDEVICE_VERSION_H
#define AVDEVICE_VERSION_H

#include "../libavutil/version.h"

#define LIBAVDEVICE_VERSION_MAJOR  61
#define LIBAVDEVICE_VERSION_MINOR   1
#define LIBAVDEVICE_VERSION_MICRO 100

#define LIBAVDEVICE_VERSION_INT AV_VERSION_INT(LIBAVDEVICE_VERSION_MAJOR, \
                                               LIBAVDEVICE_VERSION_MINOR, \
                                               LIBAVDEVICE_VERSION_MICRO)
#define LIBAVDEVICE_VERSION     AV_VERSION(LIBAVDEVICE_VERSION_MAJOR, \
                                           LIBAVDEVICE_VERSION_MINOR, \
                                           LIBAVDEVICE_VERSION_MICRO)

#endif /* AVDEVICE_VERSION_H */
