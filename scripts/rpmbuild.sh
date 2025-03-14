#!/bin/bash

get_git_sha() {
    git rev-parse HEAD
}

get_git_short() {
    git rev-parse --short HEAD
}


build_rpm() {
    local git_sha=$(get_git_sha)
    local git_short=$(get_git_short)
    rpmbuild -ba ci/readymade.spec --define "gitcommit $git_sha" --define "shortcommit $git_short" --define "_rpmdir $PWD/build" --define "_disable_source_fetch 0"
}

build_rpm