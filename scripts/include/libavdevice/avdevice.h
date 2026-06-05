/*
 * avdevice.h - FFmpeg device library (stub for Moho)
 * Moho does not include libavdevice, this is a minimal stub
 */

#ifndef AVDEVICE_AVDEVICE_H
#define AVDEVICE_AVDEVICE_H

#include "version.h"

/**
 * Return the LIBAVDEVICE_VERSION_INT constant.
 */
unsigned avdevice_version(void);

/**
 * Return the libavdevice build-time configuration.
 */
const char *avdevice_configuration(void);

/**
 * Return the libavdevice license.
 */
const char *avdevice_license(void);

/**
 * Initialize libavdevice and register all the input and output devices.
 */
void avdevice_register_all(void);

#endif /* AVDEVICE_AVDEVICE_H */
