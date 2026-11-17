
#include <string.h>

#include "crypto_scalarmult_ed25519.h"
#include "crypto_scalarmult_ristretto255.h"
#include "private/ed25519_ref10.h"
#include "utils.h"

int
crypto_scalarmult_ristretto255(unsigned char *q, const unsigned char *n,
                               const unsigned char *p)
{
    unsigned char *t = q;
    ge25519_p3     Q;
    ge25519_p3     P;
    unsigned int   i;

    if (ristretto255_frombytes(&P, p) != 0) {
        return -1;
    }
    for (i = 0; i < 32; ++i) {
        t[i] = n[i];
    }
    t[31] &= 127;
    ge25519_scalarmult(&Q, t, &P);
    ristretto255_p3_tobytes(q, &Q);
    if (sodium_is_zero(q, 32)) {
        return -1;
    }
    return 0;
}

int
crypto_scalarmult_ristretto255_base(unsigned char *q,
                                    const unsigned char *n)
{
    unsigned char *t = q;
    ge25519_p3     Q;
    unsigned int   i;

    for (i = 0; i < 32; ++i) {
        t[i] = n[i];
    }
    t[31] &= 127;
    ge25519_scalarmult_base(&Q, t);
    ristretto255_p3_tobytes(q, &Q);
    if (sodium_is_zero(q, 32)) {
        return -1;
    }
    return 0;
}

size_t
crypto_scalarmult_ristretto255_bytes(void)
{
    return crypto_scalarmult_ristretto255_BYTES;
}

size_t
crypto_scalarmult_ristretto255_scalarbytes(void)
{
    return crypto_scalarmult_ristretto255_SCALARBYTES;
}
