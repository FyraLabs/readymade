# todo: CI RPM packages xd

Name:           readymade
Version:        git~%{shortcommit}
Release:        1%?dist
Summary:        Install ready-made distribution images
License:        GPL-3.0-or-later
URL:            https://github.com/FyraLabs/readymade
Source0:        %{url}/archive/%{gitcommit}.tar.gz
BuildRequires:	anda-srpm-macros rust-packaging
BuildRequires:  pkgconfig(libhelium-1)
BuildRequires:  pkgconfig(gnome-desktop-4)
BuildRequires:  clang-devel
BuildRequires:  gcc
BuildRequires:  mold
BuildRequires:  cmake
# We'll need cryptsetup to unlock disks for now
Requires:       cryptsetup
Recommends:     readymade-config

%description
Readymade is a Linux Distribution installer based on the great distinst library by System76.

It is created as a replacement to Red Hat's Anaconda installer for Ultramarine Linux and tauOS after we have heard many complaints about the poor UX design of Anaconda, and the lack of working alternative installers for RPM-based distributions.

%package config-ultramarine
Summary:        Readymade Configuration for Ultramarine Linux
Requires:       readymade
Provides:       readymade-config

%description config-ultramarine
This package contains the configuration files for Readymade to install Ultramarine Linux.

%prep
%autosetup -n %{name}-%{gitcommit}
%cargo_prep_online
# Add debug assertions to the rpm profile
sed -i 's/^\[profile\.rpm\]/[profile.rpm]\ndebug-assertions = true/' .cargo/config

%build
%{cargo_build} --locked

%install
install -Dm755 target/rpm/readymade %buildroot%_bindir/readymade
./install.sh %buildroot

%files
%_bindir/readymade
%_datadir/polkit-1/actions/com.fyralabs.pkexec.readymade.policy
%{_datadir}/applications/com.fyralabs.Readymade.desktop
%{_datadir}/icons/hicolor/*/apps/com.fyralabs.Readymade.*

%files config-ultramarine
%_sysconfdir/readymade.toml
%_datadir/readymade
