#!/bin/bash
set -euxo pipefail

# This script installs all language files, bento assets and templates,
# taking the root directory as an optional argument.

root=$(realpath "${1-/}")

BENTO=$(sed -nE 's@^\s+(pub\(super\) )?const BENTO_ASSETS_PATH: &str = "(.+)";$@\2@p' src/pages/installation.rs | head -n1)
[[ -n $BENTO ]] || { echo "cannot find BENTO_ASSETS_PATH in src/pages/installation.rs" >&2; exit 1; }

for f in po/*; do
    install -Dm644 "$f/readymade.ftl" "$root/$BENTO/$f/readymade.ftl"
done

pushd data
for f in *.webp; do
    install -Dm644 "$f" "$root/$BENTO/$f"
done
ln -sfn viewports-light.webp "$root/$BENTO/1"
ln -sfn viewports-dark.webp "$root/$BENTO/1-dark"
ln -sfn umbrella-light.webp "$root/$BENTO/2"
ln -sfn umbrella-dark.webp "$root/$BENTO/2-dark"
ln -sfn blueprint.webp "$root/$BENTO/3"
ln -sfn blueprint.webp "$root/$BENTO/3-dark"
ln -sfn foresty-skies-light.webp "$root/$BENTO/4"
ln -sfn foresty-skies-dark.webp "$root/$BENTO/4-dark"
popd

pushd crates/libreadymade/templates
for dir in */; do
    dir=${dir%/}
    install -Dm644 -t "$root/usr/share/readymade/repart-cfgs/$dir" "$dir"/*.conf
done
popd

install -Dpm644 data/com.fyralabs.Readymade.svg "$root/usr/share/icons/hicolor/scalable/apps/com.fyralabs.Readymade.svg"
install -Dpm644 data/com.fyralabs.Readymade.desktop "$root/usr/share/applications/com.fyralabs.Readymade.desktop"
install -Dpm644 data/com.fyralabs.pkexec.readymade.policy "$root/usr/share/polkit-1/actions/com.fyralabs.pkexec.readymade.policy"
install -Dpm644 crates/libreadymade/templates/ultramarine.toml "$root/etc/readymade.toml"
