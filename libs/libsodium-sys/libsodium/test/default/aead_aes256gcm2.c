
#define TEST_NAME "aead_aes256gcm2"
#include "cmptest.h"

static struct {
    const char *key_hex;
    const char  nonce_hex[crypto_aead_aes256gcm_NPUBBYTES * 2 + 1];
    const char *ad_hex;
    const char *message_hex;
    const char *detached_ciphertext_hex;
    const char  mac_hex[crypto_aead_aes256gcm_ABYTES * 2 + 1];
    const char *outcome;
} tests[] = {
    { "92ace3e348cd821092cd921aa3546374299ab46209691bc28b8752d17f123c20",
      "00112233445566778899aabb", "00000000ffffffff", "00010203040506070809",
      "e27abdd2d2a53d2f136b", "9a4a2579529301bcfb71c78d4060f52c", "valid" },
    { "29d3a44f8723dc640239100c365423a312934ac80239212ac3df3421a2098123",
      "00112233445566778899aabb", "aabbccddeeff", "", "",
      "2a7d77fa526b8250cb296078926b5020", "valid" },
    { "cc56b680552eb75008f5484b4cb803fa5063ebd6eab91f6ab6aef4916a766273",
      "99e23ec48985bccdeeab60f1", "", "2a", "06",
      "633c1e9703ef744ffffb40edf9d14355", "valid" },
    { "51e4bf2bad92b7aff1a4bc05550ba81df4b96fabf41c12c7b00e60e48db7e152",
      "4f07afedfdc3b6c2361823d3", "", "be3308f72a2c6aed", "cf332a12fdee800b",
      "602e8d7c4799d62c140c9bb834876b09", "valid" },
    { "67119627bd988eda906219e08c0d0d779a07d208ce8a4fe0709af755eeec6dcb",
      "68ab7fdbf61901dad461d23c", "", "51f8c1f731ea14acdb210a6d973e07",
      "43fc101bff4b32bfadd3daf57a590e", "ec04aacb7148a8b8be44cb7eaf4efa69",
      "valid" },
    { "59d4eafb4de0cfc7d3db99a8f54b15d7b39f0acc8da69763b019c1699f87674a",
      "2fcb1b38a99e71b84740ad9b", "", "549b365af913f3b081131ccb6b825588",
      "f58c16690122d75356907fd96b570fca", "28752c20153092818faba2a334640d6e",
      "valid" },
    { "3b2458d8176e1621c0cc24c0c0e24c1e80d72f7ee9149a4b166176629616d011",
      "45aaa3e5d16d2d42dc03445d", "", "3ff1514b1c503915918f0c0c31094a6e1f",
      "73a6b6f45f6ccc5131e07f2caa1f2e2f56", "2d7379ec1db5952d4e95d30c340b1b1d",
      "valid" },
    { "0212a8de5007ed87b33f1a7090b6114f9e08cefd9607f2c276bdcfdbc5ce9cd7",
      "e6b1adf2fd58a8762c65f31b", "",
      "10f1ecf9c60584665d9ae5efe279e7f7377eea6916d2b111",
      "0843fff52d934fc7a071ea62c0bd351ce85678cde3ea2c9e",
      "7355fde599006715053813ce696237a8", "valid" },
    { "b279f57e19c8f53f2f963f5f2519fdb7c1779be2ca2b3ae8e1128b7d6c627fc4",
      "98bc2c7438d5cd7665d76f6e", "c0",
      "fcc515b294408c8645c9183e3f4ecee5127846d1",
      "eb5500e3825952866d911253f8de860c00831c81",
      "ecb660e1fb0541ec41e8d68a64141b3a", "valid" },
    { "cdccfe3f46d782ef47df4e72f0c02d9c7f774def970d23486f11a57f54247f17",
      "376187894605a8d45e30de51", "956846a209e087ed",
      "e28e0e9f9d22463ac0e42639b530f42102fded75",
      "feca44952447015b5df1f456df8ca4bb4eee2ce2",
      "082e91924deeb77880e1b1c84f9b8d30", "valid" },
    { "f32364b1d339d82e4f132d8f4a0ec1ff7e746517fa07ef1a7f422f4e25a48194",
      "5a86a50a0e8a179c734b996d", "ab2ac7c44c60bdf8228c7884adb20184",
      "43891bccb522b1e72a6b53cf31c074e9d6c2df8e",
      "43dda832e942e286da314daa99bef5071d9d2c78",
      "c3922583476ced575404ddb85dd8cd44", "valid" },
    { "ff0089ee870a4a39f645b0a5da774f7a5911e9696fc9cad646452c2aa8595a12",
      "bc2a7757d0ce2d8b1f14ccd9",
      "972ab4e06390caae8f99dd6e2187be6c7ff2c08a24be16ef",
      "748b28031621d95ee61812b4b4f47d04c6fc2ff3",
      "a929ee7e67c7a2f91bbcec6389a3caf43ab49305",
      "ebec6774b955e789591c822dab739e12", "valid" },
    { "00112233445566778899aabbccddeeff102132435465768798a9bacbdcedfe0f",
      "000000000000000000000000", "", "561008fa07a68f5c61285cd013464eaf",
      "23293e9b07ca7d1b0cae7cc489a973b3", "ffffffffffffffffffffffffffffffff",
      "valid" },
    { "00112233445566778899aabbccddeeff102132435465768798a9bacbdcedfe0f",
      "ffffffffffffffffffffffff", "", "c6152244cea1978d3e0bc274cf8c0b3b",
      "7cb6fc7c6abc009efe9551a99f36a421", "00000000000000000000000000000000",
      "valid" },
    { "000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f",
      "505152535455565758595a5b", "", "202122232425262728292a2b2c2d2e2f",
      "b2061457c0759fc1749f174ee1ccadfa", "9de8fef6d8ab1bf1bf887232eab590dd",
      "invalid" },
    { "000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f",
      "505152535455565758595a5b", "", "202122232425262728292a2b2c2d2e2f",
      "b2061457c0759fc1749f174ee1ccadfa", "9ee8fef6d8ab1bf1bf887232eab590dd",
      "invalid" },
    { "000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f",
      "505152535455565758595a5b", "", "202122232425262728292a2b2c2d2e2f",
      "b2061457c0759fc1749f174ee1ccadfa", "1ce8fef6d8ab1bf1bf887232eab590dd",
      "invalid" },
    { "000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f",
      "505152535455565758595a5b", "", "202122232425262728292a2b2c2d2e2f",
      "b2061457c0759fc1749f174ee1ccadfa", "9ce9fef6d8ab1bf1bf887232eab590dd",
      "invalid" },
    { "000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f",
      "505152535455565758595a5b", "", "202122232425262728292a2b2c2d2e2f",
      "b2061457c0759fc1749f174ee1ccadfa", "9ce8fe76d8ab1bf1bf887232eab590dd",
      "invalid" },
    { "000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f",
      "505152535455565758595a5b", "", "202122232425262728292a2b2c2d2e2f",
      "b2061457c0759fc1749f174ee1ccadfa", "9ce8fef6d9ab1bf1bf887232eab590dd",
      "invalid" },
    { "000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f",
      "505152535455565758595a5b", "", "202122232425262728292a2b2c2d2e2f",
      "b2061457c0759fc1749f174ee1ccadfa", "9ce8fef6daab1bf1bf887232eab590dd",
      "invalid" },
    { "000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f",
      "505152535455565758595a5b", "", "202122232425262728292a2b2c2d2e2f",
      "b2061457c0759fc1749f174ee1ccadfa", "9ce8fef6d8ab1b71bf887232eab590dd",
      "invalid" },
    { "000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f",
      "505152535455565758595a5b", "", "202122232425262728292a2b2c2d2e2f",
      "b2061457c0759fc1749f174ee1ccadfa", "9ce8fef6d8ab1bf1be887232eab590dd",
      "invalid" },
    { "000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f",
      "505152535455565758595a5b", "", "202122232425262728292a2b2c2d2e2f",
      "b2061457c0759fc1749f174ee1ccadfa", "9ce8fef6d8ab1bf13f887232eab590dd",
      "invalid" },
    { "000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f",
      "505152535455565758595a5b", "", "202122232425262728292a2b2c2d2e2f",
      "b2061457c0759fc1749f174ee1ccadfa", "9ce8fef6d8ab1bf1bfa87232eab590dd",
      "invalid" },
    { "000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f",
      "505152535455565758595a5b", "", "202122232425262728292a2b2c2d2e2f",
      "b2061457c0759fc1749f174ee1ccadfa", "9ce8fef6d8ab1bf1bf887332eab590dd",
      "invalid" },
    { "000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f",
      "505152535455565758595a5b", "", "202122232425262728292a2b2c2d2e2f",
      "b2061457c0759fc1749f174ee1ccadfa", "9ce8fef6d8ab1bf1bf887232ebb590dd",
      "invalid" },
    { "000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f",
      "505152535455565758595a5b", "", "202122232425262728292a2b2c2d2e2f",
      "b2061457c0759fc1749f174ee1ccadfa", "9ce8fef6d8ab1bf1bf887232e8b590dd",
      "invalid" },
    { "000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f",
      "505152535455565758595a5b", "", "202122232425262728292a2b2c2d2e2f",
      "b2061457c0759fc1749f174ee1ccadfa", "9ce8fef6d8ab1bf1bf8872326ab590dd",
      "invalid" },
    { "000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f",
      "505152535455565758595a5b", "", "202122232425262728292a2b2c2d2e2f",
      "b2061457c0759fc1749f174ee1ccadfa", "9ce8fef6d8ab1bf1bf887232eab590dc",
      "invalid" },
    { "000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f",
      "505152535455565758595a5b", "", "202122232425262728292a2b2c2d2e2f",
      "b2061457c0759fc1749f174ee1ccadfa", "9ce8fef6d8ab1bf1bf887232eab590df",
      "invalid" },
    { "000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f",
      "505152535455565758595a5b", "", "202122232425262728292a2b2c2d2e2f",
      "b2061457c0759fc1749f174ee1ccadfa", "9ce8fef6d8ab1bf1bf887232eab5909d",
      "invalid" },
    { "000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f",
      "505152535455565758595a5b", "", "202122232425262728292a2b2c2d2e2f",
      "b2061457c0759fc1749f174ee1ccadfa", "9ce8fef6d8ab1bf1bf887232eab5905d",
      "invalid" },
    { "000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f",
      "505152535455565758595a5b", "", "202122232425262728292a2b2c2d2e2f",
      "b2061457c0759fc1749f174ee1ccadfa", "9de8fef6d8ab1bf1be887232eab590dd",
      "invalid" },
    { "000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f",
      "505152535455565758595a5b", "", "202122232425262728292a2b2c2d2e2f",
      "b2061457c0759fc1749f174ee1ccadfa", "9ce8fe76d8ab1b71bf887232eab590dd",
      "invalid" },
    { "000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f",
      "505152535455565758595a5b", "", "202122232425262728292a2b2c2d2e2f",
      "b2061457c0759fc1749f174ee1ccadfa", "9ce8fef6d8ab1b71bf887232eab5905d",
      "invalid" },
    { "000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f",
      "505152535455565758595a5b", "", "202122232425262728292a2b2c2d2e2f",
      "b2061457c0759fc1749f174ee1ccadfa", "631701092754e40e40778dcd154a6f22",
      "invalid" },
    { "000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f",
      "505152535455565758595a5b", "", "202122232425262728292a2b2c2d2e2f",
      "b2061457c0759fc1749f174ee1ccadfa", "00000000000000000000000000000000",
      "invalid" },
    { "000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f",
      "505152535455565758595a5b", "", "202122232425262728292a2b2c2d2e2f",
      "b2061457c0759fc1749f174ee1ccadfa", "ffffffffffffffffffffffffffffffff",
      "invalid" },
    { "000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f",
      "505152535455565758595a5b", "", "202122232425262728292a2b2c2d2e2f",
      "b2061457c0759fc1749f174ee1ccadfa", "1c687e76582b9b713f08f2b26a35105d",
      "invalid" },
    { "000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f",
      "505152535455565758595a5b", "", "202122232425262728292a2b2c2d2e2f",
      "b2061457c0759fc1749f174ee1ccadfa", "9de9fff7d9aa1af0be897333ebb491dc",
      "invalid" }
};

