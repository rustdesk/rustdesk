
EXTRA_DIST = \
	cmptest.h \
	wasi-test-wrapper.sh \
	wintest.bat \
	pre.js.inc \
	aead_aes256gcm.exp \
	aead_aes256gcm2.exp \
	aead_chacha20poly1305.exp \
	aead_chacha20poly13052.exp \
	aead_xchacha20poly1305.exp \
	auth.exp \
	auth2.exp \
	auth3.exp \
	auth5.exp \
	auth6.exp \
	auth7.exp \
	box.exp \
	box2.exp \
	box7.exp \
	box8.exp \
	box_easy.exp \
	box_easy2.exp \
	box_seal.exp \
	box_seed.exp \
	chacha20.exp \
	codecs.exp \
	core_ed25519.exp \
	core_ristretto255.exp \
	core1.exp \
	core2.exp \
	core3.exp \
	core4.exp \
	core5.exp \
	core6.exp \
	ed25519_convert.exp \
	generichash.exp \
	generichash2.exp \
	generichash3.exp \
	hash.exp \
	hash2.exp \
	hash3.exp \
	kdf.exp \
	keygen.exp \
	kx.exp \
	metamorphic.exp \
	misuse.exp \
	onetimeauth.exp \
	onetimeauth2.exp \
	onetimeauth7.exp \
	pwhash_argon2i.exp \
	pwhash_argon2id.exp \
	pwhash_scrypt.exp \
	pwhash_scrypt_ll.exp \
	randombytes.exp \
	scalarmult.exp \
	scalarmult_ed25519.exp \
	scalarmult_ristretto255.exp \
	scalarmult2.exp \
	scalarmult5.exp \
	scalarmult6.exp \
	scalarmult7.exp \
	scalarmult8.exp \
	secretbox.exp \
	secretbox2.exp \
	secretbox7.exp \
	secretbox8.exp \
	secretbox_easy.exp \
	secretbox_easy2.exp \
	secretstream.exp \
	shorthash.exp \
	sign.exp \
	siphashx24.exp \
	sodium_core.exp \
	sodium_utils.exp \
	sodium_utils2.exp \
	sodium_utils3.exp \
	sodium_version.exp \
	stream.exp \
	stream2.exp \
	stream3.exp \
	stream4.exp \
	verify1.exp \
	xchacha20.exp

DISTCLEANFILES = \
	aead_aes256gcm.res \
	aead_aes256gcm2.res \
	aead_chacha20poly1305.res \
	aead_chacha20poly13052.res \
	aead_xchacha20poly1305.res \
	auth.res \
	auth2.res \
	auth3.res \
	auth5.res \
	auth6.res \
	auth7.res \
	box.res \
	box2.res \
	box7.res \
	box8.res \
	box_easy.res \
	box_easy2.res \
	box_seal.res \
	box_seed.res \
	chacha20.res \
	codecs.res \
	core_ed25519.res \
	core_ristretto255.res \
	core1.res \
	core2.res \
	core3.res \
	core4.res \
	core5.res \
	core6.res \
	ed25519_convert.res \
	generichash.res \
	generichash2.res \
	generichash3.res \
	hash.res \
	hash2.res \
	hash3.res \
	kdf.res \
	keygen.res \
	kx.res \
	metamorphic.res \
	misuse.res \
	onetimeauth.res \
	onetimeauth2.res \
	onetimeauth7.res \
	pwhash_argon2i.res \
	pwhash_argon2id.res \
	pwhash_scrypt.res \
	pwhash_scrypt_ll.res \
	randombytes.res \
	scalarmult.res \
	scalarmult_ed25519.res \
	scalarmult_ristretto255.res \
	scalarmult2.res \
	scalarmult5.res \
	scalarmult6.res \
	scalarmult7.res \
	scalarmult8.res \
	secretbox.res \
	secretbox2.res \
	secretbox7.res \
	secretbox8.res \
	secretbox_easy.res \
	secretbox_easy2.res \
	secretstream.res \
	shorthash.res \
	sign.res \
	siphashx24.res \
	sodium_core.res \
	sodium_utils.res \
	sodium_utils2.res \
	sodium_utils3.res \
	sodium_version.res \
	stream.res \
	stream2.res \
	stream3.res \
	stream4.res \
	verify1.res \
	xchacha20.res

