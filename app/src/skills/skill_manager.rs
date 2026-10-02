#[path = "file_watchers/mod.rs"]
mod file_watchers;
use std::collections::{HashMap, HashSet};

use agent_context::skills::{
    ParsedSkill, SkillProvider, SkillReference, SkillScope, provider_rank,
};
pub use file_watchers::{SkillWatcher, SkillWatcherEvent, extract_skill_parent_directory};
use warp_util::local_or_remote_path::LocalOrRemotePath;
use warpui::{AppContext, Entity, ModelContext, ModelHandle, SingletonEntity};

use super::skill_utils::SkillDeduplicator;
use super::{SkillDescriptor, SkillManagerEvent};

pub struct SkillManager {
    /// Maps a directory path to the set of skill file paths defined in that directory.
    ///
    /// The key is the directory containing the `.agents/skills/` (or similar provider) folder,
    /// not the skills folder itself.
    ///
    /// Example: For a skill at `/repo/frontend/.agents/skills/deploy/SKILL.md`:
    /// - Key: `/repo/frontend`
    /// - Value (in the set): `/repo/frontend/.agents/skills/deploy/SKILL.md`
    ///
    /// NOT:
    /// - Key: `/repo/frontend/.agents/skills`
    directory_skills: HashMap<LocalOrRemotePath, HashSet<LocalOrRemotePath>>,
    skills_by_path: HashMap<LocalOrRemotePath, ParsedSkill>,
    /// Reverse lookup: skill name → set of paths with that name.
    /// This allows efficient lookup by skill name without scanning all paths.
    skills_by_name: HashMap<String, HashSet<LocalOrRemotePath>>,
    #[allow(dead_code)]
    skill_watcher: ModelHandle<SkillWatcher>, // Can't remove this or it'll get cleaned up after new()
}

impl SkillManager {
    pub fn new(ctx: &mut ModelContext<Self>) -> Self {
        let (skill_watcher_tx, skill_watcher_rx) = async_channel::unbounded();

        ctx.spawn_stream_local(
            skill_watcher_rx,
            |me, message, ctx| {
                me.handle_skill_watcher_event(message, ctx);
            },
            |_, _| {}, // No cleanup needed when stream ends
        );

        // Create skill watcher
        let skill_watcher = ctx.add_model(|ctx| SkillWatcher::new(ctx, skill_watcher_tx));

        Self {
            directory_skills: HashMap::new(),
            skills_by_path: HashMap::new(),
            skills_by_name: HashMap::new(),
            skill_watcher,
        }
    }

    /// Returns skills available for the given working directory: home skills for local working
    /// directories plus project skills from the working directory and its ancestors within the
    /// enclosing repository.
    pub fn get_skills_for_working_directory(
        &self,
        working_directory: Option<&LocalOrRemotePath>,
        ctx: &AppContext,
    ) -> Vec<SkillDescriptor> {
        // Collect file-backed skills for one shared deduplication pass. Home skills use
        // the home directory as their dir_path; project skills use their owning directory.
        let mut skill_paths = Vec::new();
        let mut deduplicator = SkillDeduplicator::default();

        let is_remote = matches!(working_directory, Some(LocalOrRemotePath::Remote(_)));
        if !is_remote
            && let Some(home_dir) = local_home_directory()
            && let Some(home_skill_paths) = self.directory_skills.get(&home_dir)
        {
            skill_paths.extend(
                home_skill_paths
                    .iter()
                    .cloned()
                    .map(|path| (home_dir.clone(), path)),
            );
        }

        if let Some(working_directory) = working_directory {
            let repo_root = repo_metadata::repositories::DetectedRepositories::as_ref(ctx)
                .get_root_for_path(working_directory);

            for (dir, dir_skill_paths) in &self.directory_skills {
                if is_local_home_directory(dir) {
                    continue;
                }
                // Only include skills from directories that are ancestors of the working directory
                // (or the working directory itself)
                if working_directory.starts_with(dir) {
                    // Also verify this directory is within the detected repo (if any)
                    if repo_root.as_ref().is_none_or(|root| dir.starts_with(root)) {
                        for path in dir_skill_paths {
                            skill_paths.push((dir.clone(), path.clone()));
                        }
                    }
                }
            }
        }

        // Deduplicate skills with identical content installed under the same directory across
        // multiple providers, keeping the skill from the highest-priority provider per
        // [`SKILL_PROVIDER_DEFINITIONS`].
        deduplicator.extend_paths(&skill_paths, &self.skills_by_path);
        let mut skills = deduplicator.into_descriptors();

        // Apply icon overrides for well-known skill names (e.g. partner integrations).
        for skill in &mut skills {
            if skill.icon_override.is_none() {
                skill.icon_override = super::icon_override_for_skill_name(&skill.name);
            }
        }

        skills
    }

