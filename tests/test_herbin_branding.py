#!/usr/bin/env python3
import plistlib
import re
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
LEFT_MOUSE_DOWN = "allow_err!(en.mouse_down(MouseButton::Left));"
WATCHER_NAME_FRAGMENTS = ("watch", "notify", "fsevent")
CANONICAL_LEFT_MOUSE_ARM = """
MOUSE_BUTTON_LEFT => {
    #[cfg(target_os = "macos")]
    if let Some((x, y)) = crate::get_cursor_pos() {
        crate::window_targeting::preprocess_remote_left_click(x, y);
    }
    allow_err!(en.mouse_down(MouseButton::Left));
}
"""
COLLECTOR_EARLY_EXCLUSION = re.compile(
    r"\bif\s*\((?P<condition>(?:[^()]|\([^()]*\))*)\)\s*"
    r"(?:\{\s*)?(?:continue\s*;|break\s*;|return\b[^;]*;)"
)
RAW_STRING_START = re.compile(r'(?:br|r)(?P<hashes>#{0,})"')
CHARACTER_LITERAL = re.compile(
    r"'(?:\\(?:x[\dA-Fa-f]{2}|u\{[\dA-Fa-f_]+\}|.)|[^\\'\n])'"
)


def read(relative_path: str) -> str:
    return (ROOT / relative_path).read_text(encoding="utf-8")


def assert_in_order(text: str, first: str, second: str) -> None:
    first_index = text.index(first)
    second_index = text.index(second)
    assert first_index < second_index, f"expected {first!r} before {second!r}"


def sanitized_source(source: str, *, remove_strings: bool = False) -> str:
    """Blank active-source comments/literals while preserving offsets/newlines."""
    output = list(source)
    cursor = 0
    while cursor < len(source):
        end = None
        blank = True
        if source.startswith("//", cursor):
            end = source.find("\n", cursor + 2)
            end = len(source) if end == -1 else end
        elif source.startswith("/*", cursor):
            depth = 1
            end = cursor + 2
            while end < len(source) and depth:
                if source.startswith("/*", end):
                    depth += 1
                    end += 2
                elif source.startswith("*/", end):
                    depth -= 1
                    end += 2
                else:
                    end += 1
        else:
            raw = RAW_STRING_START.match(source, cursor)
            if raw and (
                cursor == 0 or not re.match(r"[\w]", source[cursor - 1])
            ):
                terminator = '"' + raw.group("hashes")
                closing = source.find(terminator, raw.end())
                end = len(source) if closing == -1 else closing + len(terminator)
                blank = remove_strings
            elif source[cursor] == '"':
                end = cursor + 1
                while end < len(source):
                    if source[end] == "\\":
                        end += 2
                    elif source[end] == '"':
                        end += 1
                        break
                    else:
                        end += 1
                blank = remove_strings
            else:
                character = CHARACTER_LITERAL.match(source, cursor)
                if character:
                    end = character.end()
                    blank = remove_strings

        if end is None:
            cursor += 1
            continue
        if blank:
            output[cursor:end] = [
                "\n" if char == "\n" else " " for char in output[cursor:end]
            ]
        cursor = end
    return "".join(output)


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
    assert sanitized_source(input_service_rs, remove_strings=True).count(LEFT_MOUSE_DOWN) == 1
    active_arm = down_code[left_arm.start() : arm_close + 1]
    actual_arm = re.sub(r"\s+", "", sanitized_source(active_arm))
    assert actual_arm == re.sub(r"\s+", "", CANONICAL_LEFT_MOUSE_ARM)


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