AM_CPPFLAGS = \
	-DTEST_SRCDIR=\"@srcdir@\" \
	-I$(top_srcdir)/src/libsodium/include \
	-I$(top_srcdir)/src/libsodium/include/sodium \
	-I$(top_builddir)/src/libsodium/include \
	-I$(top_builddir)/src/libsodium/include/sodium \
	-I$(top_srcdir)/test/quirks

AM_LDFLAGS = @TEST_LDFLAGS@

TESTS_TARGETS = \
	aead_aes256gcm \
	aead_aes256gcm2 \
	aead_chacha20poly1305 \
	aead_chacha20poly13052 \
	aead_xchacha20poly1305 \
	auth \
	auth2 \
	auth3 \
	auth5 \
	auth6 \
	auth7 \
	box \
	box2 \
	box7 \
	box8 \
	box_easy \
	box_easy2 \
	box_seal \
	box_seed \
	chacha20 \
	codecs \
	core1 \
	core2 \
	core3 \
	core4 \
	core5 \
	core6 \
	ed25519_convert \
	generichash \
	generichash2 \
	generichash3 \
	hash \
	hash3 \
	kdf \
	keygen \
	kx \
	metamorphic \
	misuse \
	onetimeauth \
	onetimeauth2 \
	onetimeauth7 \
	pwhash_argon2i \
	pwhash_argon2id \
	randombytes \
	scalarmult \
	scalarmult2 \
	scalarmult5 \
	scalarmult6 \
	scalarmult7 \
	scalarmult8 \
	secretbox \
	secretbox2 \
	secretbox7 \
	secretbox8 \
	secretbox_easy \
	secretbox_easy2 \
	secretstream \
	shorthash \
	sign \
	sodium_core \
	sodium_utils \
	sodium_version \
	stream \
	stream2 \
	stream3 \
	stream4 \
	verify1

if !EMSCRIPTEN
TESTS_TARGETS += \
	sodium_utils2 \
	sodium_utils3
endif

check_PROGRAMS = $(TESTS_TARGETS)

TESTS = $(TESTS_TARGETS)

TESTS_LDADD = \
	${top_builddir}/src/libsodium/libsodium.la

aead_aes256gcm_SOURCE                 = cmptest.h aead_aes256gcm.c
aead_aes256gcm_LDADD                  = $(TESTS_LDADD)

aead_aes256gcm2_SOURCE                = cmptest.h aead_aes256gcm2.c
aead_aes256gcm2_LDADD                 = $(TESTS_LDADD)

aead_chacha20poly1305_SOURCE          = cmptest.h aead_chacha20poly1305.c
aead_chacha20poly1305_LDADD           = $(TESTS_LDADD)

aead_chacha20poly13052_SOURCE         = cmptest.h aead_chacha20poly13052.c
aead_chacha20poly13052_LDADD          = $(TESTS_LDADD)

aead_xchacha20poly1305_SOURCE         = cmptest.h aead_xchacha20poly1305.c
aead_xchacha20poly1305_LDADD          = $(TESTS_LDADD)

auth_SOURCE               = cmptest.h auth.c
auth_LDADD                = $(TESTS_LDADD)

auth2_SOURCE              = cmptest.h auth2.c
auth2_LDADD               = $(TESTS_LDADD)

auth3_SOURCE              = cmptest.h auth3.c
auth3_LDADD               = $(TESTS_LDADD)

auth5_SOURCE              = cmptest.h auth5.c quirks.h
auth5_LDADD               = $(TESTS_LDADD)

auth6_SOURCE              = cmptest.h auth6.c quirks.h
auth6_LDADD               = $(TESTS_LDADD)

auth7_SOURCE              = cmptest.h auth7.c quirks.h
auth7_LDADD               = $(TESTS_LDADD)

box_SOURCE                = cmptest.h box.c
box_LDADD                 = $(TESTS_LDADD)

box2_SOURCE               = cmptest.h box2.c
box2_LDADD                = $(TESTS_LDADD)

box7_SOURCE               = cmptest.h box7.c
box7_LDADD                = $(TESTS_LDADD)

box8_SOURCE               = cmptest.h box8.c
box8_LDADD                = $(TESTS_LDADD)

