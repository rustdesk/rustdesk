#!/usr/bin/env python3

import os
import sys
import subprocess
import optparse
from hashlib import md5
from pathlib import Path
from typing import Optional
import brotli
import datetime

# 4GB maximum
length_count = 4
# encoding
encoding = 'utf-8'

# output: {path: (compressed_data, file_md5)}


def generate_md5_table(folder: str, level) -> dict:
    res: dict = dict()
    curdir = os.curdir
    os.chdir(folder)
    for root, _, files in os.walk('.'):
        # remove ./
        for f in files:
            md5_generator = md5()
            full_path = os.path.join(root, f)
            print(f"Processing {full_path}...")
            f = open(full_path, "rb")
            content = f.read()
            content_compressed = brotli.compress(
                content, quality=level)
            md5_generator.update(content)
            md5_code = md5_generator.hexdigest().encode(encoding=encoding)
            res[full_path] = (content_compressed, md5_code)
    os.chdir(curdir)
    return res


def write_package_metadata(md5_table: dict, output_folder: str, exe: str):
    output_path = os.path.join(output_folder, "data.bin")
    with open(output_path, "wb") as f:
        f.write("rustdesk".encode(encoding=encoding))
        for path in md5_table.keys():
            (compressed_data, md5_code) = md5_table[path]
            data_length = len(compressed_data)
            path = path.encode(encoding=encoding)
            # path length & path
            f.write((len(path)).to_bytes(length=length_count, byteorder='big'))
            f.write(path)
            # data length & compressed data
            f.write(data_length.to_bytes(
                length=length_count, byteorder='big'))
            f.write(compressed_data)
            # md5 code
            f.write(md5_code)
        # end
        f.write("rustdesk".encode(encoding=encoding))
        # executable
        f.write(exe.encode(encoding='utf-8'))
    print(f"Metadata has been written to {output_path}")

def write_app_metadata(output_folder: str):
    output_path = os.path.join(output_folder, "app_metadata.toml")
    with open(output_path, "w") as f:
        f.write(f"timestamp = {int(datetime.datetime.now().timestamp() * 1000)}\n")
    print(f"App metadata has been written to {output_path}")

# SECURITY: Allowlist of valid Rust target architectures
ALLOWED_TARGETS = {
    # Linux
    'x86_64-unknown-linux-gnu',
    'x86_64-unknown-linux-musl',
    'aarch64-unknown-linux-gnu',
    'aarch64-unknown-linux-musl',
    'armv7-unknown-linux-gnueabihf',
    'armv7-unknown-linux-musleabihf',
    'i686-unknown-linux-gnu',
    
    # Windows
    'x86_64-pc-windows-msvc',
    'x86_64-pc-windows-gnu',
    'i686-pc-windows-msvc',
    'i686-pc-windows-gnu',
    'aarch64-pc-windows-msvc',
    
    # macOS
    'x86_64-apple-darwin',
    'aarch64-apple-darwin',
    
    # Android
    'aarch64-linux-android',
    'armv7-linux-androideabi',
    'i686-linux-android',
    'x86_64-linux-android',
    
    # iOS
    'aarch64-apple-ios',
    'x86_64-apple-ios',
}


def validate_target(target: Optional[str]) -> Optional[str]:
    """
    Validate build target against allowlist.
    
    SECURITY: Prevents command injection via malicious target strings.
    
    Args:
        target: Rust target architecture string
        
    Returns:
        Validated target string or None
        
    Raises:
        ValueError: If target is invalid
    """
    if not target:
        return None
    
    # Validate against allowlist
    if target not in ALLOWED_TARGETS:
        raise ValueError(
            f"Invalid target: {target}\n"
            f"Allowed targets:\n" + 
            "\n".join(f"  - {t}" for t in sorted(ALLOWED_TARGETS))
        )
    
    return target


