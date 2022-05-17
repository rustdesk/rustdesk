#!/usr/bin/env bash
cd build/web/
python3 -c 'x=open("./main.dart.js", "rt").read();import re;y=re.search("https://.*canvaskit-wasm@([\d\.]+)/bin/",x);dirname="canvaskit@"+y.groups()[0];z=x.replace(y.group(),"/"+dirname+"/");f=open("./main.dart.js", "wt");f.write(z);import os;os.system("ln -s canvaskit " + dirname);'
mv jds/dist/index.js ./
mv jds/dist/vendor.js ./
/bin/rm -rf js
python3 -c 'import hashlib;x=hashlib.sha1(open("./main.dart.js").read().encode()).hexdigest()[:10];y=open("index.html","rt").read().replace("main.dart.js", "main.dart.js?v="+x);open("index.html","wt").write(y)'
python3 -c 'import hashlib;x=hashlib.sha1(open("./index.js").read().encode()).hexdigest()[:10];y=open("index.html","rt").read().replace("js/dist/index.js", "index.js?v="+x);open("index.html","wt").write(y)'
python3 -c 'import hashlib;x=hashlib.sha1(open("./vendor.js").read().encode()).hexdigest()[:10];y=open("index.html","rt").read().replace("js/dist/vendor.js", "vendor.js?v="+x);open("index.html","wt").write(y)'
tar czf x *
scp x sg:/tmp/
ssh sg "sudo tar xzf /tmp/x -C /var/www/html/web.rustdesk.com/ && /bin/rm /tmp/x && sudo chown www-data:www-data /var/www/html/web.rustdesk.com/ -R"
/bin/rm x
cd -
