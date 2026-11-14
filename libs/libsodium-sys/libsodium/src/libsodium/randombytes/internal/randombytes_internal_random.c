
#include <assert.h>
#include <errno.h>
#include <fcntl.h>
#include <limits.h>
#include <stdint.h>
#include <stdlib.h>
#include <stdlib.h>
#include <string.h>
#if !defined(_MSC_VER) && !defined(__BORLANDC__)
# include <unistd.h>
#endif

#include <sys/types.h>
#ifndef _WIN32
# include <sys/stat.h>
# include <sys/time.h>
#endif
#ifdef __linux__
# define _LINUX_SOURCE
#endif
#ifdef HAVE_SYS_RANDOM_H
# include <sys/random.h>
#endif
#ifdef __linux__
# ifdef HAVE_GETRANDOM
#  define HAVE_LINUX_COMPATIBLE_GETRANDOM
# else
#  include <sys/syscall.h>
#  if defined(SYS_getrandom) && defined(__NR_getrandom)
#   define getrandom(B, S, F) syscall(SYS_getrandom, (B), (int) (S), (F))
#   define HAVE_LINUX_COMPATIBLE_GETRANDOM
#  endif
# endif
#elif defined(__FreeBSD__)
# include <sys/param.h>
# if defined(__FreeBSD_version) && __FreeBSD_version >= 1200000
#  define HAVE_LINUX_COMPATIBLE_GETRANDOM
# endif
#endif
#if !defined(NO_BLOCKING_RANDOM_POLL) && defined(__linux__)
# define BLOCK_ON_DEV_RANDOM
#endif
#ifdef BLOCK_ON_DEV_RANDOM
# include <poll.h>
#endif
#ifdef HAVE_RDRAND
# pragma GCC target("rdrnd")
# include <immintrin.h>
#endif

#include "core.h"
#include "crypto_core_hchacha20.h"
#include "crypto_stream_chacha20.h"
#include "private/common.h"
#include "randombytes.h"
#include "randombytes_internal_random.h"
#include "runtime.h"
#include "utils.h"

#ifdef _WIN32
# include <windows.h>
# include <sys/timeb.h>
# define RtlGenRandom SystemFunction036
# if defined(__cplusplus)
extern "C"
# endif
BOOLEAN NTAPI RtlGenRandom(PVOID RandomBuffer, ULONG RandomBufferLength);
# pragma comment(lib, "advapi32.lib")
# ifdef __BORLANDC__
#  define _ftime ftime
#  define _timeb timeb
# endif
#endif

#define INTERNAL_RANDOM_BLOCK_SIZE crypto_core_hchacha20_OUTPUTBYTES

#if defined(__OpenBSD__) || defined(__CloudABI__) || defined(__wasi__)
# define HAVE_SAFE_ARC4RANDOM 1
#endif
#if defined(__CloudABI__) || defined(__wasm__)
# define NONEXISTENT_DEV_RANDOM 1
#endif

#ifndef SSIZE_MAX
# define SSIZE_MAX (SIZE_MAX / 2 - 1)
#endif
#ifndef S_ISNAM
# ifdef __COMPCERT__
#  define S_ISNAM(X) 1
# else
#  define S_ISNAM(X) 0
# endif
#endif

#ifndef TLS
# ifdef _WIN32
#  define TLS __declspec(thread)
# else
#  define TLS
# endif
#endif

typedef struct InternalRandomGlobal_ {
    int           initialized;
    int           random_data_source_fd;
    int           getentropy_available;
    int           getrandom_available;
    int           rdrand_available;
#ifdef HAVE_GETPID
    pid_t         pid;
#endif
} InternalRandomGlobal;

typedef struct InternalRandom_ {
    int           initialized;
    size_t        rnd32_outleft;
    unsigned char key[crypto_stream_chacha20_KEYBYTES];
    unsigned char rnd32[16U * INTERNAL_RANDOM_BLOCK_SIZE];
    uint64_t      nonce;
} InternalRandom;

static InternalRandomGlobal global = {
    SODIUM_C99(.initialized =) 0,
    SODIUM_C99(.random_data_source_fd =) -1
};

static TLS InternalRandom stream = {
    SODIUM_C99(.initialized =) 0,
    SODIUM_C99(.rnd32_outleft =) (size_t) 0U
};


/*
 * Get a high-resolution timestamp, as a uint64_t value
 */

#ifdef _WIN32
static uint64_t
sodium_hrtime(void)
{
    struct _timeb tb;
# pragma warning(push)
# pragma warning(disable: 4996)
    _ftime(&tb);
# pragma warning(pop)
    return ((uint64_t) tb.time) * 1000000U + ((uint64_t) tb.millitm) * 1000U;
}

#else /* _WIN32 */

