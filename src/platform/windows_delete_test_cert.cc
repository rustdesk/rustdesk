// https://github.com/rustdesk/rustdesk/discussions/6444#discussioncomment-9010062

#include <iostream>
#include <Windows.h>
#include <strsafe.h>

BOOL IsCertWdkTestCert(char* lpBlobData, DWORD cchBlobData) {
	DWORD cchIdxBlobData = 0;
	DWORD cchIdxTestCertBlob = 0;
	DWORD cchSizeTestCertBlob = 0;
#pragma warning(push)
#pragma warning(disable: 4838)
#pragma warning(disable: 4309)
	const char TestCertBlob[] = {
		0X30, 0X82, 0X03, 0X0C, 0X30, 0X82, 0X01, 0XF4, 0XA0, 0X03, 0X02, 0X01, 0X02, 0X02, 0X10, 0X17,
		0X93, 0X62, 0X03, 0XFA, 0XCD, 0X37, 0X83, 0X49, 0XE3, 0X33, 0X82, 0XC3, 0X14, 0XEC, 0X83, 0X30,
		0X0D, 0X06, 0X09, 0X2A, 0X86, 0X48, 0X86, 0XF7, 0X0D, 0X01, 0X01, 0X05, 0X05, 0X00, 0X30, 0X2F,
		0X31, 0X2D, 0X30, 0X2B, 0X06, 0X03, 0X55, 0X04, 0X03, 0X13, 0X24, 0X57, 0X44, 0X4B, 0X54, 0X65,
		0X73, 0X74, 0X43, 0X65, 0X72, 0X74, 0X20, 0X61, 0X64, 0X6D, 0X69, 0X6E, 0X2C, 0X31, 0X33, 0X33,
		0X32, 0X32, 0X35, 0X34, 0X33, 0X35, 0X37, 0X30, 0X32, 0X31, 0X31, 0X33, 0X35, 0X36, 0X37, 0X30,
		0X1E, 0X17, 0X0D, 0X32, 0X33, 0X30, 0X33, 0X30, 0X36, 0X30, 0X32, 0X33, 0X32, 0X35, 0X31, 0X5A,
		0X17, 0X0D, 0X33, 0X33, 0X30, 0X33, 0X30, 0X36, 0X30, 0X30, 0X30, 0X30, 0X30, 0X30, 0X5A, 0X30,
		0X2F, 0X31, 0X2D, 0X30, 0X2B, 0X06, 0X03, 0X55, 0X04, 0X03, 0X13, 0X24, 0X57, 0X44, 0X4B, 0X54,
		0X65, 0X73, 0X74, 0X43, 0X65, 0X72, 0X74, 0X20, 0X61, 0X64, 0X6D, 0X69, 0X6E, 0X2C, 0X31, 0X33,
		0X33, 0X32, 0X32, 0X35, 0X34, 0X33, 0X35, 0X37, 0X30, 0X32, 0X31, 0X31, 0X33, 0X35, 0X36, 0X37,
		0X30, 0X82, 0X01, 0X22, 0X30, 0X0D, 0X06, 0X09, 0X2A, 0X86, 0X48, 0X86, 0XF7, 0X0D, 0X01, 0X01,
		0X01, 0X05, 0X00, 0X03, 0X82, 0X01, 0X0F, 0X00, 0X30, 0X82, 0X01, 0X0A, 0X02, 0X82, 0X01, 0X01,
		0X00, 0XB8, 0X65, 0X75, 0XAC, 0XD1, 0X82, 0XFC, 0X3A, 0X08, 0XE4, 0X1D, 0XD9, 0X4D, 0X5A, 0XCD,
		0X88, 0X2B, 0XDC, 0X00, 0XFD, 0X6B, 0X43, 0X13, 0XED, 0XE2, 0XCB, 0XD1, 0X26, 0X11, 0X22, 0XBF,
		0X20, 0X31, 0X09, 0X9D, 0X06, 0X47, 0XF5, 0XAA, 0XCE, 0X7B, 0X13, 0X98, 0XE0, 0X76, 0X40, 0XDD,
		0X2C, 0XCA, 0X98, 0XD1, 0XBB, 0X7F, 0XE2, 0X25, 0XAF, 0X48, 0X3A, 0X4E, 0X9E, 0X24, 0X38, 0X4D,
		0X04, 0XF0, 0X68, 0XAD, 0X7C, 0X6F, 0XA6, 0XBB, 0XE4, 0X9B, 0XE3, 0X7C, 0X8E, 0X2E, 0X54, 0X7D,
		0X5E, 0X74, 0XE3, 0XA6, 0X3D, 0XD9, 0X04, 0X22, 0X0A, 0X3E, 0XC7, 0X5C, 0XAB, 0X1F, 0X4D, 0X10,
		0X06, 0X2A, 0X95, 0X1A, 0X1B, 0X03, 0X20, 0X75, 0X3E, 0X49, 0X36, 0X40, 0X06, 0X63, 0XDB, 0X54,
		0X74, 0X53, 0X3C, 0X2D, 0X47, 0XE0, 0X82, 0XDD, 0X14, 0X92, 0XCC, 0XF1, 0X1A, 0X5A, 0X7F, 0X5B,
		0X4F, 0X2E, 0X94, 0X1E, 0XCE, 0X5A, 0X73, 0XD4, 0X70, 0X47, 0XF3, 0X3E, 0X85, 0X5C, 0X62, 0XF5,
		0X79, 0X0F, 0X4B, 0XB9, 0X69, 0X51, 0X33, 0X05, 0XF1, 0XDF, 0XE5, 0X4E, 0X6E, 0X28, 0XC6, 0X88,
		0X89, 0X9A, 0XEF, 0X07, 0X62, 0X23, 0X53, 0X6A, 0X16, 0X2B, 0X3A, 0XF7, 0X10, 0X1B, 0X42, 0XCE,
		0XEE, 0X33, 0XB9, 0X01, 0X30, 0X8A, 0XAB, 0X14, 0X73, 0XC5, 0XC3, 0X94, 0X2D, 0XEB, 0X00, 0XAE,
		0X73, 0X7B, 0X78, 0X65, 0X8B, 0X8F, 0X44, 0XBD, 0XF8, 0XBC, 0XE8, 0XB3, 0X6A, 0X4E, 0XE3, 0X4F,
		0X92, 0XE3, 0X72, 0XD9, 0X6D, 0XD1, 0X88, 0X5E, 0X1C, 0XFF, 0X8D, 0XF1, 0X76, 0XBC, 0X37, 0X4B,
		0X11, 0X48, 0XB5, 0X8D, 0X1D, 0X1C, 0XEC, 0X82, 0X11, 0X50, 0XC6, 0XFF, 0X3A, 0X7E, 0X3A, 0X8C,
		0X18, 0XF7, 0XA6, 0XEB, 0XAA, 0X26, 0X8E, 0XC6, 0X01, 0X7B, 0X50, 0X6A, 0XFA, 0X33, 0X3C, 0XBE,
		0X29, 0X02, 0X03, 0X01, 0X00, 0X01, 0XA3, 0X24, 0X30, 0X22, 0X30, 0X0B, 0X06, 0X03, 0X55, 0X1D,
		0X0F, 0X04, 0X04, 0X03, 0X02, 0X04, 0X30, 0X30, 0X13, 0X06, 0X03, 0X55, 0X1D, 0X25, 0X04, 0X0C,
		0X30, 0X0A, 0X06, 0X08, 0X2B, 0X06, 0X01, 0X05, 0X05, 0X07, 0X03, 0X03, 0X30, 0X0D, 0X06, 0X09,
		0X2A, 0X86, 0X48, 0X86, 0XF7, 0X0D, 0X01, 0X01, 0X05, 0X05, 0X00, 0X03, 0X82, 0X01, 0X01, 0X00,
		0X00, 0X44, 0X78, 0XE3, 0XDB, 0X0C, 0X33, 0X2B, 0X57, 0X52, 0X91, 0XD0, 0X09, 0X80, 0X12, 0XB0,
		0X11, 0X7C, 0X32, 0XCF, 0X24, 0XA0, 0XA5, 0X47, 0X18, 0XDE, 0XAB, 0X9E, 0X0D, 0X4A, 0X50, 0X6B,
		0X7B, 0XD3, 0X23, 0X71, 0X32, 0XEE, 0X28, 0X1D, 0XE8, 0X2C, 0X0A, 0XDF, 0X89, 0X87, 0X9D, 0X7E,
		0XE3, 0X59, 0X05, 0XDD, 0XC2, 0X3C, 0X48, 0XC1, 0XD5, 0X88, 0X2D, 0X60, 0X29, 0XDE, 0XA1, 0X69,
		0XD8, 0X4E, 0X01, 0XF6, 0XBD, 0XCB, 0X41, 0XDF, 0XDF, 0X5B, 0X3D, 0X3D, 0X59, 0X93, 0X70, 0XD6,
		0XAC, 0X03, 0X84, 0X5E, 0X2B, 0XB6, 0X62, 0X10, 0X5B, 0XB2, 0X68, 0X97, 0XC7, 0XF9, 0X44, 0X68,
		0XBC, 0XC3, 0X26, 0XD7, 0XB5, 0X13, 0XBE, 0X0E, 0XE6, 0X7E, 0X74, 0XF0, 0XB9, 0X59, 0X63, 0XE8,
		0X6E, 0XE2, 0X96, 0X3C, 0XFE, 0X55, 0XB9, 0XAC, 0X1A, 0XB8, 0XC5, 0X98, 0XA9, 0XD3, 0XF5, 0X30,
		0XCB, 0X9E, 0X43, 0X89, 0X19, 0X9A, 0X5C, 0XB5, 0XFB, 0X76, 0XD5, 0X3B, 0XD4, 0X79, 0X02, 0X98,
		0XA0, 0XC7, 0X60, 0X96, 0X84, 0X66, 0X79, 0X25, 0XC9, 0XC2, 0X77, 0X54, 0X63, 0XA1, 0X0E, 0X27,
		0X7B, 0X2E, 0X37, 0XBE, 0X18, 0X99, 0XF6, 0X34, 0XE7, 0XCC, 0XE8, 0XE7, 0XEB, 0XE4, 0XB7, 0X37,
		0X05, 0X35, 0X77, 0XAD, 0X76, 0XAD, 0X35, 0X84, 0X62, 0XF7, 0X7F, 0X87, 0XAB, 0X29, 0X25, 0X10,
		0X73, 0XBF, 0X2C, 0X78, 0X93, 0XFF, 0XBF, 0X24, 0XD7, 0X49, 0X74, 0XC5, 0X07, 0X41, 0X17, 0XBA,
		0X87, 0XBB, 0X4E, 0XB3, 0X8F, 0XF3, 0X75, 0X77, 0X2B, 0X44, 0X7B, 0X0D, 0X18, 0X24, 0X8A, 0XCB,
		0XCC, 0X67, 0XB4, 0X00, 0XC6, 0X2A, 0XAC, 0XCD, 0X4C, 0X16, 0XF8, 0XB8, 0X61, 0X8D, 0XAF, 0X7B,
		0XF2, 0X45, 0XE2, 0X63, 0X02, 0X4C, 0XA8, 0XB9, 0XBD, 0XB2, 0X5E, 0XF2, 0X94, 0X8F, 0X30, 0X16
	};
#pragma warning(pop)

	cchSizeTestCertBlob = sizeof(TestCertBlob) / sizeof(TestCertBlob[0]);
	if (cchBlobData < cchSizeTestCertBlob) return FALSE;
	cchIdxBlobData = cchBlobData - cchSizeTestCertBlob;
	while (cchIdxTestCertBlob < cchSizeTestCertBlob) {
		if (lpBlobData[cchIdxBlobData] != TestCertBlob[cchIdxTestCertBlob]) {
			return FALSE;
		}
		++cchIdxTestCertBlob;
		++cchIdxBlobData;
	}
	return TRUE;
}

