#!/usr/bin/env python3

# Based on 'cn.rs', generate entries that are not completed in other languages

import os    
import glob    
    
def get_lang(lang):    
  out = {}    
  for ln in open('./src/lang/%s.rs'%lang):    
    ln = ln.strip()
    if ln.startswith('("'):
      k,v = line_split(ln)
      out[k] = v    
  return out 

def line_split(line):
    toks = line.split('", "')    
    assert(len(toks) == 2)    
    k = toks[0][2:]    
    v = toks[1][:-3]
    return k,v


def main():     
  for fn in glob.glob('./src/lang/*'): 
    lang = os.path.basename(fn)[:-3] 
    if lang in ['en','cn']: continue  
    fw = open("%s.rs.gen"%lang, "wb+")
    dict = get_lang(lang)
    for line in open('./src/lang/cn.rs'):
      line_strip = line.strip()
      if line_strip.startswith('("'):
        k,v = line_split(line_strip)
        if k in dict:
            line = line.replace(v, dict[k])
        else:
            line = line.replace(v, "")
        fw.write(line.encode())
      else:
        fw.write(line.encode())
    fw.close()
    os.remove("./src/lang/%s.rs"%lang)
    os.rename(fw.name, "./src/lang/%s.rs"%lang)
    
main()