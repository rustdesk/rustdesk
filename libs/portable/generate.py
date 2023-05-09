from ast import parse
import os
import optparse
from hashlib import md5
import brotli

# file compress level(0-11)
compress_level = 11
# 4GB maximum
length_count = 4
# encoding
encoding = 'utf-8'

# output: {path: (compressed_data, file_md5)}


def generate_md5_table(folder: str) -> dict:
    res: dict = dict()
    curdir = os.curdir
    os.chdir(folder)
    for root, _, files in os.walk('.'):
        # remove ./
        for f in files:
            md5_generator = md5()
            full_path = os.path.join(root, f)
            print(f"processing {full_path}...")
            f = open(full_path, "rb")
            content = f.read()
            content_compressed = brotli.compress(
                content, quality=compress_level)
            md5_generator.update(content)
            md5_code = md5_generator.hexdigest().encode(encoding=encoding)
            res[full_path] = (content_compressed, md5_code)
    os.chdir(curdir)
    return res


def write_metadata(md5_table: dict, output_folder: str, exe: str):
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
            f.write((data_length).to_bytes(
                length=length_count, byteorder='big'))
            f.write(compressed_data)
            # md5 code
            f.write(md5_code)
        # end
        f.write("rustdesk".encode(encoding=encoding))
        # executable
        f.write(exe.encode(encoding='utf-8'))
    print(f"metadata had written to {output_path}")


def build_portable(output_folder: str):
    os.chdir(output_folder)
    os.system("cargo build --release")

# Linux: python3 generate.py -f ../rustdesk-portable-packer/test -o . -e ./test/main.py
# Windows: python3 .\generate.py -f ..\rustdesk\flutter\build\windows\runner\Debug\ -o . -e ..\rustdesk\flutter\build\windows\runner\Debug\rustdesk.exe
if __name__ == '__main__':
    parser = optparse.OptionParser()
    parser.add_option("-f", "--folder", dest="folder",
                      help="folder to compress")
    parser.add_option("-o", "--output", dest="output_folder",
                      help="the root of portable packer project")
    parser.add_option("-e", "--executable", dest="executable",
                      help="specify startup file")
    (options, args) = parser.parse_args()
    folder = options.folder
    output_folder = os.path.abspath(options.output_folder)

    exe: str = os.path.abspath(options.executable)
    if not exe.startswith(os.path.abspath(folder)):
        print("the executable must locate in source folder")
        exit(-1)
    exe = '.' + exe[len(os.path.abspath(folder)):]
    print("executable path: " + exe)
    md5_table = generate_md5_table(folder)
    write_metadata(md5_table, output_folder, exe)
    build_portable(output_folder)