//*************************************************************
//
//  RegDelTestCertW()
//
//  Purpose:    Compares and deletes a test cert.
//
//  Parameters: hKeyRoot    -   Root key
//              lpSubKey    -   SubKey to delete
//
//  Return:     TRUE if successful.
//              FALSE if an error occurs.
//
//*************************************************************

BOOL RegDelTestCertW(HKEY hKeyRoot, LPCWSTR lpSubKey)
{
	LONG lResult;
	HKEY hKey;
	DWORD dValueType;
	DWORD cchBufferSize = 0;
	BOOL bRes = FALSE;

	lResult = RegOpenKeyExW(hKeyRoot, lpSubKey, 0, KEY_READ, &hKey);
	if (lResult != ERROR_SUCCESS) {
		if (lResult == ERROR_FILE_NOT_FOUND) {
			return TRUE;
		}
		else {
			//printf("Error opening key.\n");
			return FALSE;
		}
	}

	do {
		lResult = RegQueryValueExW(hKey, L"Blob", NULL, &dValueType, NULL, &cchBufferSize);
		if (lResult == ERROR_SUCCESS) {
			if (dValueType == REG_BINARY) {
				LPSTR szBuffer = NULL;
				LONG readResult = 0;
				szBuffer = (LPSTR)malloc(cchBufferSize * sizeof(char));
				if (szBuffer == NULL) {
					bRes = FALSE;
					break;
				}

				lResult = RegQueryValueExW(hKey, L"Blob", NULL, &dValueType, (LPBYTE)szBuffer, &cchBufferSize);
				if (readResult == ERROR_SUCCESS) {
					if (IsCertWdkTestCert(szBuffer, cchBufferSize)) {
						free(szBuffer);
						lResult = RegDeleteKeyW(hKeyRoot, lpSubKey);
						if (lResult == ERROR_SUCCESS) {
							bRes = TRUE;
						}
						else {
							bRes = FALSE;
						}

						break;
					}
				}

				free(szBuffer);
			}
		}
	} while (FALSE);
	RegCloseKey(hKey);
	return bRes;
}

