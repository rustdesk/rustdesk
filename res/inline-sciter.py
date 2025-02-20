#!/usr/bin/env python3

import re


def strip(s): return re.sub(r'\s+\n', '\n', re.sub(r'\n\s+', '\n', s))


def read_file(file_path, encoding=None):
    with open(file_path, encoding=encoding) as f:
        return f.read()


common_css = read_file('src/ui/common.css')
common_tis = read_file('src/ui/common.tis', encoding='UTF8')

index_html = read_file('src/ui/index.html')
index_css = read_file('src/ui/index.css')
index_tis = read_file('src/ui/index.tis')
msgbox_tis = read_file('src/ui/msgbox.tis')
ab_tis = read_file('src/ui/ab.tis')

index = index_html.replace('@import url(index.css);', index_css) \
    .replace('include "index.tis";', index_tis) \
    .replace('include "msgbox.tis";', msgbox_tis) \
    .replace('include "ab.tis";', ab_tis)

remote_html = read_file('src/ui/remote.html')
remote_css = read_file('src/ui/remote.css')
header_css = read_file('src/ui/header.css')
file_transfer_css = read_file('src/ui/file_transfer.css')
remote_tis = read_file('src/ui/remote.tis')
msgbox_tis = read_file('src/ui/msgbox.tis')
grid_tis = read_file('src/ui/grid.tis')
header_tis = read_file('src/ui/header.tis')
file_transfer_tis = read_file('src/ui/file_transfer.tis')
port_forward_tis = read_file('src/ui/port_forward.tis')

remote = remote_html.replace('@import url(remote.css);', remote_css) \
    .replace('@import url(header.css);', header_css) \
    .replace('@import url(file_transfer.css);', file_transfer_css) \
    .replace('include "remote.tis";', remote_tis) \
    .replace('include "msgbox.tis";', msgbox_tis) \
    .replace('include "grid.tis";', grid_tis) \
    .replace('include "header.tis";', header_tis) \
    .replace('include "file_transfer.tis";', file_transfer_tis) \
    .replace('include "port_forward.tis";', port_forward_tis)

chatbox = read_file('src/ui/chatbox.html')
install_html = read_file('src/ui/install.html')
install_tis = read_file('src/ui/install.tis')

install = install_html.replace('include "install.tis";', install_tis)

cm_html = read_file('src/ui/cm.html')
cm_css = read_file('src/ui/cm.css')
cm_tis = read_file('src/ui/cm.tis')

cm = cm_html.replace('@import url(cm.css);', cm_css).replace('include "cm.tis";', cm_tis)


def compress(s):
    s = s.replace("\r\n", "\n")
    x = bytes(s, encoding='utf-8')
    return '&[u8; ' + str(len(x)) + '] = b"' + str(x)[2:-1].replace(r"\'", "'").replace(r'"', r'\"') + '"'


with open('src/ui/inline.rs', 'wt') as fh:
    fh.write('const _COMMON_CSS: ' + compress(strip(common_css)) + ';\n')
    fh.write('const _COMMON_TIS: ' + compress(strip(common_tis)) + ';\n')
    fh.write('const _INDEX: ' + compress(strip(index)) + ';\n')
    fh.write('const _REMOTE: ' + compress(strip(remote)) + ';\n')
    fh.write('const _CHATBOX: ' + compress(strip(chatbox)) + ';\n')
    fh.write('const _INSTALL: ' + compress(strip(install)) + ';\n')
    fh.write('const _CONNECTION_MANAGER: ' + compress(strip(cm)) + ';\n')
    fh.write('''
fn get(data: &[u8]) -> String {
    String::from_utf8_lossy(data).to_string()
}
fn replace(data: &[u8]) -> String {
    let css = get(&_COMMON_CSS[..]);
    let res = get(data).replace("@import url(common.css);", &css);
    let tis = get(&_COMMON_TIS[..]);
    res.replace("include \\\"common.tis\\\";", &tis)
}
#[inline]
pub fn get_index() -> String {
    replace(&_INDEX[..])
}
#[inline]
pub fn get_remote() -> String {
    replace(&_REMOTE[..])
}
#[inline]
pub fn get_install() -> String {
    replace(&_INSTALL[..])
}
#[inline]
pub fn get_chatbox() -> String {
    replace(&_CHATBOX[..])
}
#[inline]
pub fn get_cm() -> String {
    replace(&_CONNECTION_MANAGER[..])
}
''')
