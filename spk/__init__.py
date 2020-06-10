"""SPack - an SpFS Package Manager"""

__version__ = "0.1.0"

from . import api, build, storage, solve
from ._global import load_spec, save_spec

# promote useful front line api functions
from .solve import (
    Solver,
    UnresolvedPackageError,
    ConflictingRequestsError,
    SolverError,
    DecisionTree,
    Decision,
)
from .api import read_spec_file
from .build import make_source_package, make_binary_package