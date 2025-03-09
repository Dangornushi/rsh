#!/bin/bash

echo 'Hello world!'
pwd                 # 改行で区切られているので上のechoとは別のコマンドとして実行される
echo \
        'I' \
        'like' \
        'shell' \
        'script'

sort < test.txt > sorted.txt