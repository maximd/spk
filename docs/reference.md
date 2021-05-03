---
title: API Reference
summary: Detailed package specification information
weight: 110
---

## Package Spec

| Field      | Type                              | Description                                                                                                                                           |
| ---------- | --------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------- |
| pkg        | _[Identifier](#identifier)_       | The name and version number of this package                                                                                                           |
| compat     | _[Compat](#compat)_               | The compatibility semantics of this packages versioning scheme                                                                                        |
| deprecated | _boolean_                         | True if this package has been deprecated, this is usually reserved for internal use only and should not generally be specified directly in spec files |
| sources    | _List[[SourceSpec](#sourcespec)]_ | Specifies where to get source files for building this package                                                                                         |
| build      | _[BuildSpec](#buildspec)_         | Specifies how the package is to be built                                                                                                              |
| tests      | _List[[TestSpec](#testspec)]_     | Specifies any number of tests to validate the package and software                                                                                    |
| install    | _[InstallSpec](#installspec)_     | Specifies how the package is to be installed                                                                                                          |

## SourceSpec

A source spec can be one of [LocalSource](#localsource), [GitSource](#gitsource), or [TarSource](#tarsource).

### LocalSource

Defines a local directory to collect sources from. This process will also automatically detect a git repository and not transfer ignored files.

| Field   | Type        | Description                                                                                  |
| ------- | ----------- | -------------------------------------------------------------------------------------------- |
| path    | _str_       | The relative or absolute path to a local directory                                           |
| exclude | _List[str]_ | A list of glob patterns for files and directories to exclude (defaults to `".git/", ".svn/")    |
| filter | _List[str]_ | A list of filter rules for rsync (defaults to reading from the gitignore file: `":- .gitignore"`) |
| subdir  | _str_       | An alternative path to place these files in the source package                               |

### GitSource

Clones a git repository as package source files.

| Field  | Type  | Description                                                    |
| ------ | ----- | -------------------------------------------------------------- |
| git    | _str_ | The url or local path to a git repository to be cloned         |
| ref    | _str_ | Optional branch, commit or tag name for the source repo        |
| subdir | _str_ | An alternative path to place these files in the source package |

### TarSource

Fetches and extracts a tar archive as package source files.

| Field  | Type  | Description                                                    |
| ------ | ----- | -------------------------------------------------------------- |
| tar    | _str_ | The url or local path to tar file                              |
| subdir | _str_ | An alternative path to place these files in the source package |

## BuildSpec

| Field    | Type                                | Description                                                    |
| -------- | ----------------------------------- | -------------------------------------------------------------- |
| script   | _str_ or _List[str]_                | The bash script which builds and installs the package to /spfs |
| options  | _List[[BuildOption](#buildoption)]_ | The set of inputs for the package build process                |
| variants | _List[[OptionMap](#optionmap)]_     | The default variants of the package options to build           |

### BuildOption

A build option can be one of [VariableOption](#variableoption), or [PackageOption](#packageoption).

#### VariableOption

Variable options represents some arbitrary configuration parameter to the build. When the value of this string changes, a new build of the package is required. Some common examples of these options are: `arch`, `os`, `debug`.

| Field       | Type        | Description                                                                                                                                                                                                                                                                                                                                                                                                                       |
| ----------- | ----------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| var         | _str_       | The name of the option, with optional default value (eg `my_option` or `my_option/default_value`)                                                                                                                                                                                                                                                                                                                                                                                                            |
| choices     | _List[str]_ | An optional set of possible values for this variable                                                                                                                                                                                                                                                                                                                                                                              |
| inheritance | _str_       | Defines how this option is inherited by downstream packages. `Weak` is the default behaviour and does not influence downstream packages directly. `Strong` propagates this build option into every package that has this one in it's build environment while also adding an install requirement for this option. `StrongForBuildOnly` can be used to propagate this requirement as a build option but not an install requirement. |
| static      | _str_       | Defines an unchangeable value for this variable - this is usually reserved for use by the system and is set when a package build is published to save the value of the variable at build time                                                                                                                                                                                                                                     |

#### PackageOption

Package options define a package that is required at build time.

| Field            | Type                                    | Description                                                                                                                                                                                    |
| ---------------- | --------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| pkg              | _str_                                   | The name of the package that is required, with optional default value (eg just `package_name`, or `package_name/1.4`)                                                                                                                                                       |
| prereleasePolicy | _[PreReleasePolicy](#prereleasepolicy)_ | Defines how pre-release versions should be handled when resolving this request                                                                                                                 |
| static           | _str_                                   | Defines an unchangeable value for this variable - this is usually reserved for use by the system and is set when a package build is published to save the version of the package at build time |

### OptionMap

An option map is a key-value mapping of option names to option values. In practice, this is just a dictionary: `{debug: on, python: 2.7}`. Any values that are not strings are converted to strings in the normal python fashion.

## TestSpec

A test spec defines one test script that should be run against the package to validate it. Each test script can run against one stage of the package, meaning that you can define test processes for the source package, build environment (unit tests), or install environment (integration tests).

| Field        | Type                            | Description                                                                                                                        |
| ------------ | ------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------- |
| stage        | _str_                           | The stage that this test validates, one of: **sources**, **build**, **install**                                                    |
| selectors    | _List[[OptionMap](#optionmap)]_ | Identifies which variants this test should be executed against. Variants must match one of the selectors in this list to be tested |
| requirements | _List[[Request](#request)]_     | Additional packages required in the test environment                                                                               |
| script       | _str_ or _List[str]_            | The sh script which tests the package                                                                                              |

## InstallSpec

| Field        | Type                        | Description                                      |
| ------------ | --------------------------- | ------------------------------------------------ |
| requirements | _List[[Request](#request)]_ | The set of packages required at runtime          |
| embedded     | _List[[Spec](#spec)]_       | A list of packages that come bundled in this one |

### Request

A build option can be one of [VariableRequest](#variablerequest), or [PackageRequest](#packagerequest).

#### VariableRequest

| Field        | Type   | Description                                                                                                                                                                  |
| ------------ | ------ | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| var          | _str_  | The requested value of a package build variable in the form `name=value`, this can reference a specific package or the global variable (eg `debug=on`, or `python.abi=cp37`) |
| fromBuildEnv | _bool_ | If true, replace the requested value of this variable with the value used in the build environment                                                                           |

#### PackageRequest

| Field            | Type                                    | Description                                                                                                                                                                                                                                                                                                                         |
| ---------------- | --------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| pkg              | RangeIdentifier                         | Like an [Identifier](#identifier) but with a verison range rather than an exact version, see [versioning](../versioning)                                                                                                                                                                                                            |
| prereleasePolicy | _[PreReleasePolicy](#prereleasepolicy)_ | Defines how pre-release versions should be handled when resolving this request                                                                                                                                                                                                                                                      |
| inclusionPolicy  | _[InclusionPolicy](#inclusionpolicy)_   | Defines when the requested package should be included in the environment                                                                                                                                                                                                                                                            |
| fromBuildEnv     | _str_                                   | The optional template to use to generate this request based on the version of the package resolved into the build environment. This template takes the form `x.x.x`, where any _x_ is replaced by digits in the version number. For example, if `python/2.7.5` is in the build environment, the template `~x.x` would become `~2.7` |

#### PreReleasePolicy

| Value                | Description                                 |
| -------------------- | ------------------------------------------- |
| ExcludeAll (default) | Do not include pre-release package versions |
| IncludeAll           | Include all pre-release package versions    |

#### InclusionPolicy

| Value            | Description                                                                                                                                   |
| ---------------- | --------------------------------------------------------------------------------------------------------------------------------------------- |
| Always (default) | Always include the requested package in the envrionment                                                                                       |
| IfAlreadyPresent | Only include this package in the environment if it is already in the environment or another request exists with the `Always` inclusion policy |

## Identifier

The package identifer takes the form `<name>[/<version>[/<build>]]`, where:

| Component | Description                                                                                                                                                                                                                                                                       |
| --------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| name      | The package name, can only have lowercase letter and dashes (`-`)                                                                                                                                                                                                                 |
| version   | The version number, see [versioning](../versioning)                                                                                                                                                                                                                               |
| build     | The build string, should not be specified in a spec file as it is generated by the system at build time. Digests are calculated based on the package build options, and there are two special values `src` and `embedded` for source packages and embedded packages, respectively |

## Compat

Specifies the compatilbility contract of a version number. The compat string is a dot-separated set of characters that define contract, for example `x.a.b` (the default contract) says that major version changes are not compatible, minor version changes provides **A**PI compatibility, and patch version changes provide **B**inary compatibility.
