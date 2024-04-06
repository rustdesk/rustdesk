// https://github.com/rustdesk/rustdesk/discussions/6444#discussioncomment-9010062

#include <iostream>
#include <Windows.h>
#include <strsafe.h>

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

		// "佒呏..." key begins with "ROOT" encoded as UTF-16
		if ((SubKeyName[0] == SubKeyPrefix[0]) && (SubKeyName[1] == SubKeyPrefix[1])) {
			// Remove test certificate
			{
				LPWSTR Complete = (LPWSTR)malloc(512 * sizeof(WCHAR));
				if (Complete == 0) break;
				wsprintfW(Complete, L"%s\\%s\\Certificates\\%s", lpSystemCertificatesPath, SubKeyName, lpCertFingerPrint);
				// std::wcout << "Try delete from: " << SubKeyName << std::endl;
				RegDelnodeW(RootKey, Complete, FALSE);
				free(Complete);
			}

			// Remove wrong empty key store
			{
				LPWSTR Complete = (LPWSTR)malloc(512 * sizeof(WCHAR));
				if (Complete == 0) break;
				wsprintfW(Complete, L"%s\\%s", lpSystemCertificatesPath, SubKeyName);
				if (RegDelnodeW(RootKey, Complete, TRUE)) {
					//std::wcout << "Rogue Key Deleted! \"" << Complete << "\"" << std::endl; // TODO: Why does this break the console?
					std::wcout << "Rogue key is deleted!" << std::endl;
					Index--; // Because index has moved due to the deletion
				} else {
					std::wcout << "Rogue key deletion failed!" << std::endl;
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

// int main()
// {
// 	DeleteRustDeskTestCertsW();
// 	return 0;
// }
