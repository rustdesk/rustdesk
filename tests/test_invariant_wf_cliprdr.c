#include <check.h>
#include <stdlib.h>

#include "../libs/clipboard/src/windows/wf_cliprdr.c"

static SIZE_T descriptor_size(UINT count)
{
	return offsetof(FILEGROUPDESCRIPTORW, fgd) + (SIZE_T)count * sizeof(FILEDESCRIPTORW);
}

START_TEST(test_descriptor_size_rejects_buffer_smaller_than_header)
{
	ck_assert_int_eq(wf_cliprdr_file_group_descriptor_size_valid(0, 1), FALSE);
	ck_assert_int_eq(wf_cliprdr_file_group_descriptor_size_valid(
	                     offsetof(FILEGROUPDESCRIPTORW, fgd) - 1, 1),
	                 FALSE);
}
END_TEST

START_TEST(test_descriptor_size_rejects_zero_items)
{
	ck_assert_int_eq(
	    wf_cliprdr_file_group_descriptor_size_valid(offsetof(FILEGROUPDESCRIPTORW, fgd), 0),
	    FALSE);
}
END_TEST

START_TEST(test_descriptor_size_accepts_max_stream_count)
{
	ck_assert_int_eq(wf_cliprdr_file_group_descriptor_size_valid(
	                     descriptor_size(WF_CLIPRDR_MAX_STREAMS), WF_CLIPRDR_MAX_STREAMS),
	                 TRUE);
}
END_TEST

START_TEST(test_descriptor_size_rejects_stream_count_above_limit)
{
	ck_assert_int_eq(wf_cliprdr_file_group_descriptor_size_valid(
	                     descriptor_size(WF_CLIPRDR_MAX_STREAMS), WF_CLIPRDR_MAX_STREAMS + 1),
	                 FALSE);
}
END_TEST

START_TEST(test_descriptor_size_rejects_truncated_descriptor_array)
{
	ck_assert_int_eq(wf_cliprdr_file_group_descriptor_size_valid(descriptor_size(2) - 1, 2),
	                 FALSE);
}
END_TEST

START_TEST(test_descriptor_size_rejects_extreme_count)
{
	ck_assert_int_eq(wf_cliprdr_file_group_descriptor_size_valid((SIZE_T)-1, (UINT)-1),
	                 FALSE);
}
END_TEST

Suite *wf_cliprdr_invariant_suite(void)
{
	Suite *s;
	TCase *tc_core;

	s = suite_create("wf_cliprdr_invariants");
	tc_core = tcase_create("descriptor_size");

	tcase_add_test(tc_core, test_descriptor_size_rejects_buffer_smaller_than_header);
	tcase_add_test(tc_core, test_descriptor_size_rejects_zero_items);
	tcase_add_test(tc_core, test_descriptor_size_accepts_max_stream_count);
	tcase_add_test(tc_core, test_descriptor_size_rejects_stream_count_above_limit);
	tcase_add_test(tc_core, test_descriptor_size_rejects_truncated_descriptor_array);
	tcase_add_test(tc_core, test_descriptor_size_rejects_extreme_count);

	suite_add_tcase(s, tc_core);
	return s;
}

int main(void)
{
	int number_failed;
	Suite *s;
	SRunner *sr;

	s = wf_cliprdr_invariant_suite();
	sr = srunner_create(s);

	srunner_run_all(sr, CK_NORMAL);
	number_failed = srunner_ntests_failed(sr);
	srunner_free(sr);

	return (number_failed == 0) ? EXIT_SUCCESS : EXIT_FAILURE;
}
