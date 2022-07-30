// Copyright (c) 2021 Sony Pictures Imageworks, et al.
// SPDX-License-Identifier: Apache-2.0
// https://github.com/imageworks/spk

use rstest::rstest;

use crate::{api, fixtures::*, pkg_name, prelude::*, Error};

#[rstest]
#[case::mem(RepoKind::Mem)]
#[case::spfs(RepoKind::Spfs)]
#[tokio::test]
async fn test_repo_list_empty(#[case] repo: RepoKind) {
    let repo = make_repo(repo).await;
    assert!(
        repo.list_packages().await.unwrap().is_empty(),
        "should not fail when empty"
    );
}

#[rstest]
#[case::mem(RepoKind::Mem)]
#[case::spfs(RepoKind::Spfs)]
#[tokio::test]
async fn test_repo_list_package_versions_empty(#[case] repo: RepoKind) {
    let repo = make_repo(repo).await;
    assert!(
        repo.list_package_versions(pkg_name!("nothing"))
            .await
            .unwrap()
            .is_empty(),
        "should not fail with unknown package"
    );
}

#[rstest]
#[case::mem(RepoKind::Mem)]
#[case::spfs(RepoKind::Spfs)]
#[tokio::test]
async fn test_repo_list_package_builds_empty(#[case] repo: RepoKind) {
    let repo = make_repo(repo).await;
    let nothing = api::parse_ident("nothing/1.0.0").unwrap();
    assert!(
        repo.list_package_builds(&nothing).await.unwrap().is_empty(),
        "should not fail with unknown package"
    );
}

#[rstest]
#[case::mem(RepoKind::Mem)]
#[case::spfs(RepoKind::Spfs)]
#[tokio::test]
async fn test_repo_read_recipe_empty(#[case] repo: RepoKind) {
    let repo = make_repo(repo).await;
    let nothing = api::parse_ident("nothing").unwrap();
    match repo.read_recipe(&nothing).await {
        Err(Error::PackageNotFoundError(_)) => (),
        _ => panic!("expected package not found error"),
    }
}

#[rstest]
#[case::mem(RepoKind::Mem)]
#[case::spfs(RepoKind::Spfs)]
#[tokio::test]
async fn test_repo_read_package_empty(#[case] repo: RepoKind) {
    let repo = make_repo(repo).await;
    let nothing = api::parse_ident("nothing/1.0.0/src").unwrap();
    match repo.read_package(&nothing).await {
        Err(Error::PackageNotFoundError(_)) => (),
        res => panic!("expected package not found error, got {:?}", res),
    }
}

#[rstest]
#[case::mem(RepoKind::Mem)]
#[case::spfs(RepoKind::Spfs)]
#[tokio::test]
async fn test_repo_publish_recipe(#[case] repo: RepoKind) {
    let repo = make_repo(repo).await;
    let spec = crate::recipe!({"pkg": "my-pkg/1.0.0"});
    repo.publish_recipe(&spec).await.unwrap();
    assert_eq!(
        repo.list_packages().await.unwrap(),
        vec![spec.name().to_owned()]
    );
    assert_eq!(
        repo.list_packages()
            .await
            .unwrap()
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>(),
        vec![spec.name().to_string()]
    );
    assert_eq!(
        repo.list_package_versions(spec.name())
            .await
            .unwrap()
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>(),
        vec!["1.0.0"]
    );

    match repo.publish_recipe(&spec).await {
        Err(Error::VersionExistsError(_)) => (),
        _ => panic!("expected version exists error"),
    }
    repo.force_publish_recipe(&spec)
        .await
        .expect("force publish should ignore existing version");
}

#[rstest]
#[case::mem(RepoKind::Mem)]
#[case::spfs(RepoKind::Spfs)]
#[tokio::test]
async fn test_repo_publish_package(#[case] repo: RepoKind) {
    let repo = make_repo(repo).await;
    let recipe = crate::recipe!({"pkg": "my-pkg/1.0.0"});
    repo.publish_recipe(&recipe).await.unwrap();
    let spec = crate::spec!({"pkg": "my-pkg/1.0.0/7CI5R7Y4"});
    repo.publish_package(
        &spec,
        &vec![(api::Component::Run, empty_layer_digest())]
            .into_iter()
            .collect(),
    )
    .await
    .unwrap();
    assert_eq!(
        repo.list_package_builds(spec.ident()).await.unwrap(),
        [spec.ident().clone()]
    );
    assert_eq!(*repo.read_recipe(&recipe.ident()).await.unwrap(), recipe);
    repo.publish_package(
        &spec,
        &vec![(api::Component::Run, empty_layer_digest())]
            .into_iter()
            .collect(),
    )
    .await
    .unwrap();
    assert_eq!(
        repo.list_package_builds(spec.ident()).await.unwrap(),
        vec![spec.ident().clone()]
    );
    assert_eq!(*repo.read_recipe(&recipe.ident()).await.unwrap(), recipe);
    repo.remove_package(api::Package::ident(&spec))
        .await
        .unwrap();
    assert!(repo
        .list_package_builds(api::Package::ident(&spec))
        .await
        .unwrap()
        .is_empty());
}