box_easy_SOURCE           = cmptest.h box_easy.c
box_easy_LDADD            = $(TESTS_LDADD)

box_easy2_SOURCE          = cmptest.h box_easy2.c
box_easy2_LDADD           = $(TESTS_LDADD)

box_seal_SOURCE           = cmptest.h box_seal.c
box_seal_LDADD            = $(TESTS_LDADD)

box_seed_SOURCE           = cmptest.h box_seed.c
box_seed_LDADD            = $(TESTS_LDADD)

chacha20_SOURCE           = cmptest.h chacha20.c
chacha20_LDADD            = $(TESTS_LDADD)

codecs_SOURCE             = cmptest.h codecs.c
codecs_LDADD              = $(TESTS_LDADD)

core_ed25519_SOURCE       = cmptest.h core_ed25519.c
core_ed25519_LDADD        = $(TESTS_LDADD)

core_ristretto255_SOURCE  = cmptest.h core_ristretto255.c
core_ristretto255_LDADD   = $(TESTS_LDADD)

core1_SOURCE              = cmptest.h core1.c
core1_LDADD               = $(TESTS_LDADD)

core2_SOURCE              = cmptest.h core2.c
core2_LDADD               = $(TESTS_LDADD)

core3_SOURCE              = cmptest.h core3.c
core3_LDADD               = $(TESTS_LDADD)

core4_SOURCE              = cmptest.h core4.c
core4_LDADD               = $(TESTS_LDADD)

core5_SOURCE              = cmptest.h core5.c
core5_LDADD               = $(TESTS_LDADD)

core6_SOURCE              = cmptest.h core6.c
core6_LDADD               = $(TESTS_LDADD)

ed25519_convert_SOURCE    = cmptest.h ed25519_convert.c
ed25519_convert_LDADD     = $(TESTS_LDADD)

generichash_SOURCE        = cmptest.h generichash.c
generichash_LDADD         = $(TESTS_LDADD)

generichash2_SOURCE       = cmptest.h generichash2.c
generichash2_LDADD        = $(TESTS_LDADD)

generichash3_SOURCE       = cmptest.h generichash3.c
generichash3_LDADD        = $(TESTS_LDADD)

hash_SOURCE               = cmptest.h hash.c
hash_LDADD                = $(TESTS_LDADD)

hash3_SOURCE              = cmptest.h hash3.c
hash3_LDADD               = $(TESTS_LDADD)

kdf_SOURCE                = cmptest.h kdf.c
kdf_LDADD                 = $(TESTS_LDADD)

keygen_SOURCE             = cmptest.h keygen.c
keygen_LDADD              = $(TESTS_LDADD)

kx_SOURCE                 = cmptest.h kx.c
kx_LDADD                  = $(TESTS_LDADD)

metamorphic_SOURCE        = cmptest.h metamorphic.c
metamorphic_LDADD         = $(TESTS_LDADD)

misuse_SOURCE             = cmptest.h misuse.c
misuse_LDADD              = $(TESTS_LDADD)

onetimeauth_SOURCE        = cmptest.h onetimeauth.c
onetimeauth_LDADD         = $(TESTS_LDADD)

onetimeauth2_SOURCE       = cmptest.h onetimeauth2.c
onetimeauth2_LDADD        = $(TESTS_LDADD)

onetimeauth7_SOURCE       = cmptest.h onetimeauth7.c
onetimeauth7_LDADD        = $(TESTS_LDADD)

pwhash_argon2i_SOURCE     = cmptest.h pwhash_argon2i.c
pwhash_argon2i_LDADD      = $(TESTS_LDADD)

pwhash_argon2id_SOURCE    = cmptest.h pwhash_argon2id.c
pwhash_argon2id_LDADD     = $(TESTS_LDADD)

pwhash_scrypt_SOURCE      = cmptest.h pwhash_scrypt.c
pwhash_scrypt_LDADD       = $(TESTS_LDADD)

pwhash_scrypt_ll_SOURCE   = cmptest.h pwhash_scrypt_ll.c
pwhash_scrypt_ll_LDADD    = $(TESTS_LDADD)

randombytes_SOURCE        = cmptest.h randombytes.c
randombytes_LDADD         = $(TESTS_LDADD)

