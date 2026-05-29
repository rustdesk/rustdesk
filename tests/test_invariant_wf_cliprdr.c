#include <check.h>
#include <stdlib.h>
#include <stdint.h>
#include <string.h>

/*
 * Security invariant:
 * When allocating memory for a stream array using calloc(count, sizeof(LPSTREAM)),
 * the count value MUST be validated before use. Specifically:
 * 1. count * sizeof(element) must not overflow SIZE_MAX
 * 2. count must be within a reasonable bound to prevent excessive allocation
 * 3. If count is invalid/dangerous, allocation must either fail safely or be rejected
 *
 * This test simulates the vulnerable pattern from wf_cliprdr.c line 774 and
 * verifies that a safe wrapper enforces bounds before calling calloc.
 */

/* Simulate sizeof(LPSTREAM) - pointer size on the platform */
#define LPSTREAM_SIZE (sizeof(void *))

/* Maximum allowed stream count - a reasonable upper bound for clipboard streams */
#define MAX_SAFE_STREAM_COUNT 1024

/*
 * Safe allocation function that mirrors what the vulnerable code SHOULD do.
 * Returns NULL if the count is invalid or would cause overflow.
 * This encodes the security invariant: bounds must be checked before allocation.
 */
static void **safe_alloc_streams(size_t count)
{
    /* Invariant: count must be within safe bounds */
    if (count == 0) {
        return NULL;
    }

    /* Invariant: count must not exceed reasonable maximum */
    if (count > MAX_SAFE_STREAM_COUNT) {
        return NULL;
    }

    /* Invariant: multiplication must not overflow */
    if (count > SIZE_MAX / LPSTREAM_SIZE) {
        return NULL;
    }

    void **result = (void **)calloc(count, LPSTREAM_SIZE);
    return result;
}

/*
 * Helper: check if a given count value would cause integer overflow
 * when multiplied by LPSTREAM_SIZE.
 */
static int would_overflow(size_t count)
{
    if (count == 0) return 0;
    return (count > SIZE_MAX / LPSTREAM_SIZE);
}

START_TEST(test_stream_alloc_bounds_validation)
{
    /* Invariant: adversarial stream counts must never result in under-allocated buffers
     * that could be written beyond their bounds. */

    /* Adversarial count values derived from remote clipboard data */
    size_t adversarial_counts[] = {
        /* Integer overflow candidates */
        SIZE_MAX,
        SIZE_MAX / 2,
        SIZE_MAX / LPSTREAM_SIZE,
        SIZE_MAX / LPSTREAM_SIZE + 1,
        (SIZE_MAX / LPSTREAM_SIZE) + 2,
        /* Large values that exceed reasonable clipboard stream counts */
        0xFFFFFFFF,
        0x7FFFFFFF,
        0x80000000,
        0xFFFFFFFE,
        /* Values near typical int/size_t boundaries */
        (size_t)INT32_MAX,
        (size_t)INT32_MAX + 1,
        (size_t)UINT32_MAX,
        /* Extremely large values */
        1ULL << 32,
        1ULL << 48,
        1ULL << 62,
        /* Values just above the safe maximum */
        MAX_SAFE_STREAM_COUNT + 1,
        MAX_SAFE_STREAM_COUNT * 2,
        MAX_SAFE_STREAM_COUNT * 1000,
    };

    int num_counts = sizeof(adversarial_counts) / sizeof(adversarial_counts[0]);

    for (int i = 0; i < num_counts; i++) {
        size_t count = adversarial_counts[i];

        /* INVARIANT: For any adversarial count, safe_alloc_streams must return NULL
         * (reject the allocation) rather than return an under-sized buffer */
        void **result = safe_alloc_streams(count);

        /* All adversarial counts exceed MAX_SAFE_STREAM_COUNT or would overflow,
         * so the result MUST be NULL */
        ck_assert_msg(result == NULL,
            "SECURITY VIOLATION: safe_alloc_streams(%zu) returned non-NULL "
            "for adversarial input - allocation should have been rejected",
            count);

        /* Ensure no memory leak if somehow allocation succeeded */
        if (result != NULL) {
            free(result);
        }
    }
}
END_TEST

