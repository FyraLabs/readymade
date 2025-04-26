#!/bin/bash
set -x

# This script install all language files and templates,
# taking the root directory as an optional argument.

root=`realpath ${1-/}`
#for langfile in po/*.po; do
#    install -Dd $root/usr/share/locale/$(basename $langfile .po)/LC_MESSAGES
#    msgfmt $langfile -o $root/usr/share/locale/$(basename $langfile .po)/LC_MESSAGES/com.fyralabs.Readymade.mo
#done

BENTO=`sed -nE 's@^\s+const BENTO_ASSETS_PATH: &str = "(.+)";$@\1@p' src/pages/installation.rs | head -n1`

for f in po/*; do
    install -Dm644 {,$root/$BENTO/}$f/readymade.ftl
done

pushd data
for f in *.webp; do
    install -Dm644 {,$root/$BENTO/}$f
done
ln -s viewports-light.webp $root/$BENTO/1
ln -s viewports-dark.webp $root/$BENTO/1-dark
ln -s umbrella-light.webp $root/$BENTO/2
ln -s umbrella-dark.webp $root/$BENTO/2-dark
ln -s blueprint.webp $root/$BENTO/3
ln -s blueprint.webp $root/$BENTO/3-dark
ln -s foresty-skies-light.webp $root/$BENTO/4
ln -s foresty-skies-dark.webp $root/$BENTO/4-dark
popd

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
