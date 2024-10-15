#!/bin/bash
for size in 16 32 64 128 256 512 1024; do
    #inkscape -z -o $size.png -w $size -h $size icon.svg >/dev/null 2>/dev/null
    convert icon.png -resize ${size}x${size} app_icon_$size.png
done
# from ImageMagick
convert 16.png 32.png 48.png 128.png 256.png -colors 256 icon.ico
#/bin/rm 16.png 32.png 48.png 128.png 256.png
