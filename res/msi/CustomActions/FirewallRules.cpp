// https://learn.microsoft.com/en-us/previous-versions/windows/desktop/ics/c-adding-an-application-rule-edge-traversal

/********************************************************************
Copyright (C) Microsoft. All Rights Reserved.

Abstract:
    This C++ file includes sample code that adds a firewall rule with
 EdgeTraversalOptions (one of the EdgeTraversalOptions values).

********************************************************************/

#include "pch.h"
#include <windows.h>
#include <stdio.h>
#include <netfw.h>
#include <strsafe.h>

#pragma comment(lib, "ole32.lib")
#pragma comment(lib, "oleaut32.lib")

#define STRING_BUFFER_SIZE  500     


// Forward declarations
HRESULT    WFCOMInitialize(INetFwPolicy2** ppNetFwPolicy2);
void       WFCOMCleanup(INetFwPolicy2* pNetFwPolicy2);
HRESULT    RemoveFirewallRule(
    __in INetFwPolicy2* pNetFwPolicy2,
    __in LPWSTR exeName);
HRESULT    AddFirewallRuleWithEdgeTraversal(__in INetFwPolicy2* pNetFwPolicy2,
                                            __in bool in,
                                            __in LPWSTR exeName,
                                            __in LPWSTR exeFile);


bool AddFirewallRule(bool add, LPWSTR exeName, LPWSTR exeFile)
{
    bool result = false;
    HRESULT hrComInit = S_OK;
    HRESULT hr = S_OK;
    INetFwPolicy2* pNetFwPolicy2 = NULL;

    // Initialize COM.
    hrComInit = CoInitializeEx(
        0,
        COINIT_APARTMENTTHREADED
    );

    // Ignore RPC_E_CHANGED_MODE; this just means that COM has already been
    // initialized with a different mode. Since we don't care what the mode is,
    // we'll just use the existing mode.
    if (hrComInit != RPC_E_CHANGED_MODE)
    {
        if (FAILED(hrComInit))
        {
            WcaLog(LOGMSG_STANDARD, "CoInitializeEx failed: 0x%08lx\n", hrComInit);
            goto Cleanup;
        }
    }

    // Retrieve INetFwPolicy2
    hr = WFCOMInitialize(&pNetFwPolicy2);
    if (FAILED(hr))
    {
        goto Cleanup;
    }

    if (add) {
        // Add firewall rule with EdgeTraversalOption=DeferApp (Windows7+) if available 
        //   else add with Edge=True (Vista and Server 2008).
        hr = AddFirewallRuleWithEdgeTraversal(pNetFwPolicy2, true, exeName, exeFile);
        hr = AddFirewallRuleWithEdgeTraversal(pNetFwPolicy2, false, exeName, exeFile);
    }
    else {
        hr = RemoveFirewallRule(pNetFwPolicy2, exeName);
    }
    result = SUCCEEDED(hr);

Cleanup:

    // Release INetFwPolicy2
    WFCOMCleanup(pNetFwPolicy2);

    // Uninitialize COM.
    if (SUCCEEDED(hrComInit))
    {
        CoUninitialize();
    }

    return result;
}

BSTR MakeRuleName(__in LPWSTR exeName)
{
    WCHAR pwszTemp[STRING_BUFFER_SIZE] = L"";
    HRESULT hr = StringCchPrintfW(pwszTemp, STRING_BUFFER_SIZE, L"%ls Service", exeName);
    if (FAILED(hr))
    {
        WcaLog(LOGMSG_STANDARD, "Failed to compose a resource identifier string: 0x%08lx\n", hr);
        return NULL;
    }
    return SysAllocString(pwszTemp);
}