async fn create_repo_for_embed_stubs_test(repo: &TempRepo) -> (api::SpecRecipe, api::Spec) {
    let recipe = crate::recipe!({
        "pkg": "my-pkg/1.0.0",
        "install": {
            "embedded": [
                {"pkg": "my-embedded-pkg/1.0.0"}
            ]
        }
    });
    repo.publish_recipe(&recipe).await.unwrap();
    let spec = crate::spec!({
        "pkg": "my-pkg/1.0.0/7CI5R7Y4",
        "install": {
            "embedded": [
                {"pkg": "my-embedded-pkg/1.0.0/embedded"}
            ]
        }
    });
    repo.publish_package(
        &spec,
        &vec![(api::Component::Run, empty_layer_digest())]
            .into_iter()
            .collect(),
    )
    .await
    .unwrap();
    (recipe, spec)
}

#[rstest]
#[case::spfs(RepoKind::Spfs)]
#[tokio::test]
async fn test_repo_publish_spec_updates_embed_stubs(#[case] repo: RepoKind) {
    let repo = make_repo(repo).await;
    let _ = create_repo_for_embed_stubs_test(&repo).await;
    // `test_repo_publish_package_creates_embed_stubs` proves that the stub
    // would exist at this point.
    //
    // Change the embedded package to a different name.
    let recipe = crate::recipe!({
        "pkg": "my-pkg/1.0.0",
        "install": {
            "embedded": [
                {"pkg": "my-embedded-pkg2/1.0.0"}
            ]
        }
    });
    repo.force_publish_recipe(&recipe).await.unwrap();
    let spec = crate::spec!({
        "pkg": "my-pkg/1.0.0/7CI5R7Y4",
        "install": {
            "embedded": [
                {"pkg": "my-embedded-pkg2/1.0.0/embedded"}
            ]
        }
    });
    repo.update_package(&spec).await.unwrap();
    // The original stub should be gone.
    assert!(!repo
        .list_packages()
        .await
        .unwrap()
        .iter()
        .any(|pkg| pkg == "my-embedded-pkg"));
    // The new stub should exist.
    assert!(repo
        .list_packages()
        .await
        .unwrap()
        .iter()
        .any(|pkg| pkg == "my-embedded-pkg2"));
}

#[rstest]
#[case::spfs(RepoKind::Spfs)]
#[tokio::test]
async fn test_repo_deprecate_spec_updates_embed_stubs(#[case] repo: RepoKind) {
    let repo = make_repo(repo).await;
    let (_, mut package) = create_repo_for_embed_stubs_test(&repo).await;
    // `test_repo_publish_package_creates_embed_stubs` proves that the stub
    // would exist at this point.
    //
    // Deprecate the package.
    package.deprecate().unwrap();
    repo.update_package(&package).await.unwrap();
    // The stub should be deprecated too.
    let builds = repo
        .list_package_builds(&api::Ident {
            name: "my-embedded-pkg".parse().unwrap(),
            version: "1.0.0".parse().unwrap(),
            build: None,
        })
        .await
        .unwrap();
    assert!(!builds.is_empty());
    assert!(repo
        .read_embed_stub(&builds[0])
        .await
        .unwrap()
        .is_deprecated())
}

#[rstest]
#[case::spfs(RepoKind::Spfs)]
#[tokio::test]
async fn test_repo_publish_package_creates_embed_stubs(#[case] repo: RepoKind) {
    let repo = make_repo(repo).await;
    let _ = create_repo_for_embed_stubs_test(&repo).await;
    assert!(repo
        .list_packages()
        .await
        .unwrap()
        .iter()
        .any(|pkg| pkg == "my-embedded-pkg"));
}

#[rstest]
#[case::spfs(RepoKind::Spfs)]
#[tokio::test]
async fn test_repo_remove_package_removes_embed_stubs(#[case] repo: RepoKind) {
    let repo = make_repo(repo).await;
    let (_, spec) = create_repo_for_embed_stubs_test(&repo).await;
    // `test_repo_publish_package_creates_embed_stubs` proves that the stub
    // would exist at this point.
    repo.remove_package(spec.ident()).await.unwrap();
    assert!(!repo
        .list_packages()
        .await
        .unwrap()
        .iter()
        .any(|pkg| pkg == "my-embedded-pkg"));
}
