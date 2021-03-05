use chrono::{TimeZone, Utc};
use rstest::rstest;

use super::{get_prunable_tags, prune_tags, PruneParameters};
use crate::{encoding, storage, tracking, Error};
use std::collections::HashMap;

fixtures!();

#[rstest]
fn test_prunable_tags_age(tmprepo: TempRepo) {
    let (_td, mut tmprepo) = tmprepo;
    let mut old = tracking::Tag::new(
        Some("testing".to_string()),
        "prune",
        encoding::NULL_DIGEST.into(),
    )
    .unwrap();
    old.parent = encoding::NULL_DIGEST.into();
    old.time = Utc.timestamp(10000, 0);
    let cutoff = Utc.timestamp(20000, 0);
    let mut new = tracking::Tag::new(
        Some("testing".to_string()),
        "prune",
        encoding::EMPTY_DIGEST.into(),
    )
    .unwrap();
    new.parent = encoding::EMPTY_DIGEST.into();
    new.time = Utc.timestamp(30000, 0);
    tmprepo.push_raw_tag(&old).unwrap();
    tmprepo.push_raw_tag(&new).unwrap();

    let tags = get_prunable_tags(
        &mut tmprepo,
        &PruneParameters {
            prune_if_older_than: Some(cutoff.clone()),
            ..Default::default()
        },
    )
    .unwrap();
    assert!(tags.contains(&old));
    assert!(!tags.contains(&new));

    let tags = get_prunable_tags(
        &mut tmprepo,
        &PruneParameters {
            prune_if_older_than: Some(cutoff.clone()),
            keep_if_newer_than: Some(Utc.timestamp(0, 0)),
            ..Default::default()
        },
    )
    .unwrap();
    assert!(!tags.contains(&old), "should prefer to keep when ambiguous");
    assert!(!tags.contains(&new));
}

#[rstest]
fn test_prunable_tags_version(tmprepo: TempRepo) {
    let (_td, mut tmprepo) = tmprepo;
    let tag = tracking::TagSpec::parse("testing/versioned").unwrap();
    let tag5 = tmprepo
        .push_tag(&tag, &encoding::EMPTY_DIGEST.into())
        .unwrap();
    let tag4 = tmprepo
        .push_tag(&tag, &encoding::NULL_DIGEST.into())
        .unwrap();
    let tag3 = tmprepo
        .push_tag(&tag, &encoding::EMPTY_DIGEST.into())
        .unwrap();
    let tag2 = tmprepo
        .push_tag(&tag, &encoding::NULL_DIGEST.into())
        .unwrap();
    let tag1 = tmprepo
        .push_tag(&tag, &encoding::EMPTY_DIGEST.into())
        .unwrap();
    let tag0 = tmprepo
        .push_tag(&tag, &encoding::NULL_DIGEST.into())
        .unwrap();

    let tags = get_prunable_tags(
        &mut tmprepo,
        &PruneParameters {
            prune_if_version_more_than: Some(2),
            ..Default::default()
        },
    )
    .unwrap();
    assert!(!tags.contains(&tag0));
    assert!(!tags.contains(&tag1));
    assert!(!tags.contains(&tag2));
    assert!(tags.contains(&tag3));
    assert!(tags.contains(&tag4));
    assert!(tags.contains(&tag5));

    let tags = get_prunable_tags(
        &mut tmprepo,
        &PruneParameters {
            prune_if_version_more_than: Some(2),
            keep_if_version_less_than: Some(4),
            ..Default::default()
        },
    )
    .unwrap();
    assert!(!tags.contains(&tag0));
    assert!(!tags.contains(&tag1));
    assert!(!tags.contains(&tag2));
    assert!(
        !tags.contains(&tag3),
        "should prefer to keep in ambiguous situation"
    );
    assert!(tags.contains(&tag4));
    assert!(tags.contains(&tag5));
}

#[rstest]
fn test_prune_tags(tmprepo: TempRepo) {
    let _guard = init_logging();
    let (_td, mut tmprepo) = tmprepo;
    let tag = tracking::TagSpec::parse("test/prune").unwrap();

    fn reset(tmprepo: &mut storage::RepositoryHandle) -> HashMap<i32, tracking::Tag> {
        let tag = tracking::TagSpec::parse("test/prune").unwrap();
        let mut tags = HashMap::new();
        match tmprepo.remove_tag_stream(&tag) {
            Ok(_) | Err(Error::UnknownReference(_)) => (),
            Err(err) => panic!("{:?}", err),
        }

        for year in vec![2020, 2021, 2022, 2023, 2024, 2025].into_iter() {
            let time = Utc.ymd(year, 1, 1).and_hms(0, 0, 0);
            let digest = random_digest();
            let mut tag = tracking::Tag::new(Some("test".into()), "prune", digest).unwrap();
            tag.time = time;
            tmprepo.push_raw_tag(&tag).unwrap();
            tags.insert(year, tag);
        }
        tags
    };

    let tags = reset(&mut tmprepo);
    prune_tags(
        &mut tmprepo,
        &PruneParameters {
            prune_if_older_than: Some(Utc.ymd(2025, 1, 1).and_hms(0, 0, 0)),
            ..Default::default()
        },
    )
    .unwrap();
    for tag in tmprepo.read_tag(&tag).unwrap() {
        assert_eq!(&tag, tags.get(&2025).unwrap(), "should remove all but 2025");
    }

    let tags = reset(&mut tmprepo);
    prune_tags(
        &mut tmprepo,
        &PruneParameters {
            prune_if_version_more_than: Some(2),
            ..Default::default()
        },
    )
    .unwrap();
    for tag in tmprepo.read_tag(&tag).unwrap() {
        assert_ne!(
            &tag,
            tags.get(&2020).unwrap(),
            "should remove 20, 21, and 22"
        );
        assert_ne!(
            &tag,
            tags.get(&2021).unwrap(),
            "should remove 20, 21, and 22"
        );
        assert_ne!(
            &tag,
            tags.get(&2022).unwrap(),
            "should remove 20, 21, and 22"
        );
    }

    let _tags = reset(&mut tmprepo);
    prune_tags(
        &mut tmprepo,
        &PruneParameters {
            prune_if_older_than: Some(Utc.ymd(2030, 1, 1).and_hms(0, 0, 0)),
            ..Default::default()
        },
    )
    .unwrap();
    if let Ok(_) = tmprepo.read_tag(&tag) {
        panic!("should not have any pruned tag left")
    }
}

fn random_digest() -> encoding::Digest {
    use rand::Rng;
    let mut hasher = encoding::Hasher::new();
    let mut rng = rand::thread_rng();
    let mut buf = Vec::with_capacity(64);
    buf.resize(64, 0);
    rng.fill(buf.as_mut_slice());
    hasher.update(&buf.as_slice());
    hasher.digest()
}