HRESULT    RemoveFirewallRule(
    __in INetFwPolicy2* pNetFwPolicy2,
    __in LPWSTR exeName)
{
    HRESULT hr = S_OK;
    INetFwRules* pNetFwRules = NULL;

    WCHAR pwszTemp[STRING_BUFFER_SIZE] = L"";

    BSTR RuleName = NULL;

    RuleName = MakeRuleName(exeName);
    if (NULL == RuleName)
    {
        WcaLog(LOGMSG_STANDARD, "\nERROR: Insufficient memory\n");
        goto Cleanup;
    }

    hr = pNetFwPolicy2->get_Rules(&pNetFwRules);
    if (FAILED(hr))
    {
        WcaLog(LOGMSG_STANDARD, "Failed to retrieve firewall rules collection : 0x%08lx\n", hr);
        goto Cleanup;
    }

    // We need to "Remove()" twice, because both "in" and "out" rules are added?
    // There's no remarks for this case https://learn.microsoft.com/en-us/windows/win32/api/netfw/nf-netfw-inetfwrules-remove
    hr = pNetFwRules->Remove(RuleName);
    hr = pNetFwRules->Remove(RuleName);
    if (FAILED(hr)) {
        WcaLog(LOGMSG_STANDARD, "Failed to remove firewall rule \"%ls\" : 0x%08lx\n", exeName, hr);
    }
    else {
        WcaLog(LOGMSG_STANDARD, "Firewall rule \"%ls\" is removed\n", exeName);
    }

Cleanup:

    SysFreeString(RuleName);

    if (pNetFwRules != NULL)
    {
        pNetFwRules->Release();
    }

    return hr;
}

