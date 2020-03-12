import py.path

from .... import tracking, __version__
from ... import fs
from ._016 import migrate


def test_migration(testdata: py.path.local, tmpdir: py.path.local) -> None:

    src_repo = testdata.join("repos", "0.15.0").strpath
    dst_repo = tmpdir.strpath

    migrate(src_repo, dst_repo)

    repo = fs.FSRepository(dst_repo)
    docs = repo.read_ref("docs")
    docs2 = repo.read_ref("docs-0.16.0")
    assert docs == docs2

    for manifest in repo.iter_manifests():
        path = repo.render_manifest(manifest)
        actual = tracking.compute_manifest(path)
        assert actual.digest() == manifest.digest()

    assert repo.last_migration() == __version__, "should update last migration marker"