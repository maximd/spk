import stat

import py.path

from ... import tracking
from ._blob import BlobStorage


def test_commit_dir(tmpdir: py.path.local) -> None:

    storage = BlobStorage(tmpdir.join("storage").strpath)

    src_dir = tmpdir.join("source")
    src_dir.join("dir1.0/dir2.0/file.txt").write("somedata", ensure=True)
    src_dir.join("dir1.0/dir2.1/file.txt").write("someotherdata", ensure=True)
    src_dir.join("dir2.0/file.txt").write("evenmoredata", ensure=True)
    src_dir.join("file.txt").write("rootdata", ensure=True)

    manifest = storage.commit_dir(src_dir.strpath)
    assert tmpdir.join(
        "storage", "renders", manifest.digest[:2], manifest.digest[2:]
    ).exists()

    manifest2 = storage.commit_dir(src_dir.strpath)
    assert manifest.digest == manifest2.digest


def test_render_manifest(tmpdir: py.path.local) -> None:

    storage = BlobStorage(tmpdir.join("storage").strpath)

    src_dir = tmpdir.join("source")
    src_dir.join("dir1.0/dir2.0/file.txt").write("somedata", ensure=True)
    src_dir.join("dir1.0/dir2.1/file.txt").write("someotherdata", ensure=True)
    src_dir.join("dir2.0/file.txt").write("evenmoredata", ensure=True)
    src_dir.join("file.txt").write("rootdata", ensure=True)

    expected = tracking.compute_manifest(src_dir.strpath)

    for path, entry in expected.walk_abs(src_dir.strpath):
        if entry.kind is tracking.EntryKind.BLOB:
            with open(path, "rb") as f:
                storage.write_blob(f)

    rendered_path = storage.render_manifest(expected)
    actual = tracking.compute_manifest(rendered_path)
    assert actual.digest == expected.digest


def test_commit_mode(tmpdir: py.path.local) -> None:

    storage = BlobStorage(tmpdir.join("storage").strpath)
    "dir1.0/dir2.0/file2.txt"

    datafile_path = "dir1.0/dir2.0/file.txt"
    symlink_path = "dir1.0/dir2.0/file2.txt"

    src_dir = tmpdir.join("source")
    link_dest = src_dir.join(datafile_path)
    link_dest.write("somedata", ensure=True)
    src_dir.join(symlink_path).mksymlinkto(link_dest)
    link_dest.chmod(0o444)

    manifest = storage.commit_dir(src_dir.strpath)

    rendered_dir = py.path.local(storage._root).join(
        "renders", manifest.digest[:2], manifest.digest[2:]
    )
    rendered_symlink = rendered_dir.join(symlink_path)
    assert stat.S_ISLNK(rendered_symlink.lstat().mode)

    symlink_entry = manifest.get_path(symlink_path)
    assert symlink_entry is not None
    symlink_blob = py.path.local(storage._root).join(
        symlink_entry.digest[:2], symlink_entry.digest[2:]
    )
    assert not stat.S_ISLNK(symlink_blob.lstat().mode)