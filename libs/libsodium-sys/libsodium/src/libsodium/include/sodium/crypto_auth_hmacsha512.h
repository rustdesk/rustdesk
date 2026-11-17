#ifndef crypto_auth_hmacsha512_H
#define crypto_auth_hmacsha512_H

#include <stddef.h>
#include "crypto_hash_sha512.h"
#include "export.h"

#ifdef __cplusplus
# ifdef __GNUC__
#  pragma GCC diagnostic ignored "-Wlong-long"
# endif
extern "C" {
#endif

#define crypto_auth_hmacsha512_BYTES 64U
SODIUM_EXPORT
size_t crypto_auth_hmacsha512_bytes(void);

#define crypto_auth_hmacsha512_KEYBYTES 32U
SODIUM_EXPORT
size_t crypto_auth_hmacsha512_keybytes(void);

SODIUM_EXPORT
int crypto_auth_hmacsha512(unsigned char *out,
                           const unsigned char *in,
                           unsigned long long inlen,
                           const unsigned char *k) __attribute__ ((nonnull(1, 4)));

SODIUM_EXPORT
int crypto_auth_hmacsha512_verify(const unsigned char *h,
                                  const unsigned char *in,
                                  unsigned long long inlen,
                                  const unsigned char *k)
            __attribute__ ((warn_unused_result)) __attribute__ ((nonnull(1, 4)));

/* ------------------------------------------------------------------------- */

typedef struct crypto_auth_hmacsha512_state {
    crypto_hash_sha512_state ictx;
    crypto_hash_sha512_state octx;
} crypto_auth_hmacsha512_state;

SODIUM_EXPORT
size_t crypto_auth_hmacsha512_statebytes(void);

SODIUM_EXPORT
int crypto_auth_hmacsha512_init(crypto_auth_hmacsha512_state *state,
                                const unsigned char *key,
                                size_t keylen) __attribute__ ((nonnull));

SODIUM_EXPORT
int crypto_auth_hmacsha512_update(crypto_auth_hmacsha512_state *state,
                                  const unsigned char *in,
                                  unsigned long long inlen) __attribute__ ((nonnull(1)));

SODIUM_EXPORT
int crypto_auth_hmacsha512_final(crypto_auth_hmacsha512_state *state,
                                 unsigned char *out) __attribute__ ((nonnull));

SODIUM_EXPORT
void crypto_auth_hmacsha512_keygen(unsigned char k[crypto_auth_hmacsha512_KEYBYTES])
            __attribute__ ((nonnull));

#ifdef __cplusplus
}
#endif

#endif
