
#include <stdlib.h>

/* C++Builder defines a "random" macro */
#undef random

#ifdef __EMSCRIPTEN__
# define strcmp(s1, s2) xstrcmp(s1, s2)

static int
strcmp(const char *s1, const char *s2)
{
    while (*s1 == *s2++) {
        if (*s1++ == 0) {
            return 0;
        }
    }
    return *(unsigned char *) s1 - *(unsigned char *) --s2;
}
#endif

#ifdef _WIN32
static void
srandom(unsigned seed)
{
    srand(seed);
}

static long
random(void)
{
    return (long) rand();
}
#endif