START_TEST(test_overflow_detection)
{
    /* Invariant: overflow detection must correctly identify dangerous count values */

    struct {
        size_t count;
        int expect_overflow;
    } test_cases[] = {
        /* Safe values - should NOT overflow */
        { 1,    0 },
        { 10,   0 },
        { 100,  0 },
        { 1024, 0 },
        /* Dangerous values - MUST be detected as overflow */
        { SIZE_MAX,                         1 },
        { SIZE_MAX / LPSTREAM_SIZE + 1,     1 },
        { SIZE_MAX / 2,                     1 },
    };

    int num_cases = sizeof(test_cases) / sizeof(test_cases[0]);

    for (int i = 0; i < num_cases; i++) {
        size_t count = test_cases[i].count;
        int expected = test_cases[i].expect_overflow;
        int actual = would_overflow(count);

        ck_assert_msg(actual == expected,
            "SECURITY VIOLATION: overflow detection for count=%zu returned %d, "
            "expected %d - overflow detection is broken",
            count, actual, expected);
    }
}
END_TEST

START_TEST(test_valid_stream_counts_succeed)
{
    /* Invariant: valid, small stream counts must succeed to ensure functionality */

    size_t valid_counts[] = { 1, 2, 4, 8, 16, 32, 64, 128, 256, MAX_SAFE_STREAM_COUNT };
    int num_counts = sizeof(valid_counts) / sizeof(valid_counts[0]);

    for (int i = 0; i < num_counts; i++) {
        size_t count = valid_counts[i];

        void **result = safe_alloc_streams(count);

        /* INVARIANT: valid counts within bounds must succeed */
        ck_assert_msg(result != NULL,
            "FUNCTIONALITY VIOLATION: safe_alloc_streams(%zu) returned NULL "
            "for a valid count - legitimate allocations must succeed",
            count);

        /* INVARIANT: allocated memory must be zero-initialized (calloc guarantee) */
        for (size_t j = 0; j < count; j++) {
            ck_assert_msg(result[j] == NULL,
                "INVARIANT VIOLATION: calloc'd memory at index %zu is not zero-initialized "
                "for count=%zu",
                j, count);
        }

        free(result);
    }
}
END_TEST

START_TEST(test_zero_count_rejected)
{
    /* Invariant: zero stream count must be rejected as it indicates invalid data */
    void **result = safe_alloc_streams(0);

    ck_assert_msg(result == NULL,
        "INVARIANT VIOLATION: safe_alloc_streams(0) should return NULL "
        "for zero count (invalid clipboard data)");
}
END_TEST

START_TEST(test_boundary_values)
{
    /* Invariant: values at the exact boundary must be handled correctly */

    /* Exactly at the safe maximum - must succeed */
    void **result_at_max = safe_alloc_streams(MAX_SAFE_STREAM_COUNT);
    ck_assert_msg(result_at_max != NULL,
        "INVARIANT VIOLATION: allocation at MAX_SAFE_STREAM_COUNT (%d) must succeed",
        MAX_SAFE_STREAM_COUNT);
    free(result_at_max);

    /* One above the safe maximum - must fail */
    void **result_above_max = safe_alloc_streams(MAX_SAFE_STREAM_COUNT + 1);
    ck_assert_msg(result_above_max == NULL,
        "SECURITY VIOLATION: allocation at MAX_SAFE_STREAM_COUNT+1 (%d) must be rejected",
        MAX_SAFE_STREAM_COUNT + 1);
}
END_TEST

Suite *security_suite(void)
{
    Suite *s;
    TCase *tc_core;

    s = suite_create("Security_ClipboardStreamAlloc");
    tc_core = tcase_create("Core");

    tcase_add_test(tc_core, test_stream_alloc_bounds_validation);
    tcase_add_test(tc_core, test_overflow_detection);
    tcase_add_test(tc_core, test_valid_stream_counts_succeed);
    tcase_add_test(tc_core, test_zero_count_rejected);
    tcase_add_test(tc_core, test_boundary_values);

    suite_add_tcase(s, tc_core);

    return s;
}

int main(void)
{
    int number_failed;
    Suite *s;
    SRunner *sr;

    s = security_suite();
    sr = srunner_create(s);

    srunner_run_all(sr, CK_NORMAL);
    number_failed = srunner_ntests_failed(sr);
    srunner_free(sr);

    return (number_failed == 0) ? EXIT_SUCCESS : EXIT_FAILURE;
}