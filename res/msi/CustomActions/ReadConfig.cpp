#include "pch.h"

#include <iostream>
#include <fstream>
#include <string>
#include <cwctype>

void trim(std::wstring& str) {
    str.erase(str.begin(), std::find_if(str.begin(), str.end(), [](wchar_t ch) {
        return !std::iswspace(ch);
        }));
    str.erase(std::find_if(str.rbegin(), str.rend(), [](wchar_t ch) {
        return !std::iswspace(ch);
        }).base(), str.end());
}

std::wstring ReadConfig(const std::wstring& filename, const std::wstring& key)
{
    std::wstring configValue;
    std::wstring line;
    std::wifstream file(filename);
    while (std::getline(file, line)) {
        trim(line);
        if (line.find(key) == 0) {
            std::size_t position = line.find(L"=", key.size());
            if (position != std::string::npos) {
                configValue = line.substr(position + 1);
                trim(configValue);
                break;
            }
        }
    }

    file.close();
    return configValue;
}
