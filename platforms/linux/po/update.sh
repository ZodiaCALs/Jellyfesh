#!/usr/bin/env bash
# Refreshes the translation template from the sources and merges it into
# every catalogue. Messages are marked with tr(), trc() (with a context),
# trn() (with a plural) and trf() (with {placeholders}); keep each message in
# one string literal (no line continuations).
#   platforms/linux/po/update.sh
set -euo pipefail
cd "$(dirname "$0")/.."
find src/ui -name '*.rs' | LC_ALL=C sort > po/POTFILES
xgettext --language=C --from-code=UTF-8 \
  --keyword=tr --keyword=trc:1c,2 --keyword=trn:1,2 --keyword=trf:1 \
  --flag=tr:1:no-c-format --flag=trf:1:no-c-format \
  --package-name=jellyfesh --add-comments=TRANSLATORS \
  --files-from=po/POTFILES -o po/jellyfesh.pot 2>/dev/null
for lang in $(grep -v '^#' po/LINGUAS); do
  msgmerge --quiet --update --backup=none "po/$lang.po" po/jellyfesh.pot
  msgfmt --check --statistics -o /dev/null "po/$lang.po"
done
