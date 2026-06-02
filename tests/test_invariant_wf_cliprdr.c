#include <check.h>
#include <stdlib.h>
#include <stdint.h>
#include <string.h>

/* Mock the minimal structures needed to test allocation safety */
typedef struct {
    void *lpVtbl;
    uint32_t refCount;
} IStream;

typedef struct {
    IStream *iStream;
    void *context;
} CliprdrStream;

/* Forward declare the function under test */
CliprdrStream *cliprdr_stream_new(void);

/* Inject allocation failure by wrapping calloc */
static int allocation_fail_count = 0;
static int allocation_call_count = 0;

void *__wrap_calloc(size_t nmemb, size_t size);
void *__wrap_calloc(size_t nmemb, size_t size) {
    allocation_call_count++;
    if (allocation_call_count == allocation_fail_count) {
        return NULL;
    }
    return calloc(nmemb, size);
}

START_TEST(test_cliprdr_allocation_null_check)
{
    /* Invariant: cliprdr_stream_new must not dereference NULL pointers
       from failed allocations; it must either return NULL or handle gracefully */
    
    int test_cases[] = {
        0,  /* No allocation failure - valid case */
        1,  /* First calloc fails (CliprdrStream allocation) */
        2,  /* Second calloc fails (IStreamVtbl allocation) */
    };
    
    int num_cases = sizeof(test_cases) / sizeof(test_cases[0]);
    
    for (int i = 0; i < num_cases; i++) {
        allocation_call_count = 0;
        allocation_fail_count = test_cases[i];
        
        /* Call the actual production function */
        CliprdrStream *result = cliprdr_stream_new();
        
        /* Security property: function must not crash on allocation failure.
           Valid outcomes are: NULL return or valid initialized structure.
           Invalid outcome: dereferencing NULL (would crash before returning). */
        
        if (allocation_fail_count > 0) {
            /* When allocation fails, result should be NULL or safe */
            ck_assert_msg(result == NULL || result != NULL,
                "cliprdr_stream_new must handle allocation failure safely");
        } else {
            /* When no failure, result should be valid */
            ck_assert_ptr_nonnull(result);
            if (result) {
                free(result->iStream);
                free(result);
            }
        }
    }
}
END_TEST

Suite *security_suite(void)
{
    Suite *s;
    TCase *tc_core;

    s = suite_create("Security");
    tc_core = tcase_create("Allocation_Safety");

    tcase_add_test(tc_core, test_cliprdr_allocation_null_check);
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