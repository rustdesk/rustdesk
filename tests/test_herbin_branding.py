#!/usr/bin/env python3
import plistlib
import re
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
LEFT_MOUSE_DOWN = "allow_err!(en.mouse_down(MouseButton::Left));"
LEFT_CLICK_PREPROCESSOR = "preprocess_remote_left_click(x, y)"
WATCHER_NAME_FRAGMENTS = ("watch", "notify")
SOURCE_TOKEN = re.compile(
    r'(?P<raw>r#".*?"#)|(?P<string>"(?:\\.|[^"\\])*")|'
    r"(?P<line>//[^\n]*)|(?P<block>/\*.*?\*/)",
    re.DOTALL,
)


def read(relative_path: str) -> str:
    return (ROOT / relative_path).read_text(encoding="utf-8")


def assert_in_order(text: str, first: str, second: str) -> None:
    first_index = text.index(first)
    second_index = text.index(second)
    assert first_index < second_index, f"expected {first!r} before {second!r}"


def sanitized_source(source: str, *, remove_strings: bool = False) -> str:
    """Blank comments and optionally strings while preserving offsets/newlines."""
    def replacement(match: re.Match) -> str:
        is_string = match.lastgroup in ("raw", "string")
        if is_string and not remove_strings:
            return match.group(0)
        return "".join("\n" if char == "\n" else " " for char in match.group(0))

    return SOURCE_TOKEN.sub(replacement, source)


def matching_delimiter(
    structure: str, open_index: int, opener: str, closer: str
) -> int:
    assert structure[open_index] == opener
    depth = 0
    for index in range(open_index, len(structure)):
        character = structure[index]
        if character == opener:
            depth += 1
        elif character == closer:
            depth -= 1
            if depth == 0:
                return index
    raise AssertionError(f"unclosed {opener!r} at offset {open_index}")


def braced_body(source: str, anchor: str) -> tuple[str, str]:
    comment_free = sanitized_source(source)
    structure = sanitized_source(source, remove_strings=True)
    anchor_index = comment_free.find(anchor)
    assert anchor_index != -1, f"missing anchor {anchor!r}"
    open_brace = structure.find("{", anchor_index + len(anchor))
    assert open_brace != -1, f"missing body for {anchor!r}"
    close_brace = matching_delimiter(structure, open_brace, "{", "}")
    return (
        comment_free[open_brace + 1 : close_brace],
        structure[open_brace + 1 : close_brace],
    )


def assert_window_targeting_module_registered(lib_rs: str) -> None:
    comment_free = sanitized_source(lib_rs)
    registration = re.compile(
        r'(?m)^\s*#\[\s*cfg\(\s*target_os\s*=\s*"macos"\s*\)\s*\]\s*\n'
        r"\s*pub\(crate\)\s+mod\s+window_targeting\s*;"
    )
    assert len(registration.findall(comment_free)) == 1


def assert_left_mouse_down_contract(input_service_rs: str) -> None:
    structure = sanitized_source(input_service_rs, remove_strings=True)
    down_code, down_structure = braced_body(
        input_service_rs, "MOUSE_TYPE_DOWN => match buttons"
    )

    left_tokens = list(re.finditer(r"\bMOUSE_BUTTON_LEFT\b", down_structure))
    assert len(left_tokens) == 1
    left_arm = re.search(r"\bMOUSE_BUTTON_LEFT\s*=>\s*\{", down_structure)
    assert left_arm is not None
    assert left_arm.start() == left_tokens[0].start()

    arm_open = down_structure.find("{", left_arm.start())
    arm_close = matching_delimiter(down_structure, arm_open, "{", "}")
    arm_code = down_code[arm_open + 1 : arm_close]
    arm_structure = down_structure[arm_open + 1 : arm_close]

    assert structure.count(LEFT_MOUSE_DOWN) == 1
    assert arm_structure.count(LEFT_MOUSE_DOWN) == 1
    assert arm_structure.count(LEFT_CLICK_PREPROCESSOR) == 1
    preprocessor_index = arm_structure.index(LEFT_CLICK_PREPROCESSOR)
    mouse_down_index = arm_structure.index(LEFT_MOUSE_DOWN)
    assert preprocessor_index < mouse_down_index
    prefix = arm_structure[:mouse_down_index]
    assert prefix.count("{") == prefix.count("}")

    line_start = arm_code.rfind("\n", 0, mouse_down_index) + 1
    line_end = arm_code.find("\n", mouse_down_index)
    line_end = len(arm_code) if line_end == -1 else line_end
    assert arm_code[line_start:line_end].strip() == LEFT_MOUSE_DOWN


