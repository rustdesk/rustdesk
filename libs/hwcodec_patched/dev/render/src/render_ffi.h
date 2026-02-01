#ifndef RENDER_FFI_H
#define RENDER_FFI_H

#include <stdbool.h>

void *CreateDXGIRender(long long luid, bool inputSharedHandle);
int DXGIRenderTexture(void *render, void *tex);
void DestroyDXGIRender(void *render);
void *DXGIDevice(void *render);

#endif // RENDER_FFI_H