scalarmult_SOURCE         = cmptest.h scalarmult.c
scalarmult_LDADD          = $(TESTS_LDADD)

scalarmult_ed25519_SOURCE = cmptest.h scalarmult_ed25519.c
scalarmult_ed25519_LDADD  = $(TESTS_LDADD)

scalarmult_ristretto255_SOURCE = cmptest.h scalarmult_ristretto255.c
scalarmult_ristretto255_LDADD  = $(TESTS_LDADD)

scalarmult2_SOURCE        = cmptest.h scalarmult2.c
scalarmult2_LDADD         = $(TESTS_LDADD)

scalarmult5_SOURCE        = cmptest.h scalarmult5.c
scalarmult5_LDADD         = $(TESTS_LDADD)

scalarmult6_SOURCE        = cmptest.h scalarmult6.c
scalarmult6_LDADD         = $(TESTS_LDADD)

scalarmult7_SOURCE        = cmptest.h scalarmult7.c
scalarmult7_LDADD         = $(TESTS_LDADD)

scalarmult8_SOURCE        = cmptest.h scalarmult8.c
scalarmult8_LDADD         = $(TESTS_LDADD)

secretbox_SOURCE          = cmptest.h secretbox.c
secretbox_LDADD           = $(TESTS_LDADD)

secretbox2_SOURCE         = cmptest.h secretbox2.c
secretbox2_LDADD          = $(TESTS_LDADD)

secretbox7_SOURCE         = cmptest.h secretbox7.c
secretbox7_LDADD          = $(TESTS_LDADD)

secretbox8_SOURCE         = cmptest.h secretbox8.c
secretbox8_LDADD          = $(TESTS_LDADD)

secretbox_easy_SOURCE     = cmptest.h secretbox_easy.c
secretbox_easy_LDADD      = $(TESTS_LDADD)

secretbox_easy2_SOURCE    = cmptest.h secretbox_easy2.c
secretbox_easy2_LDADD     = $(TESTS_LDADD)

secretstream_SOURCE       = cmptest.h secretstream.c
secretstream_LDADD        = $(TESTS_LDADD)

shorthash_SOURCE          = cmptest.h shorthash.c
shorthash_LDADD           = $(TESTS_LDADD)

sign_SOURCE               = cmptest.h sign.c
sign_LDADD                = $(TESTS_LDADD)

siphashx24_SOURCE         = cmptest.h siphashx24.c
siphashx24_LDADD          = $(TESTS_LDADD)

sodium_core_SOURCE        = cmptest.h sodium_core.c
sodium_core_LDADD         = $(TESTS_LDADD)

sodium_utils_SOURCE       = cmptest.h sodium_utils.c
sodium_utils_LDADD        = $(TESTS_LDADD)

sodium_utils2_SOURCE      = cmptest.h sodium_utils2.c
sodium_utils2_LDADD       = $(TESTS_LDADD)

sodium_utils3_SOURCE      = cmptest.h sodium_utils3.c
sodium_utils3_LDADD       = $(TESTS_LDADD)

sodium_version_SOURCE     = cmptest.h sodium_version.c
sodium_version_LDADD      = $(TESTS_LDADD)

stream_SOURCE             = cmptest.h stream.c
stream_LDADD              = $(TESTS_LDADD)

stream2_SOURCE            = cmptest.h stream2.c
stream2_LDADD             = $(TESTS_LDADD)

stream3_SOURCE            = cmptest.h stream3.c
stream3_LDADD             = $(TESTS_LDADD)

stream4_SOURCE            = cmptest.h stream4.c
stream4_LDADD             = $(TESTS_LDADD)

verify1_SOURCE            = cmptest.h verify1.c
verify1_LDADD             = $(TESTS_LDADD)

xchacha20_SOURCE          = cmptest.h xchacha20.c
xchacha20_LDADD           = $(TESTS_LDADD)

if !MINIMAL
TESTS_TARGETS += \
	core_ed25519 \
	core_ristretto255 \
	pwhash_scrypt \
	pwhash_scrypt_ll \
	scalarmult_ed25519 \
	scalarmult_ristretto255 \
	siphashx24 \
	xchacha20
endif

if WASI
LOG_COMPILER = ./wasi-test-wrapper.sh
endif

verify: check

@VALGRIND_CHECK_RULES@
