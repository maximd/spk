Name: spk
Version: 0.11.1
Release: 1
Summary: Package manager for SPFS.
License: NONE
URL: https://gitlab.spimageworks.com/dev-group/dev-ops/spk
Source0: https://gitlab.spimageworks.com/dev-group/dev-ops/spk/-/archive/v%{version}/%{name}-v%{version}.tar.gz

BuildRequires: gcc
BuildRequires: gcc-c++
BuildRequires: chrpath
BuildRequires: python-pip
BuildRequires: python37-devel
Requires: spfs

%description
Package manager for SPFS

%prep
%setup -q -n %{name}-v%{version}

%build
mkdir -p ./build
pipenv sync --dev
source "$(pipenv --venv)/bin/activate"
python -m nuitka \
    --standalone \
    --jobs $(nproc) \
    --follow-imports \
    --output-dir=./build \
    --include-package='sentry_sdk.integrations.stdlib' \
    --include-package='sentry_sdk.integrations.excepthook' \
    --include-package='sentry_sdk.integrations.dedupe' \
    --include-package='sentry_sdk.integrations.atexit' \
    --include-package='sentry_sdk.integrations.logging' \
    --include-package='sentry_sdk.integrations.argv' \
    --include-package='sentry_sdk.integrations.modules' \
    --include-package='sentry_sdk.integrations.threading' \
    spk

%install
mkdir -p %{buildroot}/usr/local/bin
mkdir -p %{buildroot}/opt/spk.dist
rsync -rvapog --chmod 755 %{_builddir}/%{name}-v%{version}/build/spk.dist/* %{buildroot}/opt/spk.dist/

%files
/opt/spk.dist/

%preun
[ -e /usr/local/bin/spk ] && unlink /usr/local/bin/spk

%posttrans
# must run at the absolute end in case we are updating
# and the uninstallation of the old version removes the symlink
ln -sf /opt/spk.dist/spk /usr/local/bin/spk
