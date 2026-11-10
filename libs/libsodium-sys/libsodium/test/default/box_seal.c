
#define TEST_NAME "box_seal"
#include "cmptest.h"

static
void tv1(void)
{
    unsigned char  pk[crypto_box_PUBLICKEYBYTES];
    unsigned char  sk[crypto_box_SECRETKEYBYTES];
    unsigned char *c;
    unsigned char *m;
    unsigned char *m2;
    size_t         m_len;
    size_t         c_len;

    crypto_box_keypair(pk, sk);
    m_len = (size_t) randombytes_uniform(1000);
    c_len = crypto_box_SEALBYTES + m_len;
    m     = (unsigned char *) sodium_malloc(m_len);
    m2    = (unsigned char *) sodium_malloc(m_len);
    c     = (unsigned char *) sodium_malloc(c_len);
    randombytes_buf(m, m_len);
    if (crypto_box_seal(c, m, m_len, pk) != 0) {
        printf("crypto_box_seal() failure\n");
        return;
    }
    if (crypto_box_seal_open(m2, c, c_len, pk, sk) != 0) {
        printf("crypto_box_seal_open() failure\n");
        return;
    }
    printf("%d\n", memcmp(m, m2, m_len));

    printf("%d\n", crypto_box_seal_open(m, c, 0U, pk, sk));
    printf("%d\n", crypto_box_seal_open(m, c, c_len - 1U, pk, sk));
    printf("%d\n", crypto_box_seal_open(m, c, c_len, sk, pk));

    sodium_free(c);
    sodium_free(m);
    sodium_free(m2);

    assert(crypto_box_sealbytes() == crypto_box_SEALBYTES);
}

static
void tv2(void)
{
    unsigned char  pk[crypto_box_PUBLICKEYBYTES];
    unsigned char  sk[crypto_box_SECRETKEYBYTES];
    unsigned char *cm;
    unsigned char *m2;
    size_t         m_len;
    size_t         cm_len;

    crypto_box_keypair(pk, sk);
    m_len = (size_t) randombytes_uniform(1000);
    cm_len = crypto_box_SEALBYTES + m_len;
    m2    = (unsigned char *) sodium_malloc(m_len);
    cm    = (unsigned char *) sodium_malloc(cm_len);
    randombytes_buf(cm, m_len);
    if (crypto_box_seal(cm, cm, m_len, pk) != 0) {
        printf("crypto_box_seal() failure\n");
        return;
    }
    if (crypto_box_seal_open(m2, cm, cm_len, pk, sk) != 0) {
        printf("crypto_box_seal_open() failure\n");
        return;
    }
    assert(m_len == 0 || memcmp(cm, m2, m_len) != 0);
    sodium_free(cm);
    sodium_free(m2);
}

#ifndef SODIUM_LIBRARY_MINIMAL
static
void tv3(void)
{
    unsigned char  pk[crypto_box_curve25519xchacha20poly1305_PUBLICKEYBYTES];
    unsigned char  sk[crypto_box_curve25519xchacha20poly1305_SECRETKEYBYTES];
    unsigned char *c;
    unsigned char *m;
    unsigned char *m2;
    size_t         m_len;
    size_t         c_len;

    crypto_box_curve25519xchacha20poly1305_keypair(pk, sk);
    m_len = (size_t) randombytes_uniform(1000);
    c_len = crypto_box_curve25519xchacha20poly1305_SEALBYTES + m_len;
    m     = (unsigned char *) sodium_malloc(m_len);
    m2    = (unsigned char *) sodium_malloc(m_len);
    c     = (unsigned char *) sodium_malloc(c_len);
    randombytes_buf(m, m_len);
    if (crypto_box_curve25519xchacha20poly1305_seal(c, m, m_len, pk) != 0) {
        printf("crypto_box_curve25519xchacha20poly1305_seal() failure\n");
        return;
    }
    if (crypto_box_curve25519xchacha20poly1305_seal_open(m2, c, c_len, pk, sk) != 0) {
        printf("crypto_box_curve25519xchacha20poly1305_seal_open() failure\n");
        return;
    }
    printf("%d\n", memcmp(m, m2, m_len));

    printf("%d\n", crypto_box_curve25519xchacha20poly1305_seal_open(m, c, 0U, pk, sk));
    printf("%d\n", crypto_box_curve25519xchacha20poly1305_seal_open(m, c, c_len - 1U, pk, sk));
    printf("%d\n", crypto_box_curve25519xchacha20poly1305_seal_open(m, c, c_len, sk, pk));

    sodium_free(c);
    sodium_free(m);
    sodium_free(m2);

    assert(crypto_box_curve25519xchacha20poly1305_sealbytes() ==
           crypto_box_curve25519xchacha20poly1305_SEALBYTES);
}

static
void tv4(void)
{
    unsigned char  pk[crypto_box_curve25519xchacha20poly1305_PUBLICKEYBYTES];
    unsigned char  sk[crypto_box_curve25519xchacha20poly1305_SECRETKEYBYTES];
    unsigned char *cm;
    unsigned char *m2;
    size_t         m_len;
    size_t         cm_len;

    crypto_box_curve25519xchacha20poly1305_keypair(pk, sk);
    m_len = (size_t) randombytes_uniform(1000);
    cm_len = crypto_box_curve25519xchacha20poly1305_SEALBYTES + m_len;
    m2    = (unsigned char *) sodium_malloc(m_len);
    cm    = (unsigned char *) sodium_malloc(cm_len);
    randombytes_buf(cm, m_len);
    if (crypto_box_curve25519xchacha20poly1305_seal(cm, cm, m_len, pk) != 0) {
        printf("crypto_box_curve25519xchacha20poly1305_seal() failure\n");
        return;
    }
    if (crypto_box_curve25519xchacha20poly1305_seal_open(m2, cm, cm_len, pk, sk) != 0) {
        printf("crypto_box_curve25519xchacha20poly1305_seal_open() failure\n");
        return;
    }
    assert(m_len == 0 || memcmp(cm, m2, m_len) != 0);
    sodium_free(cm);
    sodium_free(m2);
}

#else

static
void tv3(void)
{
    printf("0\n-1\n-1\n-1\n");
}

static
void tv4(void)
{ }
#endif

int
main(void)
{
    tv1();
    tv2();
    tv3();
    tv4();

    return 0;
}
