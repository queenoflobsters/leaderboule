#!/bin/bash

SRC="src"           # Source directory
DEST="src_txt"      # Destination directory

# 1. Copy the directory recursively
cp -r "$SRC" "$DEST"

# 2. Find and rename all .rs files to .txt in the new directory
find "$DEST" -type f -name "*.rs" | while read -r file; do
    mv "$file" "${file%.rs}.txt"
done

cp src/init.sql src_txt/init.txt
echo "Done! Copied '$SRC' to '$DEST' and renamed all .rs files to .txt."

