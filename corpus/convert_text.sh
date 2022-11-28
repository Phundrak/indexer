#!/bin/sh

TEXT=$(sed -r "s|<x:wf word='([^']+)' [^>]+>|\1 |g" "$1" \
           | sed -r "s|<[^>]+>||g" \
           | sed -r "s|&apos;|'|g" \
           | sed -r "s|&quot;|\"|g" \
           | sed -r "s| *([,\.]) *|\1 |g" \
           | sed -r "s|' +|'|g" \
           | sed "s|\. \. \. |...|g" \
           | sed -r "s| +| |g")
echo "$TEXT" > "$1.txt"