    /// Returns true if the skill (or any of its provider-path variants) exists in
    /// a folder matching one of the given `providers`. This handles the deduplication
    /// edge case where a skill is present in multiple provider folders (e.g. both
    /// `.agents/skills/` and `.claude/skills/`) but deduplication picked a provider
    /// that the caller doesn't support.
    pub fn skill_exists_for_any_provider(
        &self,
        skill: &SkillDescriptor,
        providers: &[SkillProvider],
    ) -> bool {
        // Fast path: the deduplicated provider already matches.
        if providers.contains(&skill.provider) {
            return true;
        }
        // Slow path: check all paths for this skill name.
        self.providers_for_descriptor(skill)
            .any(|provider| providers.contains(&provider))
    }

    /// Returns the best supported provider for a skill given a set of supported providers.
    ///
    /// When a skill is duplicated across multiple provider folders (e.g. both
    /// `.agents/skills/` and `.claude/skills/`), the global deduplication picks the
    /// highest-priority provider per [`SKILL_PROVIDER_DEFINITIONS`]. However, for the
    /// CLI agent footer `/skills` menu we want the icon to reflect the provider that
    /// the active CLI agent actually supports.
    ///
    /// This method checks all paths for the skill name and returns the supported
    /// provider with the best (lowest) rank. Falls back to the skill's deduped
    /// provider if no supported provider is found among its paths.
    pub fn best_supported_provider(
        &self,
        skill: &SkillDescriptor,
        supported_providers: &[SkillProvider],
    ) -> SkillProvider {
        // Fast path: the deduplicated provider is already supported.
        if supported_providers.contains(&skill.provider) {
            return skill.provider;
        }
        // Find the supported provider with the best (lowest) rank among all paths.
        self.providers_for_descriptor(skill)
            .filter(|provider| supported_providers.contains(provider))
            .min_by_key(|provider| provider_rank(*provider))
            .unwrap_or(skill.provider)
    }

