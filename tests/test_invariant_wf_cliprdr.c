#include <check.h>
#include <stdlib.h>

#include "../libs/clipboard/src/windows/wf_cliprdr.c"

Suite *security_suite(void)
{
	Suite *s;
	TCase *tc_core;

	s = suite_create("Security_ClipboardStreamAlloc");
	tc_core = tcase_create("Core");

	tcase_add_test(tc_core, test_stream_count_within_limit_accepts_boundary_values);
	tcase_add_test(tc_core, test_stream_count_within_limit_rejects_over_limit_value);

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
