//! Utility functions for working with skills.

use std::collections::HashMap;
use std::collections::hash_map::Entry;
use std::hash::{Hash, Hasher};

use agent_context::skills::{ParsedSkill, provider_rank};
use lazy_static::lazy_static;
use siphasher::sip::SipHasher;
use warp_core::ui::Icon;
use warp_util::local_or_remote_path::LocalOrRemotePath;

use super::SkillDescriptor;

lazy_static! {
    static ref CONTENT_HASHER: SipHasher = SipHasher::new_with_keys(0, 0);
}

/// Tries to insert or update a skill descriptor in the deduplication map.
/// If a skill with the same (directory, content) key already exists, keeps the one
/// from the higher-priority provider based on [`SKILL_PROVIDER_DEFINITIONS`].
#[cfg_attr(not(feature = "local_fs"), allow(dead_code))]
fn try_insert_skill(
    dedup_map: &mut HashMap<u64, SkillDescriptor>,
    descriptor: SkillDescriptor,
    dir_path: &LocalOrRemotePath,
    content: &str,
) {
    let mut hasher = *CONTENT_HASHER;
    // Hash the directory path and content to create a unique key for deduplication.
    dir_path.hash(&mut hasher);
    content.hash(&mut hasher);
    let key = hasher.finish();
    match dedup_map.entry(key) {
        Entry::Vacant(e) => {
            e.insert(descriptor);
        }
        Entry::Occupied(mut e) => {
            // Prefer the skill from the higher-priority provider.
            if provider_rank(descriptor.provider) < provider_rank(e.get().provider) {
                e.insert(descriptor);
            }
        }
    }
}

/// Accumulates file-backed skills from one or more catalogs and keeps the best
/// representative for each owning-directory-and-content pair.
#[derive(Default)]
#[cfg_attr(not(feature = "local_fs"), allow(dead_code))]
pub(crate) struct SkillDeduplicator {
    dedup_map: HashMap<u64, SkillDescriptor>,
}

#[cfg_attr(not(feature = "local_fs"), allow(dead_code))]
impl SkillDeduplicator {
    pub(crate) fn insert(&mut self, dir_path: &LocalOrRemotePath, skill: &ParsedSkill) {
        try_insert_skill(
            &mut self.dedup_map,
            SkillDescriptor::from(skill.clone()),
            dir_path,
            &skill.content,
        );
    }

    pub(crate) fn extend_paths(
        &mut self,
        skill_paths: &[(LocalOrRemotePath, LocalOrRemotePath)],
        skills_by_path: &HashMap<LocalOrRemotePath, ParsedSkill>,
    ) {
        for (dir_path, path) in skill_paths {
            if let Some(skill) = skills_by_path.get(path) {
                self.insert(dir_path, skill);
            }
        }
    }

    pub(crate) fn into_descriptors(self) -> Vec<SkillDescriptor> {
        self.dedup_map.into_values().collect()
    }
}

/// Deduplicates paths from one indexed catalog when identical content is installed under the
/// same directory across multiple providers, keeping the single best representative per
/// [`SKILL_PROVIDER_DEFINITIONS`] (index 0 = highest priority).
///
/// Two skills are considered duplicates only when they share the same owning directory
/// **and** identical content — which is the common case when a tool like `npx skills`
/// symlinks the same skill under `~/.agents/skills/`, `~/.warp/skills/`, `~/.claude/skills/`, etc.
///
/// Each element of `skill_paths` is a `(dir_path, skill_file_path)` tuple where
/// `dir_path` is the directory that owns the skill.
#[cfg(test)]
pub(crate) fn unique_skills(
    skill_paths: &[(LocalOrRemotePath, LocalOrRemotePath)],
    skills_by_path: &HashMap<LocalOrRemotePath, ParsedSkill>,
) -> Vec<SkillDescriptor> {
    let mut deduplicator = SkillDeduplicator::default();
    deduplicator.extend_paths(skill_paths, skills_by_path);
    deduplicator.into_descriptors()
}

/// Returns a branded icon override for well-known skill names.
pub fn icon_override_for_skill_name(name: &str) -> Option<Icon> {
    match name {
        "stripe-projects-cli" => Some(Icon::StripeLogo),
        _ => None,
    }
}

#[cfg(test)]
#[path = "skill_utils_tests.rs"]
mod tests;
