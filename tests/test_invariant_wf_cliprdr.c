#include <check.h>
#include <stdlib.h>

#include "../libs/clipboard/src/windows/wf_cliprdr.c"

START_TEST(test_stream_count_within_limit_accepts_boundary_values)
{
	ck_assert_msg(wf_cliprdr_stream_count_within_limit(WF_CLIPRDR_MAX_STREAMS - 1),
				  "stream count below the production limit should be accepted");
	ck_assert_msg(wf_cliprdr_stream_count_within_limit(WF_CLIPRDR_MAX_STREAMS),
				  "stream count at the production limit should be accepted");
}
END_TEST

START_TEST(test_stream_count_within_limit_rejects_over_limit_value)
{
	ck_assert_msg(!wf_cliprdr_stream_count_within_limit(WF_CLIPRDR_MAX_STREAMS + 1),
				  "stream count above the production limit should be rejected");
}
END_TEST

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
