#!/bin/bash

echo 'Hello world!'
pwd                 # 改行で区切られているので上のechoとは別のコマンドとして実行される
echo \
        'I' \
        'like' \
        'shell' \
        'script'

sort < input.txt >> output.txt
sort < input.txt | grep 'a' >> output.txt
ls ./not/exists/directory 2>> error.txt

a=12
b=$a

echo $b
