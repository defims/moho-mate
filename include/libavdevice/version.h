#ifndef AVDEVICE_VERSION_H
#define AVDEVICE_VERSION_H
#include "version_major.h"
#define LIBAVDEVICE_VERSION_MINOR   3
#define LIBAVDEVICE_VERSION_MICRO 104
#define LIBAVDEVICE_VERSION_INT AV_VERSION_INT(LIBAVDEVICE_VERSION_MAJOR, \
                                               LIBAVDEVICE_VERSION_MINOR, \
                                               LIBAVDEVICE_VERSION_MICRO)
#define LIBAVDEVICE_VERSION     AV_VERSION(LIBAVDEVICE_VERSION_MAJOR, \
                                           LIBAVDEVICE_VERSION_MINOR, \
                                           LIBAVDEVICE_VERSION_MICRO)
#define LIBAVDEVICE_BUILD       LIBAVDEVICE_VERSION_INT
#define LIBAVDEVICE_IDENT       "Lavd" AV_STRINGIFY(LIBAVDEVICE_VERSION)
#endif