static uint64_t
sodium_hrtime(void)
{
    struct timeval tv;

    if (gettimeofday(&tv, NULL) != 0) {
        sodium_misuse(); /* LCOV_EXCL_LINE */
    }
    return ((uint64_t) tv.tv_sec) * 1000000U + (uint64_t) tv.tv_usec;
}
#endif /* _WIN32 */

/*
 * Initialize the entropy source
 */

#ifdef _WIN32

static void
randombytes_internal_random_init(void)
{
    global.rdrand_available = sodium_runtime_has_rdrand();
}

#else /* _WIN32 */

# ifdef HAVE_GETENTROPY
static int
_randombytes_getentropy(void * const buf, const size_t size)
{
    assert(size <= 256U);
    if (getentropy(buf, size) != 0) {
        return -1; /* LCOV_EXCL_LINE */
    }
    return 0;
}

static int
randombytes_getentropy(void * const buf_, size_t size)
{
    unsigned char *buf = (unsigned char *) buf_;
    size_t         chunk_size = 256U;

    do {
        if (size < chunk_size) {
            chunk_size = size;
            assert(chunk_size > (size_t) 0U);
        }
        if (_randombytes_getentropy(buf, chunk_size) != 0) {
            return -1; /* LCOV_EXCL_LINE */
        }
        size -= chunk_size;
        buf += chunk_size;
    } while (size > (size_t) 0U);

    return 0;
}

# elif defined(HAVE_LINUX_COMPATIBLE_GETRANDOM)

static int
_randombytes_linux_getrandom(void * const buf, const size_t size)
{
    int readnb;

    assert(size <= 256U);
    do {
        readnb = getrandom(buf, size, 0);
    } while (readnb < 0 && (errno == EINTR || errno == EAGAIN));

    return (readnb == (int) size) - 1;
}

static int
randombytes_linux_getrandom(void * const buf_, size_t size)
{
    unsigned char *buf = (unsigned char *) buf_;
    size_t         chunk_size = 256U;

    do {
        if (size < chunk_size) {
            chunk_size = size;
            assert(chunk_size > (size_t) 0U);
        }
        if (_randombytes_linux_getrandom(buf, chunk_size) != 0) {
            return -1;
        }
        size -= chunk_size;
        buf += chunk_size;
    } while (size > (size_t) 0U);

    return 0;
}
# endif

# ifndef NONEXISTENT_DEV_RANDOM

#  ifdef BLOCK_ON_DEV_RANDOM
static int
randombytes_block_on_dev_random(void)
{
    struct pollfd pfd;
    int           fd;
    int           pret;

    fd = open("/dev/random", O_RDONLY);
    if (fd == -1) {
        return 0;
    }
    pfd.fd = fd;
    pfd.events = POLLIN;
    pfd.revents = 0;
    do {
        pret = poll(&pfd, 1, -1);
    } while (pret < 0 && (errno == EINTR || errno == EAGAIN));
    if (pret != 1) {
        (void) close(fd);
        errno = EIO;
        return -1;
    }
    return close(fd);
}
#  endif

/* LCOV_EXCL_START */
static int
randombytes_internal_random_random_dev_open(void)
{
    struct stat       st;
    static const char *devices[] = {
#  ifndef USE_BLOCKING_RANDOM
        "/dev/urandom",
#  endif
        "/dev/random", NULL
    };
    const char      **device = devices;
    int               fd;

#  ifdef BLOCK_ON_DEV_RANDOM
    if (randombytes_block_on_dev_random() != 0) {
        return -1;
    }
#  endif
    do {
        fd = open(*device, O_RDONLY);
        if (fd != -1) {
            if (fstat(fd, &st) == 0 && (S_ISNAM(st.st_mode) || S_ISCHR(st.st_mode))) {
#  if defined(F_SETFD) && defined(FD_CLOEXEC)
                (void) fcntl(fd, F_SETFD, fcntl(fd, F_GETFD) | FD_CLOEXEC);
#  endif
                return fd;
            }
            (void) close(fd);
        } else if (errno == EINTR) {
            continue;
        }
        device++;
    } while (*device != NULL);

    errno = EIO;
    return -1;
}
/* LCOV_EXCL_STOP */

static ssize_t
safe_read(const int fd, void * const buf_, size_t size)
{
    unsigned char *buf = (unsigned char *) buf_;
    ssize_t        readnb;

    assert(size > (size_t) 0U);
    assert(size <= SSIZE_MAX);
    do {
        while ((readnb = read(fd, buf, size)) < (ssize_t) 0 &&
               (errno == EINTR || errno == EAGAIN)); /* LCOV_EXCL_LINE */
        if (readnb < (ssize_t) 0) {
            return readnb; /* LCOV_EXCL_LINE */
        }
        if (readnb == (ssize_t) 0) {
            break; /* LCOV_EXCL_LINE */
        }
        size -= (size_t) readnb;
        buf += readnb;
    } while (size > (ssize_t) 0);

    return (ssize_t) (buf - (unsigned char *) buf_);
}

