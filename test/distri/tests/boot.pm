# Copyright 2014-2018 SUSE LLC
# SPDX-License-Identifier: GPL-2.0-or-later

use base 'basetest';
use strict;
use testapi;

sub run {
    assert_screen 'bootloader';
    send_key 'ret';

    assert_and_click 'initial-setup-language', timeout => 600;

    assert_and_click 'initial-setup-welcome';

    assert_and_click 'initial-setup-keyboard';

    assert_screen 'initial-setup-user';
    type_string 'User';
    send_key 'tab';
    type_string 'user';
    click_lastmatch;

    assert_and_click 'initial-setup-password';
    type_string 'password';
    send_key 'tab';
    type_string 'password';
    click_lastmatch;

    assert_and_click 'initial-setup-network';

    assert_and_click 'initial-setup-tweaks';

    assert_and_click 'initial-setup-codecs';

    assert_and_click 'initial-setup-ime';

    assert_and_click 'initial-setup-night-light';

    assert_and_click 'initial-setup-complete', timeout => 900;

    assert_screen 'login', timeout => 600;
}

sub test_flags {
    return { fatal => 1 };
}

1;
