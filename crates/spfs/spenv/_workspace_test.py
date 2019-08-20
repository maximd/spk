import pytest
import git

from . import storage
from ._config import Config
from ._workspace import (
    create_workspace,
    discover_workspace,
    NoWorkspaceError,
    read_workspace,
    Workspace,
)


@pytest.fixture
def tmpconf(tmpdir):

    return Config(storage_root=tmpdir.join("storage").strpath)


@pytest.fixture
def tmpwksp(tmpdir, tmpconf: Config):
    wksp = create_workspace(tmpdir.join("wksp").strpath)
    wksp.config = tmpconf
    return wksp


def test_create_workspace(tmpdir):

    expected = tmpdir.join(".spenv")
    wksp = create_workspace(tmpdir.strpath)
    assert expected.exists()
    assert expected.isdir()
    assert wksp.dotspenvdir == expected.strpath


def test_create_workspace_no_root(tmpdir):

    expected = tmpdir.join("subdir", ".spenv")
    wksp = create_workspace(tmpdir.join("subdir").strpath)
    assert expected.exists()
    assert expected.isdir()
    assert wksp.dotspenvdir == expected.strpath


def test_create_workspace_exists(tmpdir):

    create_workspace(tmpdir.strpath)
    with pytest.raises(ValueError):
        create_workspace(tmpdir.strpath)


def test_discover_workspace(tmpdir):

    expected = tmpdir.join("workspace").ensure(dir=True)
    create_workspace(expected.strpath)
    subdir = expected.join("subdir", "subdir").ensure(dir=True)

    assert discover_workspace(subdir.strpath).rootdir == expected


def test_discover_workspace_not_exist(tmpdir):

    with pytest.raises(NoWorkspaceError):

        discover_workspace(tmpdir.strpath)


def test_workspace_checkout_no_repo(tmpwksp: Workspace):

    with pytest.raises(storage.NoRepositoryError):
        tmpwksp.checkout("localhost.test/spi/base:25")


def test_workspace_checkout_no_version(tmpwksp: Workspace):

    repos = tmpwksp.config.repository_storage()
    repo = repos.create_repository("localhost.test/spi/base")
    with pytest.raises(storage.UnknownVersionError):
        tmpwksp.checkout("localhost.test/spi/base:25")


def test_workspace_checkout(tmpwksp: Workspace):

    repos = tmpwksp.config.repository_storage()
    repo = repos.create_repository("localhost.test/spi/base")
    repo._repo.index.commit("initial commit")

    tmpwksp.checkout("localhost.test/spi/base:master")


def test_workspace_sync_meta(tmpwksp: Workspace):

    repos = tmpwksp.config.repository_storage()
    repo = repos.create_repository("localhost.test/spi/base")
    repo._repo.index.commit("initial commit")

    tmpwksp.checkout("localhost.test/spi/base:master")

    tmpwksp._sync_meta()
    assert repo._repo.is_dirty()
    assert git.Diff(repo._repo)  # THIS MIGHT NOT BE RIGHT
