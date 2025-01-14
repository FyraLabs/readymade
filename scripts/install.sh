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

install -Dpm644 com.fyralabs.Readymade.svg $root/usr/share/icons/hicolor/scalable/apps/com.fyralabs.Readymade.svg
install -Dpm644 com.fyralabs.Readymade.desktop $root/usr/share/applications/com.fyralabs.Readymade.desktop
install -Dpm644 com.fyralabs.pkexec.readymade.policy $root/usr/share/polkit-1/actions/com.fyralabs.pkexec.readymade.policy
install -Dpm644 templates/ultramarine.toml $root/etc/readymade.toml