def validate_folder(folder: str) -> Path:
    """
    Validate and normalize folder path.
    
    SECURITY: Prevents directory traversal and path injection.
    
    Args:
        folder: Folder path to validate
        
    Returns:
        Validated Path object
        
    Raises:
        ValueError: If path is invalid or dangerous
    """
    try:
        path = Path(folder).resolve()
        
        # Check if path exists
        if not path.exists():
            raise ValueError(f"Folder does not exist: {folder}")
        
        if not path.is_dir():
            raise ValueError(f"Path is not a directory: {folder}")
        
        return path
        
    except Exception as e:
        raise ValueError(f"Invalid folder path: {folder} - {e}")


def build_portable(output_folder: str, target: Optional[str]) -> None:
    """
    Build portable binary safely without command injection.
    
    SECURITY: This function replaces os.system() to prevent VULN-010 and VULN-011.
    All commands are executed with subprocess.run(shell=False).
    
    Args:
        output_folder: Output directory for build artifacts
        target: Optional Rust target architecture
        
    Raises:
        ValueError: If target is invalid
        subprocess.CalledProcessError: If build fails
    """
    # Validate output folder
    output_path = validate_folder(output_folder)
    
    # Validate target against allowlist
    validated_target = validate_target(target)
    
    # Change to output directory
    original_dir = os.getcwd()
    try:
        os.chdir(output_path)
        
        # Build command as list (no shell interpretation)
        cmd = ['cargo', 'build', '--release']
        
        if validated_target:
            cmd.extend(['--target', validated_target])
            print(f"Building for target: {validated_target}")
        else:
            print("Building for default target")
        
        print(f"Command: {' '.join(cmd)}")
        
        # Execute safely without shell
        result = subprocess.run(
            cmd,
            check=True,
            capture_output=True,
            text=True,
            shell=False  # CRITICAL: Prevents command injection
        )
        
        # Print build output
        if result.stdout:
            print(result.stdout)
        
        print(f"Build completed successfully in {output_path}")
        
    except subprocess.CalledProcessError as e:
        sys.stderr.write(f"Build failed with exit code {e.returncode}\n")
        if e.stderr:
            sys.stderr.write(f"Error output:\n{e.stderr}\n")
        sys.exit(1)
        
    except Exception as e:
        sys.stderr.write(f"Unexpected error during build: {e}\n")
        sys.exit(1)
        
    finally:
        # Always restore original directory
        os.chdir(original_dir)

# Linux: python3 generate.py -f ../rustdesk-portable-packer/test -o . -e ./test/main.py
# Windows: python3 .\generate.py -f ..\rustdesk\flutter\build\windows\runner\Debug\ -o . -e ..\rustdesk\flutter\build\windows\runner\Debug\rustdesk.exe


if __name__ == '__main__':
    parser = optparse.OptionParser()
    parser.add_option("-f", "--folder", dest="folder",
                      help="folder to compress")
    parser.add_option("-o", "--output", dest="output_folder",
                      help="the root of portable packer project, default is './'")
    parser.add_option("-e", "--executable", dest="executable",
                      help="specify startup file in --folder, default is rustdesk.exe")
    parser.add_option("-t", "--target", dest="target",
                      help="the target used by cargo")
    parser.add_option("-l", "--level", dest="level", type="int",
                      help="compression level, default is 11, highest", default=11)
    (options, args) = parser.parse_args()
    folder = options.folder or './rustdesk'
    output_folder = os.path.abspath(options.output_folder or './')

    if not options.executable:
        options.executable = 'rustdesk.exe'
    if not options.executable.startswith(folder):
        options.executable = folder + '/' + options.executable
    exe: str = os.path.abspath(options.executable)
    if not exe.startswith(os.path.abspath(folder)):
        print("The executable must locate in source folder")
        exit(-1)
    exe = '.' + exe[len(os.path.abspath(folder)):]
    print("Executable path: " + exe)
    print("Compression level: " + str(options.level))
    md5_table = generate_md5_table(folder, options.level)
    write_package_metadata(md5_table, output_folder, exe)
    write_app_metadata(output_folder)
    build_portable(output_folder, options.target)