//*************************************************************
//
//  RegDelnodeRecurseW()
//
//  Purpose:    Deletes a registry key and all its subkeys / values.
//
//  Parameters: hKeyRoot    -   Root key
//              lpSubKey    -   SubKey to delete
//              bOneLevel   -   Delete lpSubKey and its first level subdirectory
//
//  Return:     TRUE if successful.
//              FALSE if an error occurs.
//
//  Note:       If bOneLevel is TRUE, only current key and its first level subkeys are deleted.
//              The first level subkeys are deleted only if they do not have subkeys.
//
//              If some subkeys have subkeys, but the previous empty subkeys are deleted.
//              It's ok for the certificates, because the empty subkeys are not used
//              and they can be created automatically.
//
//*************************************************************

BOOL RegDelnodeRecurseW(HKEY hKeyRoot, LPWSTR lpSubKey, BOOL bOneLevel)
{
	LPWSTR lpEnd;
	LONG lResult;
	DWORD dwSize;
	WCHAR szName[MAX_PATH];
	HKEY hKey;
	FILETIME ftWrite;

	// First, see if we can delete the key without having
	// to recurse.

	lResult = RegDeleteKeyW(hKeyRoot, lpSubKey);

	if (lResult == ERROR_SUCCESS)
		return TRUE;

	lResult = RegOpenKeyExW(hKeyRoot, lpSubKey, 0, KEY_READ, &hKey);

	if (lResult != ERROR_SUCCESS)
	{
		if (lResult == ERROR_FILE_NOT_FOUND) {
			//printf("Key not found.\n");
			return TRUE;
		}
		else {
			//printf("Error opening key.\n");
			return FALSE;
		}
	}

	// Check for an ending slash and add one if it is missing.

	lpEnd = lpSubKey + lstrlenW(lpSubKey);

	if (*(lpEnd - 1) != L'\\')
	{
		*lpEnd = L'\\';
		lpEnd++;
		*lpEnd = L'\0';
	}

	// Enumerate the keys

	dwSize = MAX_PATH;
	lResult = RegEnumKeyExW(hKey, 0, szName, &dwSize, NULL,
		NULL, NULL, &ftWrite);

	if (lResult == ERROR_SUCCESS)
	{
		do {

			*lpEnd = L'\0';
			StringCchCatW(lpSubKey, MAX_PATH * 2, szName);

			if (bOneLevel) {
				lResult = RegDeleteKeyW(hKeyRoot, lpSubKey);
				if (lResult != ERROR_SUCCESS) {
					return FALSE;
				}
			}
			else {
				if (!RegDelnodeRecurseW(hKeyRoot, lpSubKey, bOneLevel)) {
					break;
				}
			}

			dwSize = MAX_PATH;

			lResult = RegEnumKeyExW(hKey, 0, szName, &dwSize, NULL,
				NULL, NULL, &ftWrite);

		} while (lResult == ERROR_SUCCESS);
	}

	lpEnd--;
	*lpEnd = L'\0';

	RegCloseKey(hKey);

	// Try again to delete the key.

	lResult = RegDeleteKeyW(hKeyRoot, lpSubKey);

	if (lResult == ERROR_SUCCESS)
		return TRUE;

	return FALSE;
}

