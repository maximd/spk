// Copyright (c) Sony Pictures Imageworks, et al.
// SPDX-License-Identifier: Apache-2.0
// https://github.com/imageworks/spk

use std::str::FromStr;

use rstest::rstest;
use spk_schema_foundation::ident_component::Component;
use spk_schema_foundation::option_map::OptionMap;
use spk_schema_foundation::version::{Version, BINARY_STR};
use spk_schema_ident::{parse_ident_range, PkgRequest, Request, RequestedBy};

use crate::{InstallSpec, Package, RequirementsList};

#[rstest]
fn test_render_all_pins_renders_requirements_in_components() {
    let mut install_spec = InstallSpec::default();
    let mut requirements = RequirementsList::default();
    requirements.insert_or_replace({
        Request::Pkg(
            PkgRequest::new(
                parse_ident_range("test").unwrap(),
                RequestedBy::SpkInternalTest,
            )
            .with_pin(Some(BINARY_STR.to_string())),
        )
    });
    install_spec
        .components
        .iter_mut()
        .find(|c| c.name == Component::Run)
        .unwrap()
        .requirements = requirements;

    // Expected value before pinning.
    let Request::Pkg(req) = &install_spec
        .components
        .iter()
        .find(|c| c.name == Component::Run)
        .unwrap()
        .requirements[0]
    else {
        panic!("Expected a Pkg request");
    };
    assert_eq!(req.to_string(), "test");

    install_spec
        .render_all_pins(
            &OptionMap::default(),
            ["test/1.2.3/GMTG3CXY".parse().unwrap()].iter(),
        )
        .unwrap();

    // Now the install requirement inside the run component should be pinned to
    // version 1.2.3.
    let Request::Pkg(req) = &install_spec
        .components
        .iter()
        .find(|c| c.name == Component::Run)
        .unwrap()
        .requirements[0]
    else {
        panic!("Expected a Pkg request");
    };
    assert_eq!(req.to_string(), "test/Binary:1.2.3");
}

#[rstest]
fn test_embedded_components_defaults() {
    // By default, embedded components will embed matching components from the
    // defined embedded packages.
    let install = serde_yaml::from_str::<InstallSpec>(
        r#"
embedded:
  - pkg: "embedded/1.0.0"
        "#,
    )
    .unwrap();

    assert_eq!(
        install.components.len(),
        2,
        "expecting two default components: build and run"
    );

    assert_eq!(install.embedded.len(), 1, "expecting one embedded package");

    assert_eq!(
        install.embedded[0].components().len(),
        2,
        "expecting two default components: build and run"
    );

    for component in install.components.iter() {
        assert_eq!(
            component.embedded_components.len(),
            1,
            "expecting each host component to embed one component"
        );

        assert_eq!(
            component.embedded_components[0].pkg.name(),
            "embedded",
            "expecting the embedded package name to be correct"
        );

        assert_eq!(
            component.embedded_components[0].components.len(),
            1,
            "expecting the build and run components to get mapped 1:1"
        );

        assert_eq!(
            *component.embedded_components[0]
                .components
                .iter()
                .next()
                .unwrap(),
            component.name,
            "expecting the component names to agree"
        );
    }
}

#[rstest]
fn test_embedded_components_extra_components() {
    // If the embedded package has components that the host package doesn't
    // have, they don't get mapped anywhere automatically.
    let install = serde_yaml::from_str::<InstallSpec>(
        r#"
components:
  - name: comp1
embedded:
  - pkg: "embedded/1.0.0"
    install:
      components:
        - name: comp1
        - name: comp2
        "#,
    )
    .unwrap();

    assert_eq!(
        install.components.len(),
        3,
        "expecting two default components and one explicit component: comp1"
    );

    assert_eq!(install.embedded.len(), 1, "expecting one embedded package");

    assert_eq!(
        install.embedded[0].components().len(),
        4,
        "expecting two default components and two explicit components: comp1 and comp2"
    );

    for component in install.components.iter() {
        assert_eq!(
            component.embedded_components.len(),
            1,
            "expecting each host component to embed one component"
        );

        assert_eq!(
            component.embedded_components[0].pkg.name(),
            "embedded",
            "expecting the embedded package name to be correct"
        );

        assert_eq!(
            component.embedded_components[0].components.len(),
            1,
            "expecting all the host package's components to get mapped 1:1"
        );

        assert_eq!(
            *component.embedded_components[0]
                .components
                .iter()
                .next()
                .unwrap(),
            component.name,
            "expecting the component names to agree"
        );

        assert_ne!(
            component.name.as_str(),
            "comp2",
            "expecting the host package to not have comp2"
        );
    }
}

#[rstest]
#[case("comp1", "1.0.0")]
#[case("comp2", "2.0.0")]
fn test_embedding_multiple_versions_of_the_same_package(
    #[case] component_name: &str,
    #[case] expected_component_version: &str,
) {
    // Allow multiple versions of the same package to be embedded. Test that
    // it is possible to assign the different versions to different
    // components in the host package.
    let install = serde_yaml::from_str::<InstallSpec>(
        r#"
components:
  - name: comp1
    embedded_components:
      - embedded:all/1.0.0
  - name: comp2
    embedded_components:
      - embedded:all/2.0.0
embedded:
  - pkg: "embedded/1.0.0"
  - pkg: "embedded/2.0.0"
        "#,
    )
    .unwrap();

    assert_eq!(install.embedded.len(), 2, "expecting two embedded packages");

    assert_eq!(
        install.embedded[0].ident().name(),
        install.embedded[1].ident().name(),
        "expecting both embedded packages to be the same package"
    );

    assert_ne!(
        install.embedded[0].ident().version(),
        install.embedded[1].ident().version(),
        "expecting the embedded packages to be different versions"
    );

    let comp = install
        .components
        .iter()
        .find(|c| c.name.as_str() == component_name)
        .unwrap();

    assert_eq!(
        comp.embedded_components.len(),
        1,
        "expecting one embedded package"
    );

    assert_eq!(
        comp.embedded_components[0].pkg.target(),
        &Some(Version::from_str(expected_component_version).unwrap()),
        "expecting the embedded package version to be correct"
    );

    assert_eq!(
        comp.embedded_components[0].components.len(),
        2,
        "expecting 'all' to be expanded into all the embedded package's components"
    );
}
