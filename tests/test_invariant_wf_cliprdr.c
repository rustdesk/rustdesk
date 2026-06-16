#include <check.h>
#include <stdlib.h>
#include <stddef.h>
#include <string.h>

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

/* Regression tests for issue #15291: map_ensure_capacity() must zero-initialize
 * the region added by realloc(), because clear_format_map() frees map->name for
 * every slot up to map_capacity. Without zeroing, those slots hold uninitialized
 * (garbage) pointers and free() corrupts the heap. */

START_TEST(test_map_ensure_capacity_zeroes_grown_slots)
{
	wfClipboard cb;
	size_t i;
	size_t old_capacity;

	/* Dirty the heap so that a non-zeroing realloc would hand back non-NULL
	 * garbage for the grown region, making the regression deterministic. */
	for (i = 0; i < 64; i++)
	{
		void *p = malloc(256 * sizeof(formatMapping));
		if (p)
		{
			memset(p, 0xFF, 256 * sizeof(formatMapping));
			free(p);
		}
	}

	memset(&cb, 0, sizeof(cb));
	cb.map_capacity = 2;
	cb.map_size = 0;
	cb.format_mappings = (formatMapping *)calloc(cb.map_capacity, sizeof(formatMapping));
	ck_assert_ptr_nonnull(cb.format_mappings);

	old_capacity = cb.map_capacity;
	cb.map_size = cb.map_capacity; /* force a grow on the next ensure */
	ck_assert_int_eq(map_ensure_capacity(&cb), TRUE);
	ck_assert_uint_gt(cb.map_capacity, old_capacity);

	for (i = old_capacity; i < cb.map_capacity; i++)
	{
		ck_assert_ptr_null(cb.format_mappings[i].name);
		ck_assert_uint_eq(cb.format_mappings[i].remote_format_id, 0);
		ck_assert_uint_eq(cb.format_mappings[i].local_format_id, 0);
	}

	clear_format_map(&cb);
	free(cb.format_mappings);
}
END_TEST

START_TEST(test_clear_format_map_safe_after_realloc_growth)
{
	wfClipboard cb;
	UINT i;

	memset(&cb, 0, sizeof(cb));
	cb.map_capacity = 2;
	cb.map_size = 0;
	cb.format_mappings = (formatMapping *)calloc(cb.map_capacity, sizeof(formatMapping));
	ck_assert_ptr_nonnull(cb.format_mappings);

	/* Populate a format list larger than the initial capacity, the same way
	 * wf_cliprdr_server_format_list() does: ensure capacity, then write slot i. */
	for (i = 0; i < 10; i++)
	{
		ck_assert_int_eq(map_ensure_capacity(&cb), TRUE);
		cb.format_mappings[i].remote_format_id = i + 1;
		cb.format_mappings[i].local_format_id = i + 1;
		cb.format_mappings[i].name = _wcsdup(L"fmt");
		ck_assert_ptr_nonnull(cb.format_mappings[i].name);
		cb.map_size++;
	}

	ck_assert_uint_ge(cb.map_capacity, 10);

	/* clear_format_map() frees name across the full capacity; the untouched grown
	 * slots must be NULL so this neither crashes nor corrupts the heap. */
	ck_assert_int_eq(clear_format_map(&cb), TRUE);
	ck_assert_uint_eq(cb.map_size, 0);

	free(cb.format_mappings);
}
END_TEST

Suite *wf_cliprdr_invariant_suite(void)
{
	Suite *s;
	TCase *tc_core;
	TCase *tc_format_map;

	s = suite_create("wf_cliprdr_invariants");
	tc_core = tcase_create("descriptor_size");

	tcase_add_test(tc_core, test_descriptor_size_rejects_buffer_smaller_than_header);
	tcase_add_test(tc_core, test_descriptor_size_rejects_zero_items);
	tcase_add_test(tc_core, test_descriptor_size_accepts_max_stream_count);
	tcase_add_test(tc_core, test_descriptor_size_rejects_stream_count_above_limit);
	tcase_add_test(tc_core, test_descriptor_size_rejects_truncated_descriptor_array);
	tcase_add_test(tc_core, test_descriptor_size_rejects_extreme_count);

	suite_add_tcase(s, tc_core);

	tc_format_map = tcase_create("format_map");
	tcase_add_test(tc_format_map, test_map_ensure_capacity_zeroes_grown_slots);
	tcase_add_test(tc_format_map, test_clear_format_map_safe_after_realloc_growth);
	suite_add_tcase(s, tc_format_map);

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