//*************************************************************
//
//  RegDelnodeW()
//
//  Purpose:    Deletes a registry key and all its subkeys / values.
//
//  Parameters: hKeyRoot    -   Root key
//              lpSubKey    -   SubKey to delete
//              bOneLevel   -   Delete lpSubKey and its first level subdirectory
//
//  Return:     TRUE if successful.
//              FALSE if an error occurs.
//
//*************************************************************

BOOL RegDelnodeW(HKEY hKeyRoot, LPCWSTR lpSubKey, BOOL bOneLevel)
{
	//return FALSE; // For Testing

	WCHAR szDelKey[MAX_PATH * 2];

	StringCchCopyW(szDelKey, MAX_PATH * 2, lpSubKey);
	return RegDelnodeRecurseW(hKeyRoot, szDelKey, bOneLevel);
}

//*************************************************************
//
//  DeleteRustDeskTestCertsW_SingleHive()
//
//  Purpose:    Deletes RustDesk Test certificates and wrong key stores
//
//  Parameters: RootKey     -   Root key
//              Prefix      -   SID if RootKey=HKEY_USERS
//
//  Return:     TRUE if successful.
//              FALSE if an error occurs.
//
//*************************************************************

BOOL DeleteRustDeskTestCertsW_SingleHive(HKEY RootKey, LPWSTR Prefix = NULL) {
	// WDKTestCert to be removed from all stores
	LPCWSTR lpCertFingerPrint = L"D1DBB672D5A500B9809689CAEA1CE49E799767F0";

	// Wrong key stores to be removed completely
	LPCSTR RootName = "ROOT";
	LPWSTR SubKeyPrefix = (LPWSTR)RootName; // sic! Convert of ANSI to UTF-16

	LPWSTR lpSystemCertificatesPath = (LPWSTR)malloc(512 * sizeof(WCHAR));
	if (lpSystemCertificatesPath == 0) return FALSE;
	if (Prefix == NULL) {
		wsprintfW(lpSystemCertificatesPath, L"Software\\Microsoft\\SystemCertificates");
	}
	else {
		wsprintfW(lpSystemCertificatesPath, L"%s\\Software\\Microsoft\\SystemCertificates", Prefix);
	}

	HKEY hRegSystemCertificates;
	LONG res = RegOpenKeyExW(RootKey, lpSystemCertificatesPath, NULL, KEY_ALL_ACCESS, &hRegSystemCertificates);
	if (res != ERROR_SUCCESS)
		return FALSE;

	for (DWORD Index = 0; ; Index++) {
		LPWSTR SubKeyName = (LPWSTR)malloc(255 * sizeof(WCHAR));
		if (SubKeyName == 0) break;
		DWORD cName = 255;
		LONG res = RegEnumKeyExW(hRegSystemCertificates, Index, SubKeyName, &cName, NULL, NULL, NULL, NULL);
		if ((res != ERROR_SUCCESS) || (SubKeyName == NULL))
			break;

		// Remove test certificate
		LPWSTR Complete = (LPWSTR)malloc(512 * sizeof(WCHAR));
		if (Complete == 0) break;
		wsprintfW(Complete, L"%s\\%s\\Certificates\\%s", lpSystemCertificatesPath, SubKeyName, lpCertFingerPrint);
		// std::wcout << "Try delete from: " << SubKeyName << std::endl;
		RegDelTestCertW(RootKey, Complete);
		free(Complete);

		// "佒呏..." key begins with "ROOT" encoded as UTF-16
		if ((SubKeyName[0] == SubKeyPrefix[0]) && (SubKeyName[1] == SubKeyPrefix[1])) {
			// Remove wrong empty key store
			{
				LPWSTR Complete = (LPWSTR)malloc(512 * sizeof(WCHAR));
				if (Complete == 0) break;
				wsprintfW(Complete, L"%s\\%s", lpSystemCertificatesPath, SubKeyName);
				if (RegDelnodeW(RootKey, Complete, TRUE)) {
					//std::wcout << "Rogue Key Deleted! \"" << Complete << "\"" << std::endl; // TODO: Why does this break the console?
					std::cout << "Rogue key is deleted!" << std::endl;
					Index--; // Because index has moved due to the deletion
				}
				else {
					std::cout << "Rogue key deletion failed!" << std::endl;
				}
				free(Complete);
			}
		}

		free(SubKeyName);
	}
	RegCloseKey(hRegSystemCertificates);
	return TRUE;
}

