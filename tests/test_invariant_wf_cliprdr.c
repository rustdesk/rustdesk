#include <check.h>
#include <stdlib.h>
#include <stddef.h>

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

static UINT test_client_file_contents_request(
	CliprdrClientContext *context,
	const CLIPRDR_FILE_CONTENTS_REQUEST *request)
{
	CLIPRDR_FILE_CONTENTS_RESPONSE response = { 0 };

	ck_assert_int_eq(request->haveClipDataId, FALSE);
	ck_assert_int_eq(request->clipDataId, 0);
	response.msgFlags = CB_RESPONSE_OK;
	response.streamId = request->streamId;
	return wf_cliprdr_server_file_contents_response(context, &response);
}

START_TEST(test_file_contents_request_initializes_optional_fields)
{
	wfClipboard clipboard = { 0 };
	CliprdrClientContext context = { 0 };
	UINT rc;

	clipboard.context = &context;
	clipboard.req_f_mutex = CreateMutex(NULL, FALSE, NULL);
	clipboard.req_fevent = CreateEvent(NULL, TRUE, FALSE, NULL);
	context.Custom = &clipboard;
	context.ResponseWaitTimeoutSecs = 1;
	context.ClientFileContentsRequest = test_client_file_contents_request;

	ck_assert_ptr_nonnull(clipboard.req_f_mutex);
	ck_assert_ptr_nonnull(clipboard.req_fevent);
	rc = cliprdr_send_request_filecontents(&clipboard, 1, 7, 0, FILECONTENTS_SIZE, 0, 0, 0);
	ck_assert_int_eq(rc, CHANNEL_RC_OK);
	ck_assert_int_eq(clipboard.req_f_stream_id_expected, 7);

	CloseHandle(clipboard.req_fevent);
	CloseHandle(clipboard.req_f_mutex);
}
END_TEST

START_TEST(test_file_contents_response_validates_data)
{
	wfClipboard clipboard = { 0 };
	CliprdrClientContext context = { 0 };
	CLIPRDR_FILE_CONTENTS_RESPONSE response = { 0 };
	UINT rc;

	clipboard.context = &context;
	clipboard.req_f_mutex = CreateMutex(NULL, FALSE, NULL);
	clipboard.req_fevent = CreateEvent(NULL, TRUE, FALSE, NULL);
	context.Custom = &clipboard;
	context.ResponseWaitTimeoutSecs = 1;
	response.msgFlags = CB_RESPONSE_OK;
	response.cbRequested = 0;
	clipboard.req_f_stream_id_expected = 7;

	ck_assert_ptr_nonnull(clipboard.req_f_mutex);
	ck_assert_ptr_nonnull(clipboard.req_fevent);

	response.streamId = 8;
	ck_assert_int_eq(wf_cliprdr_server_file_contents_response(&context, &response),
	                 CHANNEL_RC_OK);
	ck_assert_int_eq(WaitForSingleObject(clipboard.req_fevent, 0), WAIT_TIMEOUT);
	ck_assert_ptr_null(clipboard.req_fdata);
	ck_assert_int_eq(clipboard.req_fsize, 0);
	ck_assert_int_eq(clipboard.req_f_response_ok, FALSE);

	response.streamId = 7;
	ck_assert_int_eq(wf_cliprdr_server_file_contents_response(&context, &response),
	                 CHANNEL_RC_OK);
	ck_assert_ptr_null(clipboard.req_fdata);
	ck_assert_int_eq(clipboard.req_fsize, 0);
	ck_assert_int_eq(clipboard.req_f_response_ok, TRUE);

	rc = wait_response_event(0, &clipboard, clipboard.req_fevent,
	                         &clipboard.req_f_received, (void **)&clipboard.req_fdata,
	                         &clipboard.req_f_response_ok);
	ck_assert_int_eq(rc, CHANNEL_RC_OK);

	response.cbRequested = 1;
	clipboard.req_f_size_requested = 1;
	ck_assert_int_eq(wf_cliprdr_server_file_contents_response(&context, &response),
	                 ERROR_INTERNAL_ERROR);
	ck_assert_ptr_null(clipboard.req_fdata);
	ck_assert_int_eq(clipboard.req_fsize, 0);
	ck_assert_int_eq(clipboard.req_f_response_ok, FALSE);

	rc = wait_response_event(0, &clipboard, clipboard.req_fevent,
	                         &clipboard.req_f_received, (void **)&clipboard.req_fdata,
	                         &clipboard.req_f_response_ok);
	ck_assert_int_eq(rc, ERROR_INTERNAL_ERROR);

	CloseHandle(clipboard.req_fevent);
	CloseHandle(clipboard.req_f_mutex);
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
	tcase_add_test(tc_core, test_file_contents_request_initializes_optional_fields);
	tcase_add_test(tc_core, test_file_contents_response_validates_data);

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
