var wasmExports;

fetch('yuv.wasm').then(function (res) { return res.arrayBuffer(); })
  .then(function (file) { return WebAssembly.instantiate(file); })
  .then(function (wasm) {
    wasmExports = wasm.instance.exports;
    console.log('yuv ready');
  });

var yPtr, yPtrLen, uPtr, uPtrLen, vPtr, vPtrLen, outPtr, outPtrLen;
let testSpeed = [0, 0];
function I420ToARGB(yb) {
  if (!wasmExports) return;
  var tm0 = new Date().getTime();
  var { malloc, free, memory } = wasmExports;
  var HEAPU8 = new Uint8Array(memory.buffer);
  let n = yb.y.bytes.length;
  if (yPtrLen != n) {
    if (yPtr) free(yPtr);
    yPtrLen = n;
    yPtr = malloc(n);
  }
  HEAPU8.set(yb.y.bytes, yPtr);
  n = yb.u.bytes.length;
  if (uPtrLen != n) {
    if (uPtr) free(uPtr);
    uPtrLen = n;
    uPtr = malloc(n);
  }
  HEAPU8.set(yb.u.bytes, uPtr);
  n = yb.v.bytes.length;
  if (vPtrLen != n) {
    if (vPtr) free(vPtr);
    vPtrLen = n;
    vPtr = malloc(n);
  }
  HEAPU8.set(yb.v.bytes, vPtr);
  var w = yb.format.displayWidth;
  var h = yb.format.displayHeight;
  n = w * h * 4;
  if (outPtrLen != n) {
    if (outPtr) free(outPtr);
    outPtrLen = n;
    outPtr = malloc(n);
    HEAPU8.fill(255, outPtr, outPtr + n);
  }
  // var res = wasmExports.I420ToARGB(yPtr, yb.y.stride, uPtr, yb.u.stride, vPtr, yb.v.stride, outPtr, w * 4, w, h);
  // var res = wasmExports.AVX_YUV_to_ARGB(outPtr, yPtr, yb.y.stride, uPtr, yb.u.stride, vPtr, yb.v.stride, w, h);
  var res = wasmExports.yuv420_rgb24_std(w, h, yPtr, uPtr, vPtr, yb.y.stride, yb.v.stride, outPtr, w * 4, 1);
  var out = HEAPU8.slice(outPtr, outPtr + n);
  testSpeed[1] += new Date().getTime() - tm0;
  testSpeed[0] += 1;
  if (testSpeed[0] > 30) {
    console.log('yuv: ' + parseInt('' + testSpeed[1] / testSpeed[0]));
    testSpeed = [0, 0];
  }
  return out;
}

var currentFrame;
self.addEventListener('message', (e) => {
  currentFrame = e.data;
});

function run() {
  if (currentFrame) {
    self.postMessage(I420ToARGB(currentFrame));
    currentFrame = undefined;
  }
  setTimeout(run, 1);
}

run();