#! /bin/sh

if [ -z "$WASI_LIBC" ]; then
  for path in /opt/wasi-libc /opt/wasi-sysroot; do
    if [ -d "$path" ]; then
      export WASI_LIBC="$path"
      break
    fi
  done
fi
if [ -z "$WASI_LIBC" ]; then
  echo "Set WASI_LIBC to the path to the WASI libc sysroot" >&2
  exit 1
fi

export PATH="/usr/local/opt/llvm/bin:$PATH"

export PREFIX="$(pwd)/libsodium-wasm32-wasi"

mkdir -p $PREFIX || exit 1

export CC="clang"
export CFLAGS="-DED25519_NONDETERMINISTIC=1 --target=wasm32-wasi --sysroot=${WASI_LIBC} -O2"
export LDFLAGS="-s -Wl,--stack-first"
export NM="llvm-nm"
export AR="llvm-ar"
export RANLIB="llvm-ranlib"
export STRIP="llvm-strip"

make distclean > /dev/null

grep -q -F -- 'wasi' build-aux/config.sub || \
  sed -i -e 's/-nacl\*)/-nacl*|-wasi)/' build-aux/config.sub

if [ "x$1" = "x--bench" ]; then
  export BENCHMARKS=1
  export CPPFLAGS="-DBENCHMARKS -DITERATIONS=100"
fi

if [ -n "$LIBSODIUM_MINIMAL_BUILD" ]; then
  export LIBSODIUM_ENABLE_MINIMAL_FLAG="--enable-minimal"
else
  export LIBSODIUM_ENABLE_MINIMAL_FLAG=""
fi

./configure ${LIBSODIUM_ENABLE_MINIMAL_FLAG} \
            --prefix="$PREFIX" --with-sysroot="$WASI_LIBC" \
            --host=wasm32-wasi \
            --disable-ssp --disable-shared || exit 1

NPROCESSORS=$(getconf NPROCESSORS_ONLN 2>/dev/null || getconf _NPROCESSORS_ONLN 2>/dev/null)
PROCESSORS=${NPROCESSORS:-3}

if [ -z "$BENCHMARKS" ]; then
  make -j${PROCESSORS} check && make install && make distclean > /dev/null
else
  make -j${PROCESSORS} && make check
fi