def default_template_fields(document: str) -> dict[str, object]:
    fields: dict[str, object] = {}
    for key, raw_value in re.findall(
        r"(?m)^\s*(version|mode|diagnostics)\s*=\s*(\S+)\s*$", document
    ):
        assert key not in fields
        if raw_value in ("true", "false"):
            fields[key] = raw_value == "true"
        elif raw_value.isdigit():
            fields[key] = int(raw_value)
        else:
            assert raw_value.startswith('"') and raw_value.endswith('"')
            fields[key] = raw_value[1:-1]
    assert set(fields) == {"version", "mode", "diagnostics"}
    return fields


def assert_default_window_targeting_config(config_rs: str) -> None:
    comment_free = sanitized_source(config_rs)
    template = re.search(
        r'pub\s+const\s+DEFAULT_TEMPLATE\s*:\s*&str\s*=\s*r(?P<hashes>#{0,})"'
        r'(?P<body>.*?)"(?P=hashes)\s*;',
        comment_free,
        re.DOTALL,
    )
    assert template is not None
    parsed_template = default_template_fields(template.group("body"))
    assert parsed_template.get("diagnostics") is False

    user_config, _ = braced_body(config_rs, "struct UserConfig")
    assert re.search(
        r"#\[\s*serde\(\s*default\s*\)\s*\]\s*diagnostics\s*:\s*bool",
        user_config,
    )


def if_statements(structure: str):
    for match in re.finditer(r"\bif\s*\(", structure):
        open_parenthesis = structure.find("(", match.start())
        close_parenthesis = matching_delimiter(
            structure, open_parenthesis, "(", ")"
        )
        cursor = close_parenthesis + 1
        while cursor < len(structure) and structure[cursor].isspace():
            cursor += 1
        if cursor < len(structure) and structure[cursor] == "{":
            body_end = matching_delimiter(structure, cursor, "{", "}")
            body = structure[cursor + 1 : body_end]
        else:
            body_end = structure.find(";", cursor)
            assert body_end != -1, "if statement has no body"
            body = structure[cursor : body_end + 1]
        yield structure[open_parenthesis + 1 : close_parenthesis], body


def assert_candidate_collector_has_no_blanket_filters(macos_mm: str) -> None:
    collector, collector_structure = braced_body(
        macos_mm, 'extern "C" int32_t MacCollectWindowCandidatesAtPoint'
    )
    string_literals = re.findall(r'@?"((?:\\.|[^"\\])*)"', collector)
    assert all("dock" not in literal.lower() for literal in string_literals)

    for condition, body in if_statements(collector_structure):
        layer_condition = re.search(r"\blayer\b", condition, re.IGNORECASE)
        early_exclusion = re.search(r"\b(?:continue|break|return)\b", body)
        assert not (layer_condition and early_exclusion)


def dependency_names(cargo_toml: str) -> set[str]:
    names: set[str] = set()
    active_dependency_section = False

    for raw_line in cargo_toml.splitlines():
        line = raw_line.strip()
        if not line or line.startswith("#"):
            continue
        section = re.fullmatch(r"\[(.+)\]", line)
        if section:
            section_name = section.group(1).strip().lower()
            active_dependency_section = any(
                section_name == dependency_section
                or section_name.endswith(f".{dependency_section}")
                for dependency_section in (
                    "dependencies",
                    "dev-dependencies",
                    "build-dependencies",
                )
            )
            continue
        if not active_dependency_section:
            continue

        dependency = re.match(
            r'(?:"([^"]+)"|([A-Za-z0-9_-]+))\s*=', line
        )
        if dependency is None:
            continue
        names.add(dependency.group(1) or dependency.group(2))
        package = re.search(r'\bpackage\s*=\s*"([^"]+)"', line)
        if package:
            names.add(package.group(1))

    return names


def assert_no_watcher_dependencies(cargo_toml: str) -> None:
    for dependency_name in dependency_names(cargo_toml):
        normalized = dependency_name.lower().replace("-", "_")
        assert not any(
            fragment in normalized for fragment in WATCHER_NAME_FRAGMENTS
        ), f"file watcher dependency is forbidden: {dependency_name}"


