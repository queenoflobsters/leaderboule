#!/bin/bash
set -euo pipefail

SRC="src"           # Source directory
DEST="src_txt"      # Destination directory

# 1. Recopie l'arborescence depuis zéro (évite les copies imbriquées)
rm -rf "$DEST"
cp -r "$SRC" "$DEST"

# 2. Renomme tous les .rs en .txt dans la nouvelle arborescence
find "$DEST" -type f -name "*.rs" | while read -r file; do
    mv "$file" "${file%.rs}.txt"
done

# 3. Le schéma SQL est aussi exporté en .txt
mv "$DEST/init.sql" "$DEST/init.txt"

echo "Done! Copied '$SRC' to '$DEST' and renamed all .rs files to .txt."
