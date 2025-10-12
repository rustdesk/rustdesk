#!/usr/bin/env python3

import requests
import argparse
import json
from datetime import datetime, timedelta, timezone


def format_timestamp(timestamp):
    """Convert Unix timestamp to readable local datetime"""
    if timestamp is None:
        return None
    try:
        # Convert to local time
        local_dt = datetime.fromtimestamp(timestamp)
        return local_dt.strftime("%Y-%m-%d %H:%M:%S")
    except (ValueError, TypeError):
        return timestamp


def parse_local_time_to_utc_string(time_str):
    """Parse local time string to UTC time string for API filtering"""
    try:
        # Parse the local time string
        local_dt = datetime.strptime(time_str, "%Y-%m-%d %H:%M:%S.%f")
        # Make the datetime object timezone-aware using system's local timezone
        local_dt = local_dt.replace(tzinfo=datetime.now().astimezone().tzinfo)
        utc_dt = local_dt.astimezone(timezone.utc)
        return utc_dt.strftime("%Y-%m-%d %H:%M:%S.000")
    except ValueError:
        try:
            # Try without microseconds
            local_dt = datetime.strptime(time_str, "%Y-%m-%d %H:%M:%S")
            # Make the datetime object timezone-aware using system's local timezone
            local_dt = local_dt.replace(tzinfo=datetime.now().astimezone().tzinfo)
            utc_dt = local_dt.astimezone(timezone.utc)
            return utc_dt.strftime("%Y-%m-%d %H:%M:%S.000")
        except ValueError:
            return None


def get_connection_type_name(conn_type):
    """Convert connection type number to readable name"""
    type_map = {
        0: "Remote Desktop",
        1: "File Transfer", 
        2: "Port Transfer",
        3: "View Camera",
        4: "Terminal"
    }
    return type_map.get(conn_type, f"Unknown ({conn_type})")


def get_console_type_name(console_type):
    """Convert console audit type number to readable name"""
    type_map = {
        0: "Group Management",
        1: "User Management", 
        2: "Device Management",
        3: "Address Book Management"
    }
    return type_map.get(console_type, f"Unknown ({console_type})")


def get_console_operation_name(operation_code):
    """Convert console operation code to readable name"""
    operation_map = {
        0: "User Login",
        1: "Add Group",
        2: "Add User", 
        3: "Add Device",
        4: "Delete Groups",
        5: "Disconnect Device",
        6: "Enable Users",
        7: "Disable Users",
        8: "Enable Devices",
        9: "Disable Devices",
        10: "Update Group",
        11: "Update User",
        12: "Update Device",
        13: "Delete User",
        14: "Delete Device",
        15: "Add Address Book",
        16: "Delete Address Book",
        17: "Change Address Book Name",
        18: "Delete Devices in the Address Book Recycle Bin",
        19: "Empty Address Book Recycle Bin",
        20: "Add Address Book Permission",
        21: "Delete Address Book Permission",
        22: "Update Address Book Permission"
    }
    return operation_map.get(operation_code, f"Unknown ({operation_code})")


def get_alarm_type_name(alarm_type):
    """Convert alarm type number to readable name"""
    type_map = {
        0: "Access attempt outside the IP whiltelist",
        1: "Over 30 consecutive access attempts",
        2: "Multiple access attempts within one minute",
        3: "Over 30 consecutive login attempts",
        4: "Multiple login attempts within one minute",
        5: "Multiple login attempts within one hour"
    }
    return type_map.get(alarm_type, f"Unknown ({alarm_type})")


def enhance_audit_data(data, audit_type):
    """Enhance audit data with readable formats"""
    if not data:
        return data
    
    enhanced_data = []
    for item in data:
        enhanced_item = item.copy()
        
        # Convert timestamps - replace original values
        if 'created_at' in enhanced_item:
            enhanced_item['created_at'] = format_timestamp(enhanced_item['created_at'])
        if 'end_time' in enhanced_item:
            enhanced_item['end_time'] = format_timestamp(enhanced_item['end_time'])
        
        # Add type-specific enhancements - replace original values
        if audit_type == 'conn':
            if 'conn_type' in enhanced_item:
                enhanced_item['conn_type'] = get_connection_type_name(enhanced_item['conn_type'])
            else:
                enhanced_item['conn_type'] = "Not Logged In"
        
        elif audit_type == 'console':
            if 'typ' in enhanced_item:
                # Replace typ field with type and convert to readable name
                enhanced_item['type'] = get_console_type_name(enhanced_item['typ'])
                del enhanced_item['typ']
            if 'iop' in enhanced_item:
                # Replace iop field with operation and convert to readable name
                enhanced_item['operation'] = get_console_operation_name(enhanced_item['iop'])
                del enhanced_item['iop']
        
        elif audit_type == 'alarm' and 'typ' in enhanced_item:
            # Replace typ field with type and convert to readable name
            enhanced_item['type'] = get_alarm_type_name(enhanced_item['typ'])
            del enhanced_item['typ']
        
        enhanced_data.append(enhanced_item)
    
    return enhanced_data


def check_response(response):
    """Check API response and return result"""
    if response.status_code == 200:
        try:
            response_json = response.json()
            return response_json
        except ValueError:
            return response.text or "Success"
    else:
        return "Failed", response.status_code, response.text