def assert_candidate_collector_has_no_blanket_filters(macos_mm: str) -> None:
    active_macos_mm = sanitized_source(macos_mm)
    string_literals = re.findall(r'@?"((?:\\.|[^"\\])*)"', active_macos_mm)
    assert all("dock" not in literal.lower() for literal in string_literals)

    _, collector_structure = braced_body(
        macos_mm, 'extern "C" int32_t MacCollectWindowCandidatesAtPoint'
    )
    for exclusion in COLLECTOR_EARLY_EXCLUSION.finditer(collector_structure):
        condition = exclusion.group("condition")
        assert not re.search(r"\blayer\b|kCGWindowLayer", condition, re.IGNORECASE)


def dependency_names(cargo_toml: str) -> set[str]:
    names: set[str] = set()
    active_dependency_section = False
    dependency_table = re.compile(
        r"^(?:target\.(?:'[^']+'|\"[^\"]+\")\.)?"
        r"(?:dependencies|dev-dependencies|build-dependencies)"
        r"(?:\.([\w-]+))?$", re.IGNORECASE
    )

    for raw_line in cargo_toml.splitlines():
        line = raw_line.strip()
        if not line or line.startswith("#"):
            continue
        section = re.fullmatch(r"\[(.+)\]", line)
        if section:
            section_name = section.group(1).strip().lower()
            dependency_section = dependency_table.fullmatch(section_name)
            table_dependency = dependency_section.group(1) if dependency_section else None
            if table_dependency:
                names.add(table_dependency)
            active_dependency_section = bool(dependency_section and not table_dependency)
            continue
        if not active_dependency_section:
            continue

        dependency = re.match(r'(?:"([^"]+)"|([\w-]+))\s*=', line)
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


def assert_source_sanitizer_contract() -> None:
    fixture = r'''
fn fixture() {
    let normal = "/* normal string } */";
    let raw = r###"// raw string }"###;
    let rust_char = '{'; if (objc_value == '}') { active(); }
    // line comment }
    /* outer block /* nested block } */ still a comment { */
}
'''
    comment_free = sanitized_source(fixture)
    for literal in ('"/* normal string } */"', 'r###"// raw string }"###'):
        assert literal in comment_free
    assert "line comment" not in comment_free
    structure = sanitized_source(fixture, remove_strings=True)
    assert structure.count("{") == structure.count("}") == 2
    braced_body(fixture, "fn fixture")


def replace_first(source: str, old: str, new: str) -> str:
    assert old in source, f"missing mutation target: {old!r}"
    return source.replace(old, new, 1)


def assert_rejected(check, *arguments) -> None:
    try:
        check(*arguments)
    except AssertionError:
        return
    raise AssertionError(f"{check.__name__} accepted a forbidden mutation")


