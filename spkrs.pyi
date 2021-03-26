from typing import List, Optional

EMPTY_DIGEST: Digest

class Digest: ...

class Runtime:
    def get_stack(self) -> List[Digest]: ...

class SpFSRepository:
    def __init__(self, address: str) -> None: ...
    def has_tag(self, tag: str) -> bool: ...
    def has_digest(self, digest: Digest) -> bool: ...
    def push_ref(self, reference: str, dest: SpFSRepository) -> None: ...
    def push_digest(self, digest: Digest, dest: SpFSRepository) -> None: ...
    def localize_digest(self, digest: Digest) -> None: ...
    def resolve_tag_to_digest(self, tag: str) -> Optional[Digest]: ...
    def push_tag(self, tag: str, target: Digest) -> None: ...
    def ls_all_tags(self) -> List[str]: ...
    def ls_tags(self, base: str) -> List[str]: ...
    def remove_tag_stream(self, tag: str) -> None: ...
    def write_spec(self, tag: str, payload: bytes) -> None: ...
    def read_spec(self, digest: Digest) -> str: ...
    def flush(self) -> None: ...

def configure_logging(verbosity: int) -> None: ...
def active_runtime() -> Runtime: ...
def local_repository() -> SpFSRepository: ...
def remote_repository(path: str) -> SpFSRepository: ...
def open_tar_repository(path: str, create: bool = False) -> SpFSRepository: ...
def validate_build_changeset() -> None: ...
def validate_source_changeset() -> None: ...
def reconfigure_runtime(
    editable: bool = None,
    reset: List[str] = None,
    stack: List[Digest] = None,
) -> None: ...
def build_shell_initialized_command(cmd: str, *args: str) -> List[str]: ...
def build_interactive_shell_command() -> List[str]: ...
def commit_layer(runtime: Runtime) -> Digest: ...
def find_layer_by_filename(path: str) -> Digest: ...
def render_into_dir(stack: List[Digest], path: str) -> None: ...
