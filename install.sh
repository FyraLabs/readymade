#!/bin/bash
set -x

# This script install all language files and templates,
# taking the root directory as an optional argument.

root=`realpath ${1-/}`
for langfile in po/*.po; do
    install -Dd $root/usr/share/locale/$(basename $langfile .po)/LC_MESSAGES
    msgfmt $langfile -o $root/usr/share/locale/$(basename $langfile .po)/LC_MESSAGES/com.fyralabs.Readymade.mo
done

pushd templates
for dir in `ls -d */`; do
    install -Dd $root/usr/share/readymade/repart-cfgs/$dir
    for f in $dir/*.conf; do
        install -Dm644 $f $root/usr/share/readymade/repart-cfgs/$dir
    done
done
popd
