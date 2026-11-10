#define TEST_NAME "core_ed25519"
#include "cmptest.h"

static const unsigned char non_canonical_p[32] = {
    0xf6, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
    0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x7f
};
static const unsigned char non_canonical_invalid_p[32] = {
    0xf5, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
    0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x7f
};
static const unsigned char max_canonical_p[32] = {
    0xe4, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
    0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x7f
};
static const unsigned char L_p1[32] = {
    0xee, 0xd3, 0xf5, 0x5c, 0x1a, 0x63, 0x12, 0x58, 0xd6, 0x9c, 0xf7, 0xa2, 0xde, 0xf9, 0xde, 0x14,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x10
};
static const unsigned char L[32] = {
    0xed, 0xd3, 0xf5, 0x5c, 0x1a, 0x63, 0x12, 0x58, 0xd6, 0x9c, 0xf7, 0xa2, 0xde, 0xf9, 0xde, 0x14,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x10
};
static const unsigned char L_1[32] = {
    0xec, 0xd3, 0xf5, 0x5c, 0x1a, 0x63, 0x12, 0x58, 0xd6, 0x9c, 0xf7, 0xa2, 0xde, 0xf9, 0xde, 0x14,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x10
};
static const unsigned char sc_8[32] = {
    0x08, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00
};
static const unsigned char sc_highbit[32] = {
    0x08, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x80
};

static void
add_P(unsigned char * const S)
{
    static const unsigned char P[32] = {
        0xed, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
        0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
        0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
        0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x7f
    };

    sodium_add(S, P, sizeof P);
}

static void
add_l64(unsigned char * const S)
{
    static const unsigned char l[crypto_core_ed25519_NONREDUCEDSCALARBYTES] =
      { 0xed, 0xd3, 0xf5, 0x5c, 0x1a, 0x63, 0x12, 0x58,
        0xd6, 0x9c, 0xf7, 0xa2, 0xde, 0xf9, 0xde, 0x14,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x10,
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0 };

    sodium_add(S, l, sizeof l);
}

