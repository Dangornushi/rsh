#!/bin/bash

echo 'Hello world!'
pwd                 # 改行で区切られているので上のechoとは別のコマンドとして実行される
echo \
        'I' \
        'like' \
        'shell' \
        'script'
echo                # 引数なしの echo コマンドとして実行される
'End world!'        # 'End world!' というコマンドを実行しようとするが存在しないのでエラーになる

sort < test.txt