static int
tv(void)
{
    unsigned char *ad;
    unsigned char *decrypted;
    unsigned char *detached_ciphertext;
    unsigned char *key;
    unsigned char *message;
    unsigned char *mac;
    unsigned char *nonce;
    size_t         ad_len;
    size_t         detached_ciphertext_len;
    size_t         message_len;
    unsigned int   i;

    key   = (unsigned char *) sodium_malloc(crypto_aead_aes256gcm_KEYBYTES);
    nonce = (unsigned char *) sodium_malloc(crypto_aead_aes256gcm_NPUBBYTES);
    mac   = (unsigned char *) sodium_malloc(crypto_aead_aes256gcm_ABYTES);

    for (i = 0U; i < (sizeof tests) / (sizeof tests[0]); i++) {
        assert(strlen(tests[i].key_hex) == 2 * crypto_aead_aes256gcm_KEYBYTES);
        sodium_hex2bin(key, crypto_aead_aes256gcm_KEYBYTES, tests[i].key_hex,
                       strlen(tests[i].key_hex), NULL, NULL, NULL);

        assert(strlen(tests[i].nonce_hex) ==
               2 * crypto_aead_aes256gcm_NPUBBYTES);
        sodium_hex2bin(nonce, crypto_aead_aes256gcm_NPUBBYTES,
                       tests[i].nonce_hex, strlen(tests[i].nonce_hex), NULL,
                       NULL, NULL);

        message_len = strlen(tests[i].message_hex) / 2;
        message     = (unsigned char *) sodium_malloc(message_len);
        sodium_hex2bin(message, message_len, tests[i].message_hex,
                       strlen(tests[i].message_hex), NULL, NULL, NULL);

        ad_len = strlen(tests[i].ad_hex) / 2;
        ad     = (unsigned char *) sodium_malloc(ad_len);
        sodium_hex2bin(ad, ad_len, tests[i].ad_hex, strlen(tests[i].ad_hex),
                       NULL, NULL, NULL);

        detached_ciphertext_len = message_len;
        assert(strlen(tests[i].detached_ciphertext_hex) == 2 * message_len);
        assert(strlen(tests[i].mac_hex) == 2 * crypto_aead_aes256gcm_ABYTES);
        sodium_hex2bin(mac, crypto_aead_aes256gcm_ABYTES, tests[i].mac_hex,
                       strlen(tests[i].mac_hex), NULL, NULL, NULL);

        detached_ciphertext =
            (unsigned char *) sodium_malloc(detached_ciphertext_len);
        sodium_hex2bin(detached_ciphertext, detached_ciphertext_len,
                       tests[i].detached_ciphertext_hex,
                       strlen(tests[i].detached_ciphertext_hex), NULL, NULL,
                       NULL);

        decrypted = (unsigned char *) sodium_malloc(message_len);
        if (crypto_aead_aes256gcm_decrypt_detached(
                decrypted, NULL, detached_ciphertext, detached_ciphertext_len,
                mac, ad, ad_len, nonce, key) == 0) {
            if (strcmp(tests[i].outcome, "valid") != 0) {
                printf("*** test case %u succeeded, was supposed to be %s\n", i,
                       tests[i].outcome);
            }
            if (memcmp(decrypted, message, message_len) != 0) {
                printf("Incorrect decryption of test vector #%u\n",
                       (unsigned int) i);
            }
        } else {
            if (strcmp(tests[i].outcome, "invalid") != 0) {
                printf("*** test case %u failed, was supposed to be %s\n", i,
                       tests[i].outcome);
            }
        }

        sodium_free(message);
        sodium_free(ad);
        sodium_free(decrypted);
        sodium_free(detached_ciphertext);
    }

    sodium_free(key);
    sodium_free(mac);
    sodium_free(nonce);

    return 0;
}

int
main(void)
{
    if (crypto_aead_aes256gcm_is_available()) {
        tv();
    }
    printf("OK\n");

    return 0;
}