    fn providers_for_descriptor<'a>(
        &'a self,
        descriptor: &'a SkillDescriptor,
    ) -> impl Iterator<Item = SkillProvider> + 'a {
        self.skills_by_name
            .get(&descriptor.name)
            .into_iter()
            .flatten()
            .filter(|path| path_matches_reference_location(path, &descriptor.reference))
            .filter_map(|path| self.skills_by_path.get(path).map(|skill| skill.provider))
    }

    fn remove_skill_by_path(&mut self, skill_path: &LocalOrRemotePath) {
        let Some(skill) = self.skills_by_path.remove(skill_path) else {
            return;
        };
        let remove_name = self
            .skills_by_name
            .get_mut(&skill.name)
            .is_some_and(|paths| {
                paths.remove(skill_path);
                paths.is_empty()
            });
        if remove_name {
            self.skills_by_name.remove(&skill.name);
        }
    }

    fn handle_skill_watcher_event(
        &mut self,
        event: SkillWatcherEvent,
        ctx: &mut ModelContext<Self>,
    ) {
        let home_skills_changed = match &event {
            SkillWatcherEvent::SkillsAdded { skills } => {
                skills.iter().any(|skill| skill.scope == SkillScope::Home)
            }
            SkillWatcherEvent::SkillsDeleted { paths } => paths.iter().any(|path| {
                self.skills_by_path.values().any(|skill| {
                    skill.scope == SkillScope::Home
                        && (skill.path.starts_with(path) || path.starts_with(&skill.path))
                })
            }),
        };
        match event {
            SkillWatcherEvent::SkillsAdded { skills } => {
                self.handle_skills_added(skills);
            }
            SkillWatcherEvent::SkillsDeleted { paths } => {
                self.handle_skills_deleted(paths);
            }
        }
        ctx.emit(SkillManagerEvent::SkillsChanged {
            home_skills_changed,
        });
    }

    pub fn handle_skills_added(&mut self, skills: Vec<ParsedSkill>) {
        for skill in skills {
            match extract_skill_parent_directory(&skill.path) {
                Ok(parent_dir) => {
                    self.directory_skills
                        .entry(parent_dir)
                        .or_default()
                        .insert(skill.path.clone());

                    self.skills_by_name
                        .entry(skill.name.clone())
                        .or_default()
                        .insert(skill.path.clone());
                    self.skills_by_path.insert(skill.path.clone(), skill);
                }
                _ => {
                    log::warn!(
                        "Could not extract parent directory for skill: {:?}",
                        skill.path
                    );
                }
            }
        }
    }

    fn handle_skills_deleted(&mut self, paths: Vec<LocalOrRemotePath>) {
        for path in paths {
            self.handle_path_deleted(&path);
        }
    }

    fn handle_path_deleted(&mut self, path: &LocalOrRemotePath) {
        // Delete all skills that are affected by this deleted path
        for (dir, skill_paths) in &self.directory_skills.clone() {
            if dir.starts_with(path) {
                // Delete this entire entry and remove all skill_paths under this directory from cache
                for skill_path in skill_paths {
                    self.remove_skill_by_path(skill_path);
                }
                self.directory_skills.remove(dir);
            } else if path.starts_with(dir) {
                // Remove all skills under this directory that is a child of the deleted path
                for skill_path in skill_paths {
                    if skill_path.starts_with(path) {
                        self.remove_skill_by_path(skill_path);
                        self.directory_skills
                            .entry(dir.clone())
                            .or_default()
                            .remove(skill_path);
                    }
                }
            }
        }
    }

    /// Adds a skill to the skill manager for testing purposes.
    #[cfg(test)]
    pub fn add_skill_for_testing(&mut self, skill: ParsedSkill) {
        let path = skill.path.clone();
        let name = skill.name.clone();
        self.skills_by_path.insert(path.clone(), skill);
        self.skills_by_name.entry(name).or_default().insert(path);
    }
}
impl Entity for SkillManager {
    type Event = SkillManagerEvent;
}

impl SingletonEntity for SkillManager {}

fn path_matches_reference_location(path: &LocalOrRemotePath, reference: &SkillReference) -> bool {
    match (path, reference) {
        (
            LocalOrRemotePath::Remote(path),
            SkillReference::Path(LocalOrRemotePath::Remote(reference)),
        ) => path.host_id == reference.host_id,
        (LocalOrRemotePath::Local(_), SkillReference::Path(LocalOrRemotePath::Local(_))) => true,
        (LocalOrRemotePath::Local(_), SkillReference::Path(LocalOrRemotePath::Remote(_)))
        | (LocalOrRemotePath::Remote(_), SkillReference::Path(LocalOrRemotePath::Local(_))) => {
            false
        }
    }
}

fn local_home_directory() -> Option<LocalOrRemotePath> {
    dirs::home_dir().map(LocalOrRemotePath::Local)
}

fn is_local_home_directory(path: &LocalOrRemotePath) -> bool {
    match path {
        LocalOrRemotePath::Local(path) => dirs::home_dir().as_ref() == Some(path),
        LocalOrRemotePath::Remote(_) => false,
    }
}

#[cfg(test)]
#[path = "skill_manager_tests.rs"]
mod tests;