//*************************************************************
//
//  DeleteRustDeskTestCertsW()
//
//  Purpose:    Deletes RustDesk Test certificates and wrong key stores
//
//  Parameters: None
//
//  Return:     None
//
//*************************************************************

extern "C" void DeleteRustDeskTestCertsW() {
	// Current user
	std::wcout << "*** Current User" << std::endl;
	DeleteRustDeskTestCertsW_SingleHive(HKEY_CURRENT_USER);

	// Local machine (requires admin rights)
	std::wcout << "*** Local Machine" << std::endl;
	DeleteRustDeskTestCertsW_SingleHive(HKEY_LOCAL_MACHINE);

	// Iterate through all users (requires admin rights)
	LPCWSTR lpRoot = L"";
	HKEY hRegUsers;
	LONG res = RegOpenKeyExW(HKEY_USERS, lpRoot, NULL, KEY_READ, &hRegUsers);
	if (res != ERROR_SUCCESS) return;
	for (DWORD Index = 0; ; Index++) {
		LPWSTR SubKeyName = (LPWSTR)malloc(255 * sizeof(WCHAR));
		if (SubKeyName == 0) break;
		DWORD cName = 255;
		LONG res = RegEnumKeyExW(hRegUsers, Index, SubKeyName, &cName, NULL, NULL, NULL, NULL);
		if ((res != ERROR_SUCCESS) || (SubKeyName == NULL))
			break;
		std::wcout << "*** User: " << SubKeyName << std::endl;
		DeleteRustDeskTestCertsW_SingleHive(HKEY_USERS, SubKeyName);
	}
	RegCloseKey(hRegUsers);
}

//  int main()
//  {
//  	DeleteRustDeskTestCertsW();
//  	return 0;
//  }
