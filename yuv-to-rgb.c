/*
 * Copyright 2018 Google LLC. All rights reserved.
 *
 * Licensed under the Apache License, Version 2.0 (the "License"); you may not
 * use this file except in compliance with the License. You may obtain a copy of
 * the License at
 *
 *     http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS, WITHOUT
 * WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied. See the
 * License for the specific language governing permissions and limitations under
 * the License.
 */
/*
 *  Brute force conversion from YUV->RGB for testing only. Replace with WebGL
 *  shader conversion ASAP.
 *  NOTE: This code explicitly knows that the input is 16-bit 4:2:0 YCbCr
 *  and assumes there's space for 8-bit/channel RGB to write into.
 *  NO ERROR CHECKING!
 */
#include <stdlib.h>
#include <stdio.h>

#define ZOF_TAB     65536
#define ZOF_RGB     3

static int      T1[ZOF_TAB], T2[ZOF_TAB], T3[ZOF_TAB], T4[ZOF_TAB];
static int      initialized;

static void
build_tables() {
    int     i;

    for (i = 0;i < ZOF_TAB; i++) {
        T1[i] = (int)(1.370705 * (float)(i - 128));
        T2[i] = (int)(-0.698001 * (float)(i - 128));
        T3[i] = (int)(-0.337633 * (float)(i - 128));
        T4[i] = (int)(1.732446 * (float)(i - 128));
    }
}

#define clamp(val)  ((val) < 0 ? 0 : (255 < (val) ? 255 : (val)))
static int foo;
static int frame;

void
AVX_YUV_to_RGB(unsigned char *dst, unsigned short *src, int width, int height) {
    int             r, g, b;
    unsigned short  *y, *u, *v, *uline, *vline;
    int             w, h;

    if (initialized == 0) {
        initialized = !0;
        build_tables();
    }
    // Setup pointers to the Y, U, V planes
    y = src;
    u = src + (width * height);
    v = u + (width * height) / 4;   // Each chroma does 4 pixels in 4:2:0
    // Loop the image, taking into account sub-sample for the chroma channels
    for (h = 0; h < height; h++) {
        uline = u;
        vline = v;
        for (w = 0; w < width; w++, y++) {
            r = *y + T1[*vline];
            g = *y + T2[*vline] + T3[*uline];
            b = *y + T4[*uline];
            dst[0] = clamp(r);     // 16-bit to 8-bit, chuck precision
            dst[1] = clamp(g);
            dst[2] = clamp(b);
            dst += ZOF_RGB;
            if (w & 0x01) {
                uline++;
                vline++;
            }
        }
        if (h & 0x01) {
            u += width / 2;
            v += width / 2;
        }
    }
}
