#!/usr/bin/env python3
import os
import glob

def get_lang(lang):
  out = {}
  for ln in open('./src/lang/%s.rs'%lang):
    ln = ln.strip()
    if ln.startswith('("'):
      toks = ln.split('", "')
      assert(len(toks) == 2)
      a = toks[0][2:]
      b = toks[1][:-3]
      out[a] = b
  return out

def main():
  cn = get_lang('cn')
  for fn in glob.glob('./src/lang/*'):
    lang = os.path.basename(fn)[:-3]
    if lang in ['en', 'cn']: continue
    not_transated = (set(cn.keys()) - set(get_lang(lang).keys()))
    if not_transated:
      extra = '\n'.join(map(lambda x: '        ("%s", ""),'%x, not_transated))
      endstr = '].iter().cloned().collect();'
      text = open(fn).read().replace(endstr, extra + '\n' + endstr)
      with open(fn, 'wt') as fh:
        fh.write(text)


main()
