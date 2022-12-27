#!/usr/bin/env python3

import os
import glob
import sys
import csv

def get_lang(lang):
  out = {}
  for ln in open('./src/lang/%s.rs'%lang, encoding='utf8'):
    ln = ln.strip()
    if ln.startswith('("'):
      k, v = line_split(ln)
      out[k] = v
  return out

def line_split(line):
    toks = line.split('", "')
    if len(toks) != 2:
        print(line)
        assert(0)
    k = toks[0][2:]
    v = toks[1][:-3]
    return k, v


def main():
  if len(sys.argv) == 1:
    expand()
  elif sys.argv[1] == '1':
    to_csv()
  else:
    to_rs(sys.argv[1])


def expand():
  for fn in glob.glob('./src/lang/*'):
    lang = os.path.basename(fn)[:-3]
    if lang in ['en','cn']: continue
    print(lang)
    dict = get_lang(lang)
    fw = open("./src/lang/%s.rs"%lang, "wt", encoding='utf8')
    for line in open('./src/lang/cn.rs', encoding='utf8'):
      line_strip = line.strip()
      if line_strip.startswith('("'):
        k, v = line_split(line_strip)
        if k in dict:
          line = line.replace(v, dict[k])
        else:
          line = line.replace(v, "")
        fw.write(line)
      else:
        fw.write(line)
    fw.close()


def to_csv():
  for fn in glob.glob('./src/lang/*.rs'):
    lang = os.path.basename(fn)[:-3]
    csvfile = open('./src/lang/%s.csv'%lang, "wt", encoding='utf8')
    csvwriter = csv.writer(csvfile)
    for line in open(fn, encoding='utf8'):
      line_strip = line.strip()
      if line_strip.startswith('("'):
        k, v = line_split(line_strip)
        csvwriter.writerow([k, v])
    csvfile.close()


def to_rs(lang):
  csvfile = open('%s.csv'%lang, "rt", encoding='utf8')
  fw = open("./src/lang/%s.rs"%lang, "wt", encoding='utf8')
  fw.write('''lazy_static::lazy_static! {
pub static ref T: std::collections::HashMap<&'static str, &'static str> =
    [
''')
  for row in csv.reader(csvfile):
    fw.write('        ("%s", "%s"),\n'%(row[0].replace('"', '\"'), row[1].replace('"', '\"')))
  fw.write('''    ].iter().cloned().collect();
}
''')
  fw.close()


main()
