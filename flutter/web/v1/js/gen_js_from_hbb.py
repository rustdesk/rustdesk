#!/usr/bin/env python3

import re
import os
import glob
from tabnanny import check

def pad_start(s, n, c = ' '):
   if len(s) >= n:
      return s
   return c * (n - len(s)) + s

def safe_unicode(s):
   res = ""
   for c in s:
      res += r"\u{}".format(pad_start(hex(ord(c))[2:], 4, '0'))
   return res

def main():
   print('export const LANGS = {')
   for fn in glob.glob('../../../src/lang/*'):
      lang = os.path.basename(fn)[:-3]
      if lang == 'template': continue
      print('  %s: {'%lang)
      for ln in open(fn, encoding='utf-8'):
         ln = ln.strip()
         if ln.startswith('("'):
            toks = ln.split('", "')
            assert(len(toks) == 2)
            a = toks[0][2:]
            b = toks[1][:-3]
            print('    "%s": "%s",'%(safe_unicode(a), safe_unicode(b)))
      print('  },')
   print('}')
   check_if_retry = ['', False]
   KEY_MAP = ['', False]
   for ln in open('../../../src/client.rs', encoding='utf-8'):
      ln = ln.strip()
      if 'check_if_retry' in ln:
         check_if_retry[1] = True
         continue
      if ln.startswith('}') and check_if_retry[1]:
         check_if_retry[1] = False
         continue
      if check_if_retry[1]:
         ln = removeComment(ln)
         check_if_retry[0] += ln + '\n'
      if 'KEY_MAP' in ln:
         KEY_MAP[1] = True
         continue
      if '.collect' in ln and KEY_MAP[1]:
         KEY_MAP[1] = False
         continue
      if KEY_MAP[1] and ln.startswith('('):
         ln = removeComment(ln)
         toks = ln.split('", Key::')
         assert(len(toks) == 2)
         a = toks[0][2:]
         b = toks[1].replace('ControlKey(ControlKey::', '').replace("Chr('", '').replace("' as _)),", '').replace(')),', '')
         KEY_MAP[0] += '  "%s": "%s",\n'%(a, b)
   print()
   print('export function checkIfRetry(msgtype: string, title: string, text: string,  retry_for_relay: boolean) {')
   print('  return %s'%check_if_retry[0].replace('to_lowercase', 'toLowerCase').replace('contains', 'indexOf').replace('!', '').replace('")', '") < 0'))
   print(';}')
   print()
   print('export const KEY_MAP: any = {')
   print(KEY_MAP[0])
   print('}')
   for ln in open('../../../Cargo.toml', encoding='utf-8'):
      if ln.startswith('version ='):
         print('export const ' + ln)


def removeComment(ln):
   return re.sub('\s+\/\/.*$', '', ln)

main()