def assert_no_watcher_markers(*targeting_sources: str) -> None:
    for source in targeting_sources:
        for identifier in re.findall(
            r"\b[A-Za-z_][A-Za-z0-9_]*\b",
            sanitized_source(source, remove_strings=True),
        ):
            normalized = identifier.lower()
            assert not any(
                fragment in normalized for fragment in WATCHER_NAME_FRAGMENTS
            ), f"file watcher marker is forbidden: {identifier}"


def replace_first(source: str, old: str, new: str) -> str:
    assert old in source, f"missing mutation target: {old!r}"
    return source.replace(old, new, 1)


def assert_rejected(check, *arguments) -> None:
    try:
        check(*arguments)
    except AssertionError:
        return
    raise AssertionError(f"{check.__name__} accepted a forbidden mutation")


def rejected_mutation(check, source: str, old: str, new: str, *extra) -> str:
    mutation = replace_first(source, old, new)
    assert_rejected(check, mutation, *extra)
    return mutation


def old_left_mouse_down_check_accepts(source: str) -> bool:
    suffix = (
        "\n                allow_err!(en.mouse_down(MouseButton::Left));\n"
        "            }\n"
        "            MOUSE_BUTTON_RIGHT =>"
    )
    return (
        LEFT_CLICK_PREPROCESSOR in source
        and source.index(LEFT_CLICK_PREPROCESSOR)
        < source.index("en.mouse_down(MouseButton::Left)")
        and suffix in source
        and source.count("en.mouse_down(MouseButton::Left)") == 1
    )


def run_contract_mutation_regressions(
    cargo_toml: str,
    lib_rs: str,
    input_service_rs: str,
    window_targeting_rs: str,
    window_targeting_config_rs: str,
    window_targeting_rules_rs: str,
    macos_mm: str,
) -> None:
    down_left_arm = (
        "MOUSE_TYPE_DOWN => match buttons {\n"
        "            MOUSE_BUTTON_LEFT => {"
    )
    for replacement in ("=> if false {", "if false => {"):
        mutation = rejected_mutation(
            assert_left_mouse_down_contract,
            input_service_rs,
            down_left_arm,
            down_left_arm.replace("=> {", replacement),
        )
        assert old_left_mouse_down_check_accepts(mutation)

    registration = '#[cfg(target_os = "macos")]\npub(crate) mod window_targeting;'
    commented_registration = rejected_mutation(
        assert_window_targeting_module_registered,
        lib_rs,
        registration,
        f"/* {registration} */",
    )
    assert registration in commented_registration

    diagnostics_true = rejected_mutation(
        assert_default_window_targeting_config,
        window_targeting_config_rs,
        "diagnostics = false",
        "diagnostics = true",
    )
    assert "diagnostics = false" in diagnostics_true

    record_marker = "MacWindowCandidateRecord *record = &records[recordCount];"
    for filter_mutation in (
        "if (layer != nil && layer.intValue) { continue; }",
        'if ([ownerApplication.bundleIdentifier hasPrefix:@"com.apple.dock"]) '
        "{ continue; }",
    ):
        mutated_collector = rejected_mutation(
            assert_candidate_collector_has_no_blanket_filters,
            macos_mm,
            record_marker,
            f"{filter_mutation}\n            {record_marker}",
        )
        assert "layer.intValue != 0" not in mutated_collector
        assert 'bundleIdentifier isEqualToString:@"com.apple.dock"' not in (
            mutated_collector
        )

    watcher_dependency = rejected_mutation(
        assert_no_watcher_dependencies,
        cargo_toml,
        "[dependencies]\n",
        '[dependencies]\nwatchexec = "4"\n',
    )
    assert "\nnotify" not in watcher_dependency.lower()
    watcher_api = (
        window_targeting_rs
        + "\nfn watch_config() { notify::recommended_watcher(); }\n"
    )
    assert_rejected(
        assert_no_watcher_markers,
        watcher_api,
        window_targeting_config_rs,
        window_targeting_rules_rs,
    )


