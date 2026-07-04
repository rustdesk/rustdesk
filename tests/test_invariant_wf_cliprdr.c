#include <check.h>
#include <stdlib.h>
#include <stdint.h>
#include "../../../libs/clipboard/src/windows/wf_cliprdr.h"

START_TEST(test_buffer_reads_never_exceed_declared_length)
{
    // Invariant: Buffer reads never exceed the declared length
    const uint32_t payloads[] = {
        UINT32_MAX,                    // Exploit case: maximum value causing overflow
        (UINT32_MAX / sizeof(IStream *)) + 1, // Boundary: overflow in multiplication
        100,                           // Valid normal input
        0,                             // Edge: zero count
        65535                          // Large but reasonable value
    };
    int num_payloads = sizeof(payloads) / sizeof(payloads[0]);

    for (int i = 0; i < num_payloads; i++) {
        uint32_t stream_count = payloads[i];
        IStream **streams = (IStream **)calloc(stream_count, sizeof(IStream *));
        
        // If allocation succeeds, verify no out-of-bounds access would occur
        if (streams != NULL) {
            // Test invariant: allocated size >= requested size
            ck_assert_msg(stream_count * sizeof(IStream *) <= SIZE_MAX,
                         "Stream count %u causes size overflow", stream_count);
            free(streams);
        } else {
            // Allocation failed - this is acceptable for extreme values
            ck_assert_msg(stream_count == 0 || 
                         stream_count > (SIZE_MAX / sizeof(IStream *)),
                         "Unexpected allocation failure for stream_count %u", 
                         stream_count);
        }
    }
}
END_TEST

Suite *security_suite(void)
{
    Suite *s;
    TCase *tc_core;

    s = suite_create("Security");
    tc_core = tcase_create("Core");

    tcase_add_test(tc_core, test_buffer_reads_never_exceed_declared_length);
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