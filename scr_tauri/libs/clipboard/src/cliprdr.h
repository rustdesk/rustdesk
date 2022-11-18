#ifndef WF_CLIPRDR_H__
#define WF_CLIPRDR_H__

#ifdef __cplusplus
extern "C"
{
#endif

    typedef signed char INT8, *PINT8;
    typedef signed short INT16, *PINT16;
    typedef signed int INT32, *PINT32;
    typedef unsigned char UINT8, *PUINT8;
    typedef unsigned short UINT16, *PUINT16;
    typedef unsigned int UINT32, *PUINT32;
    typedef unsigned int UINT;
    typedef int BOOL;
    typedef unsigned char BYTE;

/* Clipboard Messages */
#define DEFINE_CLIPRDR_HEADER_COMMON() \
    UINT32 connID;                     \
    UINT16 msgType;                    \
    UINT16 msgFlags;                   \
    UINT32 dataLen

    struct _CLIPRDR_HEADER
    {
        DEFINE_CLIPRDR_HEADER_COMMON();
    };
    typedef struct _CLIPRDR_HEADER CLIPRDR_HEADER;

    struct _CLIPRDR_CAPABILITY_SET
    {
        UINT16 capabilitySetType;
        UINT16 capabilitySetLength;
    };
    typedef struct _CLIPRDR_CAPABILITY_SET CLIPRDR_CAPABILITY_SET;

    struct _CLIPRDR_GENERAL_CAPABILITY_SET
    {
        UINT16 capabilitySetType;
        UINT16 capabilitySetLength;

        UINT32 version;
        UINT32 generalFlags;
    };
    typedef struct _CLIPRDR_GENERAL_CAPABILITY_SET CLIPRDR_GENERAL_CAPABILITY_SET;

    struct _CLIPRDR_CAPABILITIES
    {
        DEFINE_CLIPRDR_HEADER_COMMON();

        UINT32 cCapabilitiesSets;
        CLIPRDR_CAPABILITY_SET *capabilitySets;
    };
    typedef struct _CLIPRDR_CAPABILITIES CLIPRDR_CAPABILITIES;

    struct _CLIPRDR_MONITOR_READY
    {
        DEFINE_CLIPRDR_HEADER_COMMON();
    };
    typedef struct _CLIPRDR_MONITOR_READY CLIPRDR_MONITOR_READY;

    struct _CLIPRDR_TEMP_DIRECTORY
    {
        DEFINE_CLIPRDR_HEADER_COMMON();

        char szTempDir[520];
    };
    typedef struct _CLIPRDR_TEMP_DIRECTORY CLIPRDR_TEMP_DIRECTORY;

    struct _CLIPRDR_FORMAT
    {
        UINT32 formatId;
        char *formatName;
    };
    typedef struct _CLIPRDR_FORMAT CLIPRDR_FORMAT;

    struct _CLIPRDR_FORMAT_LIST
    {
        DEFINE_CLIPRDR_HEADER_COMMON();

        UINT32 numFormats;
        CLIPRDR_FORMAT *formats;
    };
    typedef struct _CLIPRDR_FORMAT_LIST CLIPRDR_FORMAT_LIST;

    struct _CLIPRDR_FORMAT_LIST_RESPONSE
    {
        DEFINE_CLIPRDR_HEADER_COMMON();
    };
    typedef struct _CLIPRDR_FORMAT_LIST_RESPONSE CLIPRDR_FORMAT_LIST_RESPONSE;

    struct _CLIPRDR_LOCK_CLIPBOARD_DATA
    {
        DEFINE_CLIPRDR_HEADER_COMMON();

        UINT32 clipDataId;
    };
    typedef struct _CLIPRDR_LOCK_CLIPBOARD_DATA CLIPRDR_LOCK_CLIPBOARD_DATA;

    struct _CLIPRDR_UNLOCK_CLIPBOARD_DATA
    {
        DEFINE_CLIPRDR_HEADER_COMMON();

        UINT32 clipDataId;
    };
    typedef struct _CLIPRDR_UNLOCK_CLIPBOARD_DATA CLIPRDR_UNLOCK_CLIPBOARD_DATA;

    struct _CLIPRDR_FORMAT_DATA_REQUEST
    {
        DEFINE_CLIPRDR_HEADER_COMMON();

        UINT32 requestedFormatId;
    };
    typedef struct _CLIPRDR_FORMAT_DATA_REQUEST CLIPRDR_FORMAT_DATA_REQUEST;

    struct _CLIPRDR_FORMAT_DATA_RESPONSE
    {
        DEFINE_CLIPRDR_HEADER_COMMON();

        const BYTE *requestedFormatData;
    };
    typedef struct _CLIPRDR_FORMAT_DATA_RESPONSE CLIPRDR_FORMAT_DATA_RESPONSE;

    struct _CLIPRDR_FILE_CONTENTS_REQUEST
    {
        DEFINE_CLIPRDR_HEADER_COMMON();

        UINT32 streamId;
        UINT32 listIndex;
        UINT32 dwFlags;
        UINT32 nPositionLow;
        UINT32 nPositionHigh;
        UINT32 cbRequested;
        BOOL haveClipDataId;
        UINT32 clipDataId;
    };
    typedef struct _CLIPRDR_FILE_CONTENTS_REQUEST CLIPRDR_FILE_CONTENTS_REQUEST;

    struct _CLIPRDR_FILE_CONTENTS_RESPONSE
    {
        DEFINE_CLIPRDR_HEADER_COMMON();

        UINT32 streamId;
        UINT32 cbRequested;
        const BYTE *requestedData;
    };
    typedef struct _CLIPRDR_FILE_CONTENTS_RESPONSE CLIPRDR_FILE_CONTENTS_RESPONSE;

    typedef struct _cliprdr_client_context CliprdrClientContext;

    typedef UINT (*pcCliprdrServerCapabilities)(CliprdrClientContext *context,
                                                const CLIPRDR_CAPABILITIES *capabilities);
    typedef UINT (*pcCliprdrClientCapabilities)(CliprdrClientContext *context,
                                                const CLIPRDR_CAPABILITIES *capabilities);
    typedef UINT (*pcCliprdrMonitorReady)(CliprdrClientContext *context,
                                          const CLIPRDR_MONITOR_READY *monitorReady);
    typedef UINT (*pcCliprdrTempDirectory)(CliprdrClientContext *context,
                                           const CLIPRDR_TEMP_DIRECTORY *tempDirectory);
    typedef UINT (*pcCliprdrClientFormatList)(CliprdrClientContext *context,
                                              const CLIPRDR_FORMAT_LIST *formatList);
    typedef UINT (*pcCliprdrServerFormatList)(CliprdrClientContext *context,
                                              const CLIPRDR_FORMAT_LIST *formatList);
    typedef UINT (*pcCliprdrClientFormatListResponse)(
        CliprdrClientContext *context, const CLIPRDR_FORMAT_LIST_RESPONSE *formatListResponse);
    typedef UINT (*pcCliprdrServerFormatListResponse)(
        CliprdrClientContext *context, const CLIPRDR_FORMAT_LIST_RESPONSE *formatListResponse);
    typedef UINT (*pcCliprdrClientLockClipboardData)(
        CliprdrClientContext *context, const CLIPRDR_LOCK_CLIPBOARD_DATA *lockClipboardData);
    typedef UINT (*pcCliprdrServerLockClipboardData)(
        CliprdrClientContext *context, const CLIPRDR_LOCK_CLIPBOARD_DATA *lockClipboardData);
    typedef UINT (*pcCliprdrClientUnlockClipboardData)(
        CliprdrClientContext *context, const CLIPRDR_UNLOCK_CLIPBOARD_DATA *unlockClipboardData);
    typedef UINT (*pcCliprdrServerUnlockClipboardData)(
        CliprdrClientContext *context, const CLIPRDR_UNLOCK_CLIPBOARD_DATA *unlockClipboardData);
    typedef UINT (*pcCliprdrClientFormatDataRequest)(
        CliprdrClientContext *context, const CLIPRDR_FORMAT_DATA_REQUEST *formatDataRequest);
    typedef UINT (*pcCliprdrServerFormatDataRequest)(
        CliprdrClientContext *context, const CLIPRDR_FORMAT_DATA_REQUEST *formatDataRequest);
    typedef UINT (*pcCliprdrClientFormatDataResponse)(
        CliprdrClientContext *context, const CLIPRDR_FORMAT_DATA_RESPONSE *formatDataResponse);
    typedef UINT (*pcCliprdrServerFormatDataResponse)(
        CliprdrClientContext *context, const CLIPRDR_FORMAT_DATA_RESPONSE *formatDataResponse);
    typedef UINT (*pcCliprdrClientFileContentsRequest)(
        CliprdrClientContext *context, const CLIPRDR_FILE_CONTENTS_REQUEST *fileContentsRequest);
    typedef UINT (*pcCliprdrServerFileContentsRequest)(
        CliprdrClientContext *context, const CLIPRDR_FILE_CONTENTS_REQUEST *fileContentsRequest);
    typedef UINT (*pcCliprdrClientFileContentsResponse)(
        CliprdrClientContext *context, const CLIPRDR_FILE_CONTENTS_RESPONSE *fileContentsResponse);
    typedef UINT (*pcCliprdrServerFileContentsResponse)(
        CliprdrClientContext *context, const CLIPRDR_FILE_CONTENTS_RESPONSE *fileContentsResponse);

    typedef BOOL (*pcCheckEnabled)(UINT32 connID);

    // TODO: hide more members of clipboard context
    struct _cliprdr_client_context
    {
        void *custom;
        BOOL enableFiles;
        BOOL enableOthers;

        pcCheckEnabled CheckEnabled;
        pcCliprdrServerCapabilities ServerCapabilities;
        pcCliprdrClientCapabilities ClientCapabilities;
        pcCliprdrMonitorReady MonitorReady;
        pcCliprdrTempDirectory TempDirectory;
        pcCliprdrClientFormatList ClientFormatList;
        pcCliprdrServerFormatList ServerFormatList;
        pcCliprdrClientFormatListResponse ClientFormatListResponse;
        pcCliprdrServerFormatListResponse ServerFormatListResponse;
        pcCliprdrClientLockClipboardData ClientLockClipboardData;
        pcCliprdrServerLockClipboardData ServerLockClipboardData;
        pcCliprdrClientUnlockClipboardData ClientUnlockClipboardData;
        pcCliprdrServerUnlockClipboardData ServerUnlockClipboardData;
        pcCliprdrClientFormatDataRequest ClientFormatDataRequest;
        pcCliprdrServerFormatDataRequest ServerFormatDataRequest;
        pcCliprdrClientFormatDataResponse ClientFormatDataResponse;
        pcCliprdrServerFormatDataResponse ServerFormatDataResponse;
        pcCliprdrClientFileContentsRequest ClientFileContentsRequest;
        pcCliprdrServerFileContentsRequest ServerFileContentsRequest;
        pcCliprdrClientFileContentsResponse ClientFileContentsResponse;
        pcCliprdrServerFileContentsResponse ServerFileContentsResponse;

        UINT32 lastRequestedFormatId;
    };

#ifdef __cplusplus
}
#endif

#endif // WF_CLIPRDR_H__

