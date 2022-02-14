from typing import Dict, Iterator, Mapping, NamedTuple, Tuple, Union, List
from . import storage, api, Digest

PackageSource = Union[Tuple[storage.Repository, Dict[str, Digest]], api.Spec]

class SolverError(Exception): ...
class SolverFailedError(SolverError): ...
class PackageNotFoundError(SolverError): ...

class SolvedRequestIter:
    def __iter__(self) -> Iterator[SolvedRequest]: ...

class SolvedRequest(NamedTuple):
    request: api.PkgRequest
    spec: api.Spec
    source: PackageSource
    def is_source_build(self) -> bool: ...

class Solution:
    def __init__(self, options: api.OptionMap = None) -> None: ...
    def __len__(self) -> int: ...
    def add(
        self, request: api.PkgRequest, package: api.Spec, source: PackageSource
    ) -> None: ...
    def items(self) -> SolvedRequestIter: ...
    def get(self, name: str) -> SolvedRequest: ...
    def options(self) -> api.OptionMap: ...
    def repositories(self) -> List[storage.Repository]: ...
    def to_environment(self, base: Mapping[str, str] = None) -> Dict[str, str]: ...

class State:
    def __init__(
        self,
        pkg_requests: List[api.PkgRequest],
        var_requests: List[api.VarRequest],
        options: Tuple[Tuple[str, str], ...],
        packages: Tuple[Tuple[api.Spec, PackageSource], ...],
    ) -> None: ...
    @property
    def pkg_requests(self) -> List[api.PkgRequest]: ...
    @staticmethod
    def default() -> State: ...
    def get_option_map(self) -> api.OptionMap: ...

class Graph:
    state: State
    def walk(self) -> Iterator[Tuple[Node, Decision]]: ...

class Node: ...

Change = Union[None]
Note = Union[None]

class Decision:
    changes: List[Change]
    notes: List[Note]
    def apply(self, base: State) -> State: ...
    # def iter_changes(self) -> Iterator[Change]: ...
    # def iter_notes(self) -> Iterator[Note]: ...

class SolverRuntime:
    def __iter__(self) -> Iterator[Tuple[Node, Decision]]: ...
    def solution(self) -> Solution: ...
    def current_solution(self) -> Solution: ...
    def graph(self) -> Graph: ...

class Solver:
    def __init__(self) -> None: ...
    def add_repository(self, repo: storage.Repository) -> None: ...
    def add_request(self, request: Union[str, api.Ident, api.Request]) -> None: ...
    def get_initial_state(self) -> State: ...
    def reset(self) -> None: ...
    def run(self) -> SolverRuntime: ...
    def set_binary_only(self, binary_only: bool) -> None: ...
    def solve(self) -> Solution: ...
    def configure_for_build_environment(self, spec: api.Spec) -> None: ...
    def solve_build_environment(self, spec: api.Spec) -> Solution: ...
    def update_options(self, options: api.OptionMap) -> None: ...
    def validate(self, node: State, spec: api.Spec) -> api.Compatibility: ...