def assert_replacements_rejected(
    check, source: str, old: str, replacements: tuple[str, ...], *extra
) -> None:
    for replacement in replacements:
        assert_rejected(check, replace_first(source, old, replacement), *extra)


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
        "MOUSE_TYPE_DOWN => match buttons {\n            MOUSE_BUTTON_LEFT => {"
    )
    assert_replacements_rejected(
        assert_left_mouse_down_contract, input_service_rs, down_left_arm,
        tuple(
            down_left_arm.replace("=> {", replacement)
            for replacement in ("=> if false {", "if false => {")
        ),
    )
    assert_replacements_rejected(
        assert_left_mouse_down_contract, input_service_rs, LEFT_MOUSE_DOWN,
        tuple(
            prefix + LEFT_MOUSE_DOWN
            for prefix in ("#[cfg(any())]\n                ", "return;\n                ")
        ),
    )

    registration = '#[cfg(target_os = "macos")]\npub(crate) mod window_targeting;'
    assert_replacements_rejected(
        assert_window_targeting_module_registered,
        lib_rs,
        registration,
        (
            f"/* {registration} */",
            f"/* outer /* nested */\n{registration}\n*/",
        ),
    )
    assert_replacements_rejected(
        assert_default_window_targeting_config, window_targeting_config_rs,
        "diagnostics = false",
        ("diagnostics = true",),
    )

    record_marker = "MacWindowCandidateRecord *record = &records[recordCount];"
    collector_filters = (
        "if (layer != nil && layer.intValue) { continue; }",
        "if ([window objectForKey:(id)kCGWindowLayer].intValue) { continue; }",
        'if ([ownerApplication.bundleIdentifier hasPrefix:@"com.apple.dock"]) '
        "{ continue; }",
    )
    assert_replacements_rejected(
        assert_candidate_collector_has_no_blanket_filters,
        macos_mm,
        record_marker,
        tuple(
            f"{filter_mutation}\n            {record_marker}"
            for filter_mutation in collector_filters
        ),
    )

    dock_helper = (
        'static bool MacWindowIsDock(NSRunningApplication *app) {\n'
        ' return [app.bundleIdentifier isEqualToString:@"com.apple.dock"]; }\n'
    )
    helper_mutation = replace_first(
        macos_mm, 'extern "C" int32_t MacCollectWindowCandidatesAtPoint',
        dock_helper + 'extern "C" int32_t MacCollectWindowCandidatesAtPoint',
    )
    helper_mutation = replace_first(
        helper_mutation, record_marker,
        "if (MacWindowIsDock(ownerApplication)) { continue; }\n"
        f"            {record_marker}",
    )
    assert_rejected(
        assert_candidate_collector_has_no_blanket_filters, helper_mutation
    )

    assert_replacements_rejected(
        assert_no_watcher_dependencies, cargo_toml, "[dependencies]\n",
        (
            '[dependencies]\nwatchexec = "4"\n',
            '[dependencies.watchexec]\nversion = "4"\n\n[dependencies]\n',
            "[target.'cfg(target_os = \"macos\")'.dependencies.fsevent]\n"
            'version = "2"\n\n[dependencies]\n',
        ),
    )

    for watcher_api in (
        "fn watch_config() { notify::recommended_watcher(); }",
        "fn reload() { fsevent::listen(); }",
    ):
        assert_rejected(
            assert_no_watcher_markers,
            window_targeting_rs + "\n" + watcher_api,
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
    headless_root_rs = read("src/headless_terminal.rs")
    headless_args_rs = read("src/headless_terminal/args.rs")
    headless_tty_rs = read("src/headless_terminal/tty.rs")
    headless_handler_rs = read("src/headless_terminal/handler.rs")
    headless_runtime_rs = read("src/headless_terminal/runtime.rs")
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

    assert_source_sanitizer_contract()
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

    assert 'mod headless_terminal;' in lib_rs
    assert '"--headless"' in headless_args_rs
    assert '"--persistent"' in headless_args_rs
    assert '"--password"' in headless_args_rs
    assert "send_terminal_input_bytes" in headless_runtime_rs
    assert "LocalTtyGuard" in headless_tty_rs
    assert "SignalKind::window_change" in headless_tty_rs
    assert "HeadlessTerminalHandler" in headless_handler_rs
    assert "core_main_invoke_new_connection" in core_main_rs
    assert "terminal_persistent" in headless_runtime_rs
    assert "toggle_option" not in headless_runtime_rs
    assert "PeerConfig::store" not in headless_runtime_rs
    assert_in_order(
        core_main_rs,
        "should_dispatch_flutter_connection(&args",
        "core_main_invoke_new_connection",
    )
    assert_in_order(
        core_main_rs,
        "hbb_common::init_log",
        "headless_terminal::run_cli(&args)",
    )

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
    assert "proc_pid_rusage" in memory_watchdog_rs
    assert "RUSAGE_INFO_V0" in memory_watchdog_rs
    assert "phys_footprint" in memory_watchdog_rs
    assert "current_rss_bytes" not in memory_watchdog_rs
    assert "sysinfo::" not in memory_watchdog_rs
    assert "rss=" not in memory_watchdog_rs
    assert "RSS" not in memory_watchdog_rs
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
    assert "cargo test --locked --lib headless_terminal" in macos_workflow


if __name__ == "__main__":
    main()