# endif /* !NONEXISTENT_DEV_RANDOM */

static void
randombytes_internal_random_init(void)
{
    const int errno_save = errno;

    global.rdrand_available = sodium_runtime_has_rdrand();
    global.getentropy_available = 0;
    global.getrandom_available = 0;

# ifdef HAVE_GETENTROPY
    {
        unsigned char fodder[16];

        if (randombytes_getentropy(fodder, sizeof fodder) == 0) {
            global.getentropy_available = 1;
            errno = errno_save;
            return;
        }
    }
# elif defined(HAVE_LINUX_COMPATIBLE_GETRANDOM)
    {
        unsigned char fodder[16];

        if (randombytes_linux_getrandom(fodder, sizeof fodder) == 0) {
            global.getrandom_available = 1;
            errno = errno_save;
            return;
        }
    }
# endif
/* LCOV_EXCL_START */
# if !defined(NONEXISTENT_DEV_RANDOM)
    assert((global.getentropy_available | global.getrandom_available) == 0);
    if ((global.random_data_source_fd =
         randombytes_internal_random_random_dev_open()) == -1) {
        sodium_misuse(); /* LCOV_EXCL_LINE */
    }
    errno = errno_save;
    return;
# endif
/* LCOV_EXCL_STOP */
# ifndef HAVE_SAFE_ARC4RANDOM
    sodium_misuse();
# endif
}

#endif /* _WIN32 */

/*
 * (Re)seed the generator using the entropy source
 */

static void
randombytes_internal_random_stir(void)
{
    stream.nonce = sodium_hrtime();
    assert(stream.nonce != (uint64_t) 0U);
    memset(stream.rnd32, 0, sizeof stream.rnd32);
    stream.rnd32_outleft = (size_t) 0U;
    if (global.initialized == 0) {
        randombytes_internal_random_init();
        global.initialized = 1;
    }
#ifdef HAVE_GETPID
    global.pid = getpid();
#endif

#ifndef _WIN32

# ifdef HAVE_GETENTROPY
     if (global.getentropy_available != 0) {
         if (randombytes_getentropy(stream.key, sizeof stream.key) != 0) {
             sodium_misuse(); /* LCOV_EXCL_LINE */
         }
     }
# elif defined(HAVE_LINUX_COMPATIBLE_GETRANDOM)
     if (global.getrandom_available != 0) {
         if (randombytes_linux_getrandom(stream.key, sizeof stream.key) != 0) {
             sodium_misuse(); /* LCOV_EXCL_LINE */
         }
     }
# elif defined(NONEXISTENT_DEV_RANDOM) && defined(HAVE_SAFE_ARC4RANDOM)
    arc4random_buf(stream.key, sizeof stream.key);
# elif !defined(NONEXISTENT_DEV_RANDOM)
    if (global.random_data_source_fd == -1 ||
        safe_read(global.random_data_source_fd, stream.key,
                  sizeof stream.key) != (ssize_t) sizeof stream.key) {
        sodium_misuse(); /* LCOV_EXCL_LINE */
    }
# else
    sodium_misuse();
# endif

#else /* _WIN32 */
    if (! RtlGenRandom((PVOID) stream.key, (ULONG) sizeof stream.key)) {
        sodium_misuse(); /* LCOV_EXCL_LINE */
    }
#endif

    stream.initialized = 1;
}

/*
 * Reseed the generator if it hasn't been initialized yet
 */

static void
randombytes_internal_random_stir_if_needed(void)
{
#ifdef HAVE_GETPID
    if (stream.initialized == 0) {
        randombytes_internal_random_stir();
    } else if (global.pid != getpid()) {
        sodium_misuse(); /* LCOV_EXCL_LINE */
    }
#else
    if (stream.initialized == 0) {
        randombytes_internal_random_stir();
    }
#endif
}

/*
 * Close the stream, free global resources
 */