int
main(void)
{
    unsigned char *r;
    unsigned char *p, *p2, *p3;
    unsigned char *sc, *sc2, *sc3;
    unsigned char *sc64;
    char          *hex;
    unsigned int   i, j;

    r = (unsigned char *) sodium_malloc(crypto_core_ed25519_UNIFORMBYTES);
    p = (unsigned char *) sodium_malloc(crypto_core_ed25519_BYTES);
    for (i = 0; i < 500; i++) {
        randombytes_buf(r, crypto_core_ed25519_UNIFORMBYTES);
        if (crypto_core_ed25519_from_uniform(p, r) != 0) {
            printf("crypto_core_ed25519_from_uniform() failed\n");
        }
        if (crypto_core_ed25519_is_valid_point(p) == 0) {
            printf("crypto_core_ed25519_from_uniform() returned an invalid point\n");
        }

        crypto_core_ed25519_random(p);
        if (crypto_core_ed25519_is_valid_point(p) == 0) {
            printf("crypto_core_ed25519_random() returned an invalid point\n");
        }
    }

    p2 = (unsigned char *) sodium_malloc(crypto_core_ed25519_BYTES);
    p3 = (unsigned char *) sodium_malloc(crypto_core_ed25519_BYTES);

    crypto_core_ed25519_random(p2);

    j = 1 + (unsigned int) randombytes_uniform(100);
    memcpy(p3, p, crypto_core_ed25519_BYTES);
    for (i = 0; i < j; i++) {
        crypto_core_ed25519_add(p, p, p2);
        if (crypto_core_ed25519_is_valid_point(p) != 1) {
            printf("crypto_core_add() returned an invalid point\n");
        }
    }
    if (memcmp(p, p3, crypto_core_ed25519_BYTES) == 0) {
        printf("crypto_core_add() failed\n");
    }
    for (i = 0; i < j; i++) {
        crypto_core_ed25519_sub(p, p, p2);
    }
    if (memcmp(p, p3, crypto_core_ed25519_BYTES) != 0) {
        printf("crypto_core_add() or crypto_core_sub() failed\n");
    }
    sc = (unsigned char *) sodium_malloc(crypto_scalarmult_ed25519_SCALARBYTES);
    memset(sc, 0, crypto_scalarmult_ed25519_SCALARBYTES);
    sc[0] = 8;
    memcpy(p2, p, crypto_core_ed25519_BYTES);
    memcpy(p3, p, crypto_core_ed25519_BYTES);

    for (i = 0; i < 254; i++) {
        crypto_core_ed25519_add(p2, p2, p2);
    }
    for (i = 0; i < 8; i++) {
        crypto_core_ed25519_add(p2, p2, p);
    }
    if (crypto_scalarmult_ed25519(p3, sc, p) != 0) {
        printf("crypto_scalarmult_ed25519() failed\n");
    }
    if (memcmp(p2, p3, crypto_core_ed25519_BYTES) != 0) {
        printf("crypto_scalarmult_ed25519() is inconsistent with crypto_core_ed25519_add()\n");
    }

    assert(crypto_core_ed25519_is_valid_point(p) == 1);

    memset(p, 0, crypto_core_ed25519_BYTES);
    assert(crypto_core_ed25519_is_valid_point(p) == 0);

    p[0] = 1;
    assert(crypto_core_ed25519_is_valid_point(p) == 0);

    p[0] = 2;
    assert(crypto_core_ed25519_is_valid_point(p) == 0);

    p[0] = 9;
    assert(crypto_core_ed25519_is_valid_point(p) == 1);

    assert(crypto_core_ed25519_is_valid_point(max_canonical_p) == 1);
    assert(crypto_core_ed25519_is_valid_point(non_canonical_invalid_p) == 0);
    assert(crypto_core_ed25519_is_valid_point(non_canonical_p) == 0);

    memcpy(p2, p, crypto_core_ed25519_BYTES);
    add_P(p2);
    crypto_core_ed25519_add(p3, p2, p2);
    crypto_core_ed25519_sub(p3, p3, p2);
    assert(memcmp(p2, p, crypto_core_ed25519_BYTES) != 0);
    assert(memcmp(p3, p, crypto_core_ed25519_BYTES) == 0);

    p[0] = 2;
    assert(crypto_core_ed25519_add(p3, p2, p) == -1);
    assert(crypto_core_ed25519_add(p3, p2, non_canonical_p) == 0);
    assert(crypto_core_ed25519_add(p3, p2, non_canonical_invalid_p) == -1);
    assert(crypto_core_ed25519_add(p3, p, p3) == -1);
    assert(crypto_core_ed25519_add(p3, non_canonical_p, p3) == 0);
    assert(crypto_core_ed25519_add(p3, non_canonical_invalid_p, p3) == -1);

    assert(crypto_core_ed25519_sub(p3, p2, p) == -1);
    assert(crypto_core_ed25519_sub(p3, p2, non_canonical_p) == 0);
    assert(crypto_core_ed25519_sub(p3, p2, non_canonical_invalid_p) == -1);
    assert(crypto_core_ed25519_sub(p3, p, p3) == -1);
    assert(crypto_core_ed25519_sub(p3, non_canonical_p, p3) == 0);
    assert(crypto_core_ed25519_sub(p3, non_canonical_invalid_p, p3) == -1);

    for (i = 0; i < 1000; i++) {
        crypto_core_ed25519_random(p);
        do {
            crypto_core_ed25519_scalar_random(sc);
        } while (sodium_is_zero(sc, crypto_core_ed25519_SCALARBYTES));
        if (crypto_scalarmult_ed25519_noclamp(p2, sc, p) != 0) {
            printf("crypto_scalarmult_ed25519_noclamp() failed\n");
        }
        assert(crypto_core_ed25519_is_valid_point(p2));
        if (crypto_core_ed25519_scalar_invert(sc, sc) != 0) {
            printf("crypto_core_ed25519_scalar_invert() failed\n");
        }
        if (crypto_scalarmult_ed25519_noclamp(p3, sc, p2) != 0) {
            printf("crypto_scalarmult_ed25519_noclamp() failed\n");
        }
        assert(memcmp(p3, p, crypto_core_ed25519_BYTES) == 0);
    }

    sc64 = (unsigned char *) sodium_malloc(64);
    crypto_core_ed25519_scalar_random(sc);
    memcpy(sc64, sc, crypto_core_ed25519_BYTES);
    memset(sc64 + crypto_core_ed25519_BYTES, 0,
           64 - crypto_core_ed25519_BYTES);
    i = (unsigned int) randombytes_uniform(100);
    do {
        add_l64(sc64);
    } while (i-- > 0);
    crypto_core_ed25519_scalar_reduce(sc64, sc64);
    if (memcmp(sc64, sc, crypto_core_ed25519_BYTES) != 0) {
        printf("crypto_core_ed25519_scalar_reduce() failed\n");
    }

    randombytes_buf(r, crypto_core_ed25519_UNIFORMBYTES);
    crypto_core_ed25519_from_uniform(p, r);
    memcpy(p2, p, crypto_core_ed25519_BYTES);
    crypto_core_ed25519_scalar_random(sc);
    if (crypto_scalarmult_ed25519_noclamp(p, sc, p) != 0) {
        printf("crypto_scalarmult_ed25519_noclamp() failed (1)\n");
    }
    crypto_core_ed25519_scalar_complement(sc, sc);
    if (crypto_scalarmult_ed25519_noclamp(p2, sc, p2) != 0) {
        printf("crypto_scalarmult_ed25519_noclamp() failed (2)\n");
    }
    crypto_core_ed25519_add(p3, p, p2);
    crypto_core_ed25519_from_uniform(p, r);
    crypto_core_ed25519_sub(p, p, p3);
    assert(p[0] == 0x01);
    for (i = 1; i < crypto_core_ed25519_BYTES; i++) {
        assert(p[i] == 0);
    }

    crypto_core_ed25519_random(p);
    memcpy(p2, p, crypto_core_ed25519_BYTES);
    crypto_core_ed25519_scalar_random(sc);
    if (crypto_scalarmult_ed25519_noclamp(p, sc, p) != 0) {
        printf("crypto_scalarmult_ed25519_noclamp() failed (3)\n");
    }
    crypto_core_ed25519_scalar_negate(sc, sc);
    if (crypto_scalarmult_ed25519_noclamp(p2, sc, p2) != 0) {
        printf("crypto_scalarmult_ed25519_noclamp() failed (4)\n");
    }
    crypto_core_ed25519_add(p, p, p2);
    assert(p[0] == 0x01);
    for (i = 1; i < crypto_core_ed25519_BYTES; i++) {
        assert(p[i] == 0);
    }

    hex = (char *) sodium_malloc(crypto_core_ed25519_SCALARBYTES * 2 + 1);

    for (i = 0; i < crypto_core_ed25519_SCALARBYTES; i++) {
        sc[i] = 255 - i;
    }
    if (crypto_core_ed25519_scalar_invert(sc, sc) != 0) {
        printf("crypto_core_ed25519_scalar_invert() failed\n");
    }
    sodium_bin2hex(hex, crypto_core_ed25519_SCALARBYTES * 2 + 1,
                   sc, crypto_core_ed25519_SCALARBYTES);
    printf("inv1: %s\n", hex);
    if (crypto_core_ed25519_scalar_invert(sc, sc) != 0) {
        printf("crypto_core_ed25519_scalar_invert() failed\n");
    }
    sodium_bin2hex(hex, crypto_core_ed25519_SCALARBYTES * 2 + 1,
                   sc, crypto_core_ed25519_SCALARBYTES);
    printf("inv2: %s\n", hex);
    for (i = 0; i < crypto_core_ed25519_SCALARBYTES; i++) {
        sc[i] = 32 - i;
    }
    if (crypto_core_ed25519_scalar_invert(sc, sc) != 0) {
        printf("crypto_core_ed25519_scalar_invert() failed\n");
    }

    sodium_bin2hex(hex, crypto_core_ed25519_SCALARBYTES * 2 + 1,
                   sc, crypto_core_ed25519_SCALARBYTES);
    printf("inv3: %s\n", hex);
    if (crypto_core_ed25519_scalar_invert(sc, sc) != 0) {
        printf("crypto_core_ed25519_scalar_invert() failed\n");
    }
    sodium_bin2hex(hex, crypto_core_ed25519_SCALARBYTES * 2 + 1,
                   sc, crypto_core_ed25519_SCALARBYTES);
    printf("inv4: %s\n", hex);

    for (i = 0; i < crypto_core_ed25519_SCALARBYTES; i++) {
        sc[i] = 255 - i;
    }
    crypto_core_ed25519_scalar_negate(sc, sc);
    sodium_bin2hex(hex, crypto_core_ed25519_SCALARBYTES * 2 + 1,
                   sc, crypto_core_ed25519_SCALARBYTES);
    printf("neg1: %s\n", hex);
    crypto_core_ed25519_scalar_negate(sc, sc);
    sodium_bin2hex(hex, crypto_core_ed25519_SCALARBYTES * 2 + 1,
                   sc, crypto_core_ed25519_SCALARBYTES);
    printf("neg2: %s\n", hex);
    for (i = 0; i < crypto_core_ed25519_SCALARBYTES; i++) {
        sc[i] = 32 - i;
    }
    crypto_core_ed25519_scalar_negate(sc, sc);
    sodium_bin2hex(hex, crypto_core_ed25519_SCALARBYTES * 2 + 1,
                   sc, crypto_core_ed25519_SCALARBYTES);
    printf("neg3: %s\n", hex);
    crypto_core_ed25519_scalar_negate(sc, sc);
    sodium_bin2hex(hex, crypto_core_ed25519_SCALARBYTES * 2 + 1,
                   sc, crypto_core_ed25519_SCALARBYTES);
    printf("neg4: %s\n", hex);

    for (i = 0; i < crypto_core_ed25519_SCALARBYTES; i++) {
        sc[i] = 255 - i;
    }
    crypto_core_ed25519_scalar_complement(sc, sc);
    sodium_bin2hex(hex, crypto_core_ed25519_SCALARBYTES * 2 + 1,
                   sc, crypto_core_ed25519_SCALARBYTES);
    printf("comp1: %s\n", hex);
    crypto_core_ed25519_scalar_complement(sc, sc);
    sodium_bin2hex(hex, crypto_core_ed25519_SCALARBYTES * 2 + 1,
                   sc, crypto_core_ed25519_SCALARBYTES);
    printf("comp2: %s\n", hex);
    for (i = 0; i < crypto_core_ed25519_SCALARBYTES; i++) {
        sc[i] = 32 - i;
    }
    crypto_core_ed25519_scalar_complement(sc, sc);
    sodium_bin2hex(hex, crypto_core_ed25519_SCALARBYTES * 2 + 1,
                   sc, crypto_core_ed25519_SCALARBYTES);
    printf("comp3: %s\n", hex);
    crypto_core_ed25519_scalar_complement(sc, sc);
    sodium_bin2hex(hex, crypto_core_ed25519_SCALARBYTES * 2 + 1,
                   sc, crypto_core_ed25519_SCALARBYTES);
    printf("comp4: %s\n", hex);

    sc2 = (unsigned char *) sodium_malloc(crypto_core_ed25519_SCALARBYTES);
    sc3 = (unsigned char *) sodium_malloc(crypto_core_ed25519_SCALARBYTES);
    for (i = 0; i < 1000; i++) {
        randombytes_buf(sc, crypto_core_ed25519_SCALARBYTES);
        randombytes_buf(sc2, crypto_core_ed25519_SCALARBYTES);
        sc[crypto_core_ed25519_SCALARBYTES - 1] &= 0x7f;
        sc2[crypto_core_ed25519_SCALARBYTES - 1] &= 0x7f;
        crypto_core_ed25519_scalar_add(sc3, sc, sc2);
        assert(!sodium_is_zero(sc, crypto_core_ed25519_SCALARBYTES));
        crypto_core_ed25519_scalar_sub(sc3, sc3, sc2);
        assert(!sodium_is_zero(sc, crypto_core_ed25519_SCALARBYTES));
        crypto_core_ed25519_scalar_sub(sc3, sc3, sc);
        assert(sodium_is_zero(sc3, crypto_core_ed25519_SCALARBYTES));
    }

    memset(sc, 0x69, crypto_core_ed25519_SCALARBYTES);
    memset(sc2, 0x42, crypto_core_ed25519_SCALARBYTES);
    crypto_core_ed25519_scalar_add(sc, sc, sc2);
    crypto_core_ed25519_scalar_add(sc, sc2, sc);
    sodium_bin2hex(hex, crypto_core_ed25519_SCALARBYTES * 2 + 1,
                   sc, crypto_core_ed25519_SCALARBYTES);
    printf("add1: %s\n", hex);

    crypto_core_ed25519_scalar_sub(sc, sc2, sc);
    crypto_core_ed25519_scalar_sub(sc, sc, sc2);
    sodium_bin2hex(hex, crypto_core_ed25519_SCALARBYTES * 2 + 1,
                   sc, crypto_core_ed25519_SCALARBYTES);
    printf("sub1: %s\n", hex);

    memset(sc, 0xcd, crypto_core_ed25519_SCALARBYTES);
    memset(sc2, 0x42, crypto_core_ed25519_SCALARBYTES);
    crypto_core_ed25519_scalar_add(sc, sc, sc2);
    crypto_core_ed25519_scalar_add(sc, sc2, sc);
    sodium_bin2hex(hex, crypto_core_ed25519_SCALARBYTES * 2 + 1,
                   sc, crypto_core_ed25519_SCALARBYTES);
    printf("add2: %s\n", hex);

    crypto_core_ed25519_scalar_sub(sc, sc2, sc);
    crypto_core_ed25519_scalar_sub(sc, sc, sc2);
    sodium_bin2hex(hex, crypto_core_ed25519_SCALARBYTES * 2 + 1,
                   sc, crypto_core_ed25519_SCALARBYTES);
    printf("sub2: %s\n", hex);

    memset(sc, 0x69, crypto_core_ed25519_SCALARBYTES);
    memset(sc2, 0x42, crypto_core_ed25519_SCALARBYTES);
    for (i = 0; i < 100; i++) {
        crypto_core_ed25519_scalar_mul(sc, sc, sc2);
        crypto_core_ed25519_scalar_mul(sc2, sc, sc2);
    }
    sodium_bin2hex(hex, crypto_core_ed25519_SCALARBYTES * 2 + 1,
                   sc2, crypto_core_ed25519_SCALARBYTES);
    printf("mul: %s\n", hex);
    for (i = 0; i < 1000; i++) {
        crypto_core_ed25519_scalar_random(sc);
        memset(sc2, 0, crypto_core_ed25519_SCALARBYTES);
        crypto_core_ed25519_scalar_mul(sc3, sc, sc2);
        assert(sodium_is_zero(sc3, crypto_core_ed25519_SCALARBYTES));

        sc2[0]++;
        crypto_core_ed25519_scalar_mul(sc3, sc, sc2);
        assert(memcmp(sc3, sc, crypto_core_ed25519_SCALARBYTES) == 0);

        sc2[0]++;
        crypto_core_ed25519_scalar_mul(sc3, sc, sc2);
        crypto_core_ed25519_scalar_sub(sc3, sc3, sc);
        crypto_core_ed25519_scalar_sub(sc3, sc3, sc);
        assert(sodium_is_zero(sc3, crypto_core_ed25519_SCALARBYTES));

        do {
            crypto_core_ed25519_scalar_random(sc2);
        } while (sodium_is_zero(sc2, crypto_core_ed25519_SCALARBYTES));
        crypto_core_ed25519_scalar_mul(sc3, sc, sc2);
        crypto_core_ed25519_scalar_invert(sc2, sc2);
        crypto_core_ed25519_scalar_mul(sc3, sc3, sc2);
        assert(memcmp(sc3, sc, crypto_core_ed25519_SCALARBYTES) == 0);

        sc[31] |= 0x11;
        memset(sc2, 0, crypto_core_ed25519_SCALARBYTES);
        sc2[0] = 1;
        crypto_core_ed25519_scalar_mul(sc3, sc, sc2);
        assert(memcmp(sc3, sc, crypto_core_ed25519_SCALARBYTES) != 0);
    }

    crypto_core_ed25519_scalar_mul(sc, L_1, sc_8);
    sodium_bin2hex(hex, crypto_core_ed25519_SCALARBYTES * 2 + 1,
                   sc, crypto_core_ed25519_SCALARBYTES);
    printf("(L-1)*8: %s\n", hex);
    crypto_core_ed25519_scalar_mul(sc, sc_8, L_1);
    sodium_bin2hex(hex, crypto_core_ed25519_SCALARBYTES * 2 + 1,
                   sc, crypto_core_ed25519_SCALARBYTES);
    printf("8(L-1): %s\n", hex);
    crypto_core_ed25519_scalar_mul(sc, L_1, L_1);
    sodium_bin2hex(hex, crypto_core_ed25519_SCALARBYTES * 2 + 1,
                   sc, crypto_core_ed25519_SCALARBYTES);
    printf("(L-1)^2: %s\n", hex);
    crypto_core_ed25519_scalar_mul(sc, L, sc_8);

    crypto_core_ed25519_scalar_mul(sc, L_p1, sc_8);
    sodium_bin2hex(hex, crypto_core_ed25519_SCALARBYTES * 2 + 1,
                   sc, crypto_core_ed25519_SCALARBYTES);
    printf("(L+1)*8: %s\n", hex);
    crypto_core_ed25519_scalar_mul(sc, sc_8, L_p1);
    sodium_bin2hex(hex, crypto_core_ed25519_SCALARBYTES * 2 + 1,
                   sc, crypto_core_ed25519_SCALARBYTES);
    printf("8(L+1): %s\n", hex);
    crypto_core_ed25519_scalar_mul(sc, L_p1, L_p1);
    sodium_bin2hex(hex, crypto_core_ed25519_SCALARBYTES * 2 + 1,
                   sc, crypto_core_ed25519_SCALARBYTES);
    printf("(L+1)^2: %s\n", hex);

    crypto_core_ed25519_scalar_mul(sc, L_1, sc_highbit);
    sodium_bin2hex(hex, crypto_core_ed25519_SCALARBYTES * 2 + 1,
                   sc, crypto_core_ed25519_SCALARBYTES);
    printf("(L-1)h: %s\n", hex);
    crypto_core_ed25519_scalar_mul(sc, sc_highbit, L_1);
    sodium_bin2hex(hex, crypto_core_ed25519_SCALARBYTES * 2 + 1,
                   sc, crypto_core_ed25519_SCALARBYTES);
    printf("h(L-1): %s\n", hex);
    crypto_core_ed25519_scalar_mul(sc, L_p1, sc_highbit);
    sodium_bin2hex(hex, crypto_core_ed25519_SCALARBYTES * 2 + 1,
                   sc, crypto_core_ed25519_SCALARBYTES);
    printf("(L+1)h: %s\n", hex);
    crypto_core_ed25519_scalar_mul(sc, sc_highbit, L_p1);
    sodium_bin2hex(hex, crypto_core_ed25519_SCALARBYTES * 2 + 1,
                   sc, crypto_core_ed25519_SCALARBYTES);
    printf("h(L+1): %s\n", hex);
    crypto_core_ed25519_scalar_mul(sc, sc_highbit, sc_highbit);
    sodium_bin2hex(hex, crypto_core_ed25519_SCALARBYTES * 2 + 1,
                   sc, crypto_core_ed25519_SCALARBYTES);
    printf("h^2: %s\n", hex);

    crypto_core_ed25519_scalar_mul(sc, L, sc_8);
    assert(sodium_is_zero(sc, crypto_core_ed25519_SCALARBYTES));
    crypto_core_ed25519_scalar_mul(sc, sc_8, L);
    assert(sodium_is_zero(sc, crypto_core_ed25519_SCALARBYTES));
    crypto_core_ed25519_scalar_mul(sc, L, L);
    assert(sodium_is_zero(sc, crypto_core_ed25519_SCALARBYTES));
    crypto_core_ed25519_scalar_mul(sc, L, L_1);
    assert(sodium_is_zero(sc, crypto_core_ed25519_SCALARBYTES));
    crypto_core_ed25519_scalar_mul(sc, L_1, L);
    assert(sodium_is_zero(sc, crypto_core_ed25519_SCALARBYTES));

    crypto_core_ed25519_scalar_add(sc, L_1, sc_8);
    sodium_bin2hex(hex, crypto_core_ed25519_SCALARBYTES * 2 + 1,
                   sc, crypto_core_ed25519_SCALARBYTES);
    printf("(L-1)+8: %s\n", hex);
    crypto_core_ed25519_scalar_add(sc, sc_8, L_1);
    sodium_bin2hex(hex, crypto_core_ed25519_SCALARBYTES * 2 + 1,
                   sc, crypto_core_ed25519_SCALARBYTES);
    printf("8+(L-1): %s\n", hex);
    crypto_core_ed25519_scalar_add(sc, L_1, L_1);
    sodium_bin2hex(hex, crypto_core_ed25519_SCALARBYTES * 2 + 1,
                   sc, crypto_core_ed25519_SCALARBYTES);
    printf("(L-1)*2: %s\n", hex);
    crypto_core_ed25519_scalar_add(sc, L, sc_8);

    crypto_core_ed25519_scalar_add(sc, L_p1, sc_8);
    sodium_bin2hex(hex, crypto_core_ed25519_SCALARBYTES * 2 + 1,
                   sc, crypto_core_ed25519_SCALARBYTES);
    printf("(L+1)+8: %s\n", hex);
    crypto_core_ed25519_scalar_add(sc, sc_8, L_p1);
    sodium_bin2hex(hex, crypto_core_ed25519_SCALARBYTES * 2 + 1,
                   sc, crypto_core_ed25519_SCALARBYTES);
    printf("8+(L+1): %s\n", hex);
    crypto_core_ed25519_scalar_add(sc, L_p1, L_p1);
    sodium_bin2hex(hex, crypto_core_ed25519_SCALARBYTES * 2 + 1,
                   sc, crypto_core_ed25519_SCALARBYTES);
    printf("(L+1)*2: %s\n", hex);

    crypto_core_ed25519_scalar_add(sc, L_1, sc_highbit);
    sodium_bin2hex(hex, crypto_core_ed25519_SCALARBYTES * 2 + 1,
                   sc, crypto_core_ed25519_SCALARBYTES);
    printf("(L-1)+h: %s\n", hex);
    crypto_core_ed25519_scalar_add(sc, sc_highbit, L_1);
    sodium_bin2hex(hex, crypto_core_ed25519_SCALARBYTES * 2 + 1,
                   sc, crypto_core_ed25519_SCALARBYTES);
    printf("h+(L-1): %s\n", hex);
    crypto_core_ed25519_scalar_add(sc, L_p1, sc_highbit);
    sodium_bin2hex(hex, crypto_core_ed25519_SCALARBYTES * 2 + 1,
                   sc, crypto_core_ed25519_SCALARBYTES);
    printf("(L+1)+h: %s\n", hex);
    crypto_core_ed25519_scalar_add(sc, sc_highbit, L_p1);
    sodium_bin2hex(hex, crypto_core_ed25519_SCALARBYTES * 2 + 1,
                   sc, crypto_core_ed25519_SCALARBYTES);
    printf("h+(L+1): %s\n", hex);
    crypto_core_ed25519_scalar_add(sc, sc_highbit, sc_highbit);
    sodium_bin2hex(hex, crypto_core_ed25519_SCALARBYTES * 2 + 1,
                   sc, crypto_core_ed25519_SCALARBYTES);
    printf("h*2: %s\n", hex);

    sodium_free(hex);
    sodium_free(sc64);
    sodium_free(sc3);
    sodium_free(sc2);
    sodium_free(sc);
    sodium_free(p3);
    sodium_free(p2);
    sodium_free(p);
    sodium_free(r);

    assert(crypto_core_ed25519_BYTES == crypto_core_ed25519_bytes());
    assert(crypto_core_ed25519_SCALARBYTES == crypto_core_ed25519_scalarbytes());
    assert(crypto_core_ed25519_NONREDUCEDSCALARBYTES == crypto_core_ed25519_nonreducedscalarbytes());
    assert(crypto_core_ed25519_NONREDUCEDSCALARBYTES >= crypto_core_ed25519_SCALARBYTES);
    assert(crypto_core_ed25519_UNIFORMBYTES == crypto_core_ed25519_uniformbytes());
    assert(crypto_core_ed25519_UNIFORMBYTES >= crypto_core_ed25519_BYTES);

    printf("OK\n");

    return 0;
}