// Add firewall rule with EdgeTraversalOption=DeferApp (Windows7+) if available 
//   else add with Edge=True (Vista and Server 2008).
HRESULT    AddFirewallRuleWithEdgeTraversal(
    __in INetFwPolicy2* pNetFwPolicy2,
    __in bool in,
    __in LPWSTR exeName,
    __in LPWSTR exeFile)
{
    HRESULT hr = S_OK;
    INetFwRules* pNetFwRules = NULL;

    INetFwRule* pNetFwRule = NULL;
    INetFwRule2* pNetFwRule2 = NULL;

    WCHAR pwszTemp[STRING_BUFFER_SIZE] = L"";

    BSTR RuleName = NULL;
    BSTR RuleGroupName = NULL;
    BSTR RuleDescription = NULL;
    BSTR RuleAppPath = NULL;

    long CurrentProfilesBitMask = 0;


    //  For localization purposes, the rule name, description, and group can be 
    //    provided as indirect strings. These indirect strings can be defined in an rc file.
    //  Examples of the indirect string definitions in the rc file -
    //    127                     "EdgeTraversalOptions Sample Application"
    //    128                     "Allow inbound TCP traffic to application EdgeTraversalOptions.exe"
    //    129                     "Allow EdgeTraversalOptions.exe to receive inbound traffic for TCP protocol 
    //                          from remote machines located within your network as well as from 
    //                          the Internet (i.e from outside of your Edge device like Firewall or NAT"


    //    Examples of using indirect strings -
    //    hr = StringCchPrintfW(pwszTemp, STRING_BUFFER_SIZE, L"@EdgeTraversalOptions.exe,-128");
    RuleName = MakeRuleName(exeName);
    if (NULL == RuleName)
    {
        WcaLog(LOGMSG_STANDARD, "\nERROR: Insufficient memory\n");
        goto Cleanup;
    }
    //    Examples of using indirect strings -
    //    hr = StringCchPrintfW(pwszTemp, STRING_BUFFER_SIZE, L"@EdgeTraversalOptions.exe,-127");
    hr = StringCchPrintfW(pwszTemp, STRING_BUFFER_SIZE, exeName);
    if (FAILED(hr))
    {
        WcaLog(LOGMSG_STANDARD, "Failed to compose a resource identifier string: 0x%08lx\n", hr);
        goto Cleanup;
    }
    RuleGroupName = SysAllocString(pwszTemp);  // Used for grouping together multiple rules
    if (NULL == RuleGroupName)
    {
        WcaLog(LOGMSG_STANDARD, "\nERROR: Insufficient memory\n");
        goto Cleanup;
    }
    //    Examples of using indirect strings -
    //    hr = StringCchPrintfW(pwszTemp, STRING_BUFFER_SIZE, L"@EdgeTraversalOptions.exe,-129");
    hr = StringCchPrintfW(pwszTemp, STRING_BUFFER_SIZE, L"Allow %ls to receive \
       inbound traffic from remote machines located within your network as well as \
       from the Internet", exeName);
    if (FAILED(hr))
    {
        WcaLog(LOGMSG_STANDARD, "Failed to compose a resource identifier string: 0x%08lx\n", hr);
        goto Cleanup;
    }
    RuleDescription = SysAllocString(pwszTemp);
    if (NULL == RuleDescription)
    {
        WcaLog(LOGMSG_STANDARD, "\nERROR: Insufficient memory\n");
        goto Cleanup;
    }

    RuleAppPath = SysAllocString(exeFile);
    if (NULL == RuleAppPath)
    {
        WcaLog(LOGMSG_STANDARD, "\nERROR: Insufficient memory\n");
        goto Cleanup;
    }

    hr = pNetFwPolicy2->get_Rules(&pNetFwRules);

    if (FAILED(hr))
    {
        WcaLog(LOGMSG_STANDARD, "Failed to retrieve firewall rules collection : 0x%08lx\n", hr);
        goto Cleanup;
    }

    hr = CoCreateInstance(
        __uuidof(NetFwRule),    //CLSID of the class whose object is to be created
        NULL,
        CLSCTX_INPROC_SERVER,
        __uuidof(INetFwRule),   // Identifier of the Interface used for communicating with the object
        (void**)&pNetFwRule);

    if (FAILED(hr))
    {
        WcaLog(LOGMSG_STANDARD, "CoCreateInstance for INetFwRule failed: 0x%08lx\n", hr);
        goto Cleanup;
    }

    hr = pNetFwRule->put_Name(RuleName);
    if (FAILED(hr))
    {
        WcaLog(LOGMSG_STANDARD, "Failed INetFwRule::put_Name failed with error: 0x %x.\n", hr);
        goto Cleanup;
    }

    hr = pNetFwRule->put_Grouping(RuleGroupName);
    if (FAILED(hr))
    {
        WcaLog(LOGMSG_STANDARD, "Failed INetFwRule::put_Grouping failed with error: 0x %x.\n", hr);
        goto Cleanup;
    }

    hr = pNetFwRule->put_Description(RuleDescription);
    if (FAILED(hr))
    {
        WcaLog(LOGMSG_STANDARD, "Failed INetFwRule::put_Description failed with error: 0x %x.\n", hr);
        goto Cleanup;
    }

    // If you want the rule to avoid public, you can refer to
    // https://learn.microsoft.com/en-us/previous-versions/windows/desktop/ics/c-adding-an-outbound-rule
    CurrentProfilesBitMask = NET_FW_PROFILE2_ALL;

    hr = pNetFwRule->put_Direction(in ? NET_FW_RULE_DIR_IN : NET_FW_RULE_DIR_OUT);
    if (FAILED(hr))
    {
        WcaLog(LOGMSG_STANDARD, "Failed INetFwRule::put_Direction failed with error: 0x %x.\n", hr);
        goto Cleanup;
    }


    hr = pNetFwRule->put_Action(NET_FW_ACTION_ALLOW);
    if (FAILED(hr))
    {
        WcaLog(LOGMSG_STANDARD, "Failed INetFwRule::put_Action failed with error: 0x %x.\n", hr);
        goto Cleanup;
    }

    hr = pNetFwRule->put_ApplicationName(RuleAppPath);
    if (FAILED(hr))
    {
        WcaLog(LOGMSG_STANDARD, "Failed INetFwRule::put_ApplicationName failed with error: 0x %x.\n", hr);
        goto Cleanup;
    }

    //hr = pNetFwRule->put_Protocol(6);  // TCP
    //if (FAILED(hr))
    //{
    //    WcaLog(LOGMSG_STANDARD, "Failed INetFwRule::put_Protocol failed with error: 0x %x.\n", hr);
    //    goto Cleanup;
    //}

    hr = pNetFwRule->put_Profiles(CurrentProfilesBitMask);
    if (FAILED(hr))
    {
        WcaLog(LOGMSG_STANDARD, "Failed INetFwRule::put_Profiles failed with error: 0x %x.\n", hr);
        goto Cleanup;
    }

    hr = pNetFwRule->put_Enabled(VARIANT_TRUE);
    if (FAILED(hr))
    {
        WcaLog(LOGMSG_STANDARD, "Failed INetFwRule::put_Enabled failed with error: 0x %x.\n", hr);
        goto Cleanup;
    }

    if (in) {
        // Check if INetFwRule2 interface is available (i.e Windows7+)
        // If supported, then use EdgeTraversalOptions
        // Else use the EdgeTraversal boolean flag.

        if (SUCCEEDED(pNetFwRule->QueryInterface(__uuidof(INetFwRule2), (void**)&pNetFwRule2)))
        {
            hr = pNetFwRule2->put_EdgeTraversalOptions(NET_FW_EDGE_TRAVERSAL_TYPE_DEFER_TO_APP);
            if (FAILED(hr))
            {
                WcaLog(LOGMSG_STANDARD, "Failed INetFwRule::put_EdgeTraversalOptions failed with error: 0x %x.\n", hr);
                goto Cleanup;
            }
        }
        else
        {
            hr = pNetFwRule->put_EdgeTraversal(VARIANT_TRUE);
            if (FAILED(hr))
            {
                WcaLog(LOGMSG_STANDARD, "Failed INetFwRule::put_EdgeTraversal failed with error: 0x %x.\n", hr);
                goto Cleanup;
            }
        }
    }

    hr = pNetFwRules->Add(pNetFwRule);
    if (FAILED(hr))
    {
        WcaLog(LOGMSG_STANDARD, "Failed to add firewall rule to the firewall rules collection : 0x%08lx\n", hr);
        goto Cleanup;
    }

    WcaLog(LOGMSG_STANDARD, "Successfully added firewall rule !\n");

Cleanup:

    SysFreeString(RuleName);
    SysFreeString(RuleGroupName);
    SysFreeString(RuleDescription);
    SysFreeString(RuleAppPath);

    if (pNetFwRule2 != NULL)
    {
        pNetFwRule2->Release();
    }

    if (pNetFwRule != NULL)
    {
        pNetFwRule->Release();
    }

    if (pNetFwRules != NULL)
    {
        pNetFwRules->Release();
    }

    return hr;
}


// Instantiate INetFwPolicy2
HRESULT WFCOMInitialize(INetFwPolicy2** ppNetFwPolicy2)
{
    HRESULT hr = S_OK;

    hr = CoCreateInstance(
        __uuidof(NetFwPolicy2),
        NULL,
        CLSCTX_INPROC_SERVER,
        __uuidof(INetFwPolicy2),
        (void**)ppNetFwPolicy2);

    if (FAILED(hr))
    {
        WcaLog(LOGMSG_STANDARD, "CoCreateInstance for INetFwPolicy2 failed: 0x%08lx\n", hr);
        goto Cleanup;
    }

Cleanup:
    return hr;
}


// Release INetFwPolicy2
void WFCOMCleanup(INetFwPolicy2* pNetFwPolicy2)
{
    // Release the INetFwPolicy2 object (Vista+)
    if (pNetFwPolicy2 != NULL)
    {
        pNetFwPolicy2->Release();
    }
}