#ifdef _WIN32
static int
randombytes_internal_random_close(void)
{
    int ret = -1;

    if (global.initialized != 0) {
        global.initialized = 0;
        ret = 0;
    }
    sodium_memzero(&stream, sizeof stream);

    return ret;
}
#else
static int
randombytes_internal_random_close(void)
{
    int ret = -1;

# ifdef HAVE_GETENTROPY
    if (global.getentropy_available != 0) {
        ret = 0;
    }
# elif defined(HAVE_LINUX_COMPATIBLE_GETRANDOM)
    if (global.getrandom_available != 0) {
        ret = 0;
    }
# elif !defined(NONEXISTENT_DEV_RANDOM) && defined(HAVE_SAFE_ARC4RANDOM)
    ret = 0;
# else
    if (global.random_data_source_fd != -1 &&
        close(global.random_data_source_fd) == 0) {
        global.random_data_source_fd = -1;
        global.initialized = 0;
# ifdef HAVE_GETPID
        global.pid = (pid_t) 0;
# endif
        ret = 0;
    }
# endif

    sodium_memzero(&stream, sizeof stream);

    return ret;
}
#endif

/*
 * RDRAND is only used to mitigate prediction if a key is compromised
 */

static void
randombytes_internal_random_xorhwrand(void)
{
/* LCOV_EXCL_START */
#ifdef HAVE_RDRAND
    unsigned int r;

    if (global.rdrand_available == 0) {
        return;
    }
    (void) _rdrand32_step(&r);
    * (uint32_t *) (void *)
        &stream.key[crypto_stream_chacha20_KEYBYTES - 4] ^= (uint32_t) r;
#endif
/* LCOV_EXCL_STOP */
}

/*
 * XOR the key with another same-length secret
 */

static inline void
randombytes_internal_random_xorkey(const unsigned char * const mix)
{
    unsigned char *key = stream.key;
    size_t         i;

    for (i = (size_t) 0U; i < sizeof stream.key; i++) {
        key[i] ^= mix[i];
    }
}

/*
 * Put `size` random bytes into `buf` and overwrite the key
 */

static void
randombytes_internal_random_buf(void * const buf, const size_t size)
{
    size_t i;
    int    ret;

    randombytes_internal_random_stir_if_needed();
    COMPILER_ASSERT(sizeof stream.nonce == crypto_stream_chacha20_NONCEBYTES);
#if defined(ULLONG_MAX) && defined(SIZE_MAX)
# if SIZE_MAX > ULLONG_MAX
    /* coverity[result_independent_of_operands] */
    assert(size <= ULLONG_MAX);
# endif
#endif
    ret = crypto_stream_chacha20((unsigned char *) buf, (unsigned long long) size,
                                 (unsigned char *) &stream.nonce, stream.key);
    assert(ret == 0);
    for (i = 0U; i < sizeof size; i++) {
        stream.key[i] ^= ((const unsigned char *) (const void *) &size)[i];
    }
    randombytes_internal_random_xorhwrand();
    stream.nonce++;
    crypto_stream_chacha20_xor(stream.key, stream.key, sizeof stream.key,
                               (unsigned char *) &stream.nonce, stream.key);
}

/*
 * Pop a 32-bit value from the random pool
 *
 * Overwrite the key after the pool gets refilled.
 */

static uint32_t
randombytes_internal_random(void)
{
    uint32_t val;
    int      ret;

    COMPILER_ASSERT(sizeof stream.rnd32 >= (sizeof stream.key) + (sizeof val));
    COMPILER_ASSERT(((sizeof stream.rnd32) - (sizeof stream.key))
                    % sizeof val == (size_t) 0U);
    if (stream.rnd32_outleft <= (size_t) 0U) {
        randombytes_internal_random_stir_if_needed();
        COMPILER_ASSERT(sizeof stream.nonce == crypto_stream_chacha20_NONCEBYTES);
        ret = crypto_stream_chacha20((unsigned char *) stream.rnd32,
                                     (unsigned long long) sizeof stream.rnd32,
                                     (unsigned char *) &stream.nonce,
                                     stream.key);
        assert(ret == 0);
        stream.rnd32_outleft = (sizeof stream.rnd32) - (sizeof stream.key);
        randombytes_internal_random_xorhwrand();
        randombytes_internal_random_xorkey(&stream.rnd32[stream.rnd32_outleft]);
        memset(&stream.rnd32[stream.rnd32_outleft], 0, sizeof stream.key);
        stream.nonce++;
    }
    stream.rnd32_outleft -= sizeof val;
    memcpy(&val, &stream.rnd32[stream.rnd32_outleft], sizeof val);
    memset(&stream.rnd32[stream.rnd32_outleft], 0, sizeof val);

    return val;
}

static const char *
randombytes_internal_implementation_name(void)
{
    return "internal";
}

struct randombytes_implementation randombytes_internal_implementation = {
    SODIUM_C99(.implementation_name =) randombytes_internal_implementation_name,
    SODIUM_C99(.random =) randombytes_internal_random,
    SODIUM_C99(.stir =) randombytes_internal_random_stir,
    SODIUM_C99(.uniform =) NULL,
    SODIUM_C99(.buf =) randombytes_internal_random_buf,
    SODIUM_C99(.close =) randombytes_internal_random_close
};