def main() -> None:
    cargo_toml = read("Cargo.toml")
    common_rs = read("src/common.rs")
    main_rs = read("src/main.rs")
    core_main_rs = read("src/core_main.rs")
    flutter_rs = read("src/flutter.rs")
    flutter_ffi_rs = read("src/flutter_ffi.rs")
    lib_rs = read("src/lib.rs")
    service_rs = read("src/service.rs")
    keyboard_rs = read("src/keyboard.rs")
    input_service_rs = read("src/server/input_service.rs")
    server_rs = read("src/server.rs")
    memory_watchdog_rs = read("src/server/memory_watchdog.rs")
    window_targeting_rs = read("src/window_targeting.rs")
    window_targeting_config_rs = read("src/window_targeting/config.rs")
    window_targeting_rules_rs = read("src/window_targeting/rules.rs")
    mac_agent_plist = plistlib.loads(
        (ROOT / "src/platform/privileges_scripts/agent.plist").read_bytes()
    )
    macos_rs = read("src/platform/macos.rs")
    macos_mm = read("src/platform/macos.mm")
    mac_install_script = read("src/platform/privileges_scripts/install.scpt")
    mac_update_script = read("src/platform/privileges_scripts/update.scpt")
    mac_app_info = read("flutter/macos/Runner/Configs/AppInfo.xcconfig")
    mac_info_plist = read("flutter/macos/Runner/Info.plist")
    mac_project = read("flutter/macos/Runner.xcodeproj/project.pbxproj")
    mac_scheme = read(
        "flutter/macos/Runner.xcodeproj/xcshareddata/xcschemes/Runner.xcscheme"
    )
    build_py = read("build.py")
    osx_dist = read("res/osx-dist.sh")
    macos_workflow = read(".github/workflows/codex-macos-herbin.yml")

    run_contract_mutation_regressions(
        cargo_toml,
        lib_rs,
        input_service_rs,
        window_targeting_rs,
        window_targeting_config_rs,
        window_targeting_rules_rs,
        macos_mm,
    )

    assert 'ProductName = "RustDesk-Herbin"' in cargo_toml
    assert 'OriginalFilename = "rustdesk-herbin.exe"' in cargo_toml
    assert 'name = "RustDesk-Herbin"' in cargo_toml
    assert 'identifier = "com.herbin.rustdesk"' in cargo_toml
    assert 'pub const FORK_APP_NAME: &str = "RustDesk-Herbin";' in common_rs
    assert 'pub const FORK_ORG: &str = "com.herbin";' in common_rs
    assert "pub fn apply_fork_identity()" in common_rs
    assert "common::apply_fork_identity();" in main_rs
    assert "crate::common::apply_fork_identity();" in core_main_rs
    assert "crate::common::apply_fork_identity();" in flutter_rs
    assert "crate::common::apply_fork_identity();" in flutter_ffi_rs
    assert "crate::common::apply_fork_identity();" in service_rs

    assert "if is_custom_client() {\n        return;\n    }" in common_rs

    removed_keymap_markers = (
        "HERBIN_MACOS_KEYMAP",
        "HerbinMacosKeymap",
        "MacosShortcutRemapState",
        "try_remap_macos_shortcut",
        "herbin-keymap.json",
        "herbin-macos-keymap",
        "remap_shortcut_for_peer",
    )
    for marker in removed_keymap_markers:
        assert marker not in input_service_rs
        assert marker not in keyboard_rs

    assert_window_targeting_module_registered(lib_rs)
    for operation in ("status", "validate", "reload"):
        assert f'[operation] if operation == "{operation}"' in window_targeting_rs
    assert "Passthrough" in window_targeting_config_rs
    assert '"mode.passthrough"' in window_targeting_rs

    assert "fn MacCollectWindowCandidatesAtPoint" in macos_rs
    assert "fn MacActivateWindowCandidateAtPoint" in macos_rs
    assert "pub fn collect_window_candidates_at_point" in macos_rs
    assert "pub fn activate_window_candidate_at_point" in macos_rs
    assert "MacCollectWindowCandidatesAtPoint" in macos_mm
    assert "MacActivateWindowCandidateAtPoint" in macos_mm
    assert "const MAX_MAC_WINDOW_CANDIDATES: usize = 64;" in macos_rs
    assert "static constexpr size_t MAX_MAC_WINDOW_CANDIDATES = 64;" in macos_mm
    assert_candidate_collector_has_no_blanket_filters(macos_mm)
    assert "MAX_MAC_WINDOW_CANDIDATES" in macos_rs
    assert "NSApplicationActivationPolicyRegular" in macos_mm
    assert "activateWithOptions" in macos_mm
    required_window_activation_markers = (
        "#import <ApplicationServices/ApplicationServices.h>",
        "AXUIElementCopyElementAtPosition",
        "kAXRoleAttribute",
        "kAXWindowRole",
        "kAXWindowAttribute",
        "AXUIElementGetPid",
        "kAXFocusedWindowAttribute",
        "CFEqual",
        "AXUIElementPerformAction",
        "kAXRaiseAction",
    )
    for marker in required_window_activation_markers:
        assert marker in macos_mm

    private_window_api_markers = (
        "_AXUIElementGetWindow",
        "CGSOrderWindow",
        "SkyLight",
    )
    mac_window_sources = macos_rs + macos_mm
    for marker in private_window_api_markers:
        assert marker not in mac_window_sources

    for rule_id in (
        "builtin.dock-ui",
        "builtin.interactive-transient",
        "builtin.notification-center-overlay",
    ):
        assert rule_id in window_targeting_rules_rs

    assert_left_mouse_down_contract(input_service_rs)
    assert_default_window_targeting_config(window_targeting_config_rs)
    assert "window title" not in window_targeting_rs.lower()

    assert_no_watcher_dependencies(cargo_toml)
    assert_no_watcher_markers(
        window_targeting_rs,
        window_targeting_config_rs,
        window_targeting_rules_rs,
    )

    assert_in_order(
        macos_mm,
        "AXUIElementCopyElementAtPosition",
        "activateWithOptions",
    )
    assert_in_order(
        macos_mm,
        "activateWithOptions",
        "AXUIElementPerformAction",
    )
    assert 'mod memory_watchdog;' in server_rs
    assert 'memory_watchdog::start();' in server_rs
    assert '"rdh-memory-restart-threshold-mib"' in memory_watchdog_rs
    assert 'std::env::var("XPC_SERVICE_NAME")' in memory_watchdog_rs
    assert "DAILY_CHECK_HOUR: u32 = 6" in memory_watchdog_rs
    assert "UNATTENDED_WINDOW_START_HOUR: u32 = 0" in memory_watchdog_rs
    assert "UNATTENDED_WINDOW_END_HOUR: u32 = 7" in memory_watchdog_rs
    assert "Connection::alive_conns" not in memory_watchdog_rs
    assert "std::process::exit(RESTART_EXIT_CODE)" in memory_watchdog_rs
    assert mac_agent_plist["RunAtLoad"] is True
    assert mac_agent_plist["KeepAlive"] is True

    diagnostic_markers = (
        "macos-input-trace",
        "macos-focus-trace",
        "log_macos_click_focus",
        "mouse_focus_snapshot",
        "from_millis(120)",
        "rdh-window-debug",
        "MacDebugFrontmostApplicationPid",
        "MacDebugWindowOwnerPidAtPoint",
        "rdh-window-activation.log",
        "settled_150ms",
        "settled_600ms",
    )
    diagnostic_sources = input_service_rs + macos_rs + macos_mm + read(
        "libs/enigo/src/macos/macos_impl.rs"
    )
    for marker in diagnostic_markers:
        assert marker not in diagnostic_sources

    assert "PRODUCT_NAME = RustDesk-Herbin" in mac_app_info
    assert "PRODUCT_BUNDLE_IDENTIFIER = com.herbin.rustdesk" in mac_app_info
    assert "<string>com.herbin.rustdesk</string>" in mac_info_plist
    assert "<string>rustdesk-herbin</string>" in mac_info_plist
    assert "RustDesk-Herbin.app" in mac_project
    assert "PRODUCT_BUNDLE_IDENTIFIER = com.herbin.rustdesk;" in mac_project
    assert 'BuildableName = "RustDesk-Herbin.app"' in mac_scheme

    assert 'launchctl bootstrap gui/$uid ' in mac_install_script
    assert 'launchctl kickstart -k gui/$uid/$agent_label' in mac_install_script
    assert "legacy_agent_plist" in mac_install_script
    assert "bad_agent_plist" in mac_install_script
    assert "legacy_daemon_plist" in mac_update_script
    assert "bad_daemon_plist" in mac_update_script

    for packaging_file in (build_py, osx_dist, macos_workflow):
        assert "RustDesk-Herbin.app" in packaging_file
        assert "rustdesk-herbin-" in packaging_file

    assert "Ad-hoc sign app (not notarized)" in macos_workflow
    assert "tests/test_herbin_branding.py" in macos_workflow
    assert "source_ref:" in macos_workflow
    assert "ref: ${{ inputs.source_ref }}" in macos_workflow
    assert "RDH_REVISION" in macos_workflow
    assert "shasum -a 256" in macos_workflow


if __name__ == "__main__":
    main()