def view_audits_common(url, token, endpoint, filters=None, page_size=None, current=None, 
                       created_at=None, days_ago=None, non_wildcard_fields=None):
    """Common function for viewing audits"""
    headers = {"Authorization": f"Bearer {token}"}
    
    # Set default page size and current page
    if page_size is None:
        page_size = 10
    if current is None:
        current = 1
    
    params = {
        "pageSize": page_size,
        "current": current
    }
    
    # Add filter parameters if provided
    if filters:
        for key, value in filters.items():
            if value is not None:
                params[key] = value
    
    # Handle time filters
    if days_ago is not None:
        # Calculate datetime from days ago
        target_time = datetime.now() - timedelta(days=days_ago)
        # Convert to UTC time string using system timezone
        utc_timestamp = target_time.timestamp()
        utc_dt = datetime.fromtimestamp(utc_timestamp, timezone.utc)
        params["created_at"] = utc_dt.strftime("%Y-%m-%d %H:%M:%S.000")
    elif created_at:
        # Parse local time string and convert to UTC time string
        utc_time_str = parse_local_time_to_utc_string(created_at)
        if utc_time_str is not None:
            params["created_at"] = utc_time_str
        else:
            # If parsing fails, pass the original value
            params["created_at"] = created_at

    # Apply wildcard patterns for string fields (excluding specific fields)
    if non_wildcard_fields is None:
        non_wildcard_fields = set()
    
    # Always exclude these fields from wildcard treatment
    non_wildcard_fields.update(["created_at", "pageSize", "current"])
    
    string_params = {}
    for k, v in params.items():
        if isinstance(v, str) and k not in non_wildcard_fields:
            if v != "-" and "%" not in v:
                string_params[k] = "%" + v + "%"
            else:
                string_params[k] = v
        else:
            string_params[k] = v

    response = requests.get(f"{url}/api/audits/{endpoint}", headers=headers, params=string_params)
    response_json = response.json()
    
    # Enhance the data with readable formats
    data = enhance_audit_data(response_json.get("data", []), endpoint)
    
    return {
        "data": data,
        "total": response_json.get("total", 0),
        "current": current,
        "pageSize": page_size
    }


def view_conn_audits(url, token, remote=None, conn_type=None, 
                     page_size=None, current=None, created_at=None, days_ago=None):
    """View connection audits"""
    filters = {
        "remote": remote,
        "conn_type": conn_type
    }
    non_wildcard_fields = {"conn_type"}
    
    return view_audits_common(
        url, token, "conn", filters, page_size, current, created_at, days_ago, non_wildcard_fields
    )


def view_file_audits(url, token, remote=None,
                     page_size=None, current=None, created_at=None, days_ago=None):
    """View file audits"""
    filters = {
        "remote": remote
    }
    non_wildcard_fields = set()
    
    return view_audits_common(
        url, token, "file", filters, page_size, current, created_at, days_ago, non_wildcard_fields
    )


def view_alarm_audits(url, token, device=None,
                      page_size=None, current=None, created_at=None, days_ago=None):
    """View alarm audits"""
    filters = {
        "device": device
    }
    non_wildcard_fields = set()
    
    return view_audits_common(
        url, token, "alarm", filters, page_size, current, created_at, days_ago, non_wildcard_fields
    )


def view_console_audits(url, token, operator=None,
                        page_size=None, current=None, created_at=None, days_ago=None):
    """View console audits"""
    filters = {
        "operator": operator
    }
    non_wildcard_fields = set()
    
    return view_audits_common(
        url, token, "console", filters, page_size, current, created_at, days_ago, non_wildcard_fields
    )


def main():
    parser = argparse.ArgumentParser(description="Audits manager")
    parser.add_argument(
        "command",
        choices=["view-conn", "view-file", "view-alarm", "view-console"],
        help="Command to execute",
    )
    parser.add_argument("--url", required=True, help="URL of the API")
    parser.add_argument("--token", required=True, help="Bearer token for authentication")
    
    # Pagination parameters
    parser.add_argument("--page-size", type=int, default=10, help="Number of records per page (default: 10)")
    parser.add_argument("--current", type=int, default=1, help="Current page number (default: 1)")
    
    # Time filtering parameters
    parser.add_argument("--created-at", help="Filter by creation time in local time (format: 2025-09-16 14:15:57 or 2025-09-16 14:15:57.000)")
    parser.add_argument("--days-ago", type=int, help="Filter by days ago (e.g., 7 for last 7 days)")
    
    # Audit filters (simplified)
    parser.add_argument("--remote", help="Remote peer ID filter (for conn/file audits)")
    parser.add_argument("--device", help="Device ID filter (for alarm audits)")
    parser.add_argument("--conn-type", type=int, help="Connection type filter (for conn audits only): 0=Remote Desktop, 1=File Transfer, 2=Port Transfer, 3=View Camera, 4=Terminal")
    parser.add_argument("--operator", help="Operator filter (for console audits only)")

    args = parser.parse_args()

    # Remove trailing slashes from URL
    while args.url.endswith("/"):
        args.url = args.url[:-1]

    if args.command == "view-conn":
        # View connection audits
        result = view_conn_audits(
            args.url, 
            args.token, 
            args.remote, 
            args.conn_type,
            args.page_size,
            args.current,
            args.created_at,
            args.days_ago
        )
        print(json.dumps(result, indent=2))
    
    elif args.command == "view-file":
        # View file audits
        result = view_file_audits(
            args.url, 
            args.token, 
            args.remote,
            args.page_size,
            args.current,
            args.created_at,
            args.days_ago
        )
        print(json.dumps(result, indent=2))
    
    elif args.command == "view-alarm":
        # View alarm audits
        result = view_alarm_audits(
            args.url, 
            args.token, 
            args.device,
            args.page_size,
            args.current,
            args.created_at,
            args.days_ago
        )
        print(json.dumps(result, indent=2))
    
    elif args.command == "view-console":
        # View console audits
        result = view_console_audits(
            args.url, 
            args.token, 
            args.operator,
            args.page_size,
            args.current,
            args.created_at,
            args.days_ago
        )
        print(json.dumps(result, indent=2))


if __name__ == "__main__":
    main()
