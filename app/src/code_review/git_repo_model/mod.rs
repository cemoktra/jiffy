use warpui::{AppContext, Entity, ModelContext, ModelHandle};

#[cfg(feature = "local_fs")]
mod jj;
#[cfg(feature = "local_fs")]
mod local;
#[cfg(feature = "local_fs")]
pub use local::LocalGitRepoStatusModel;

use super::diff_state::DiffStats;
pub use super::git_repo_models::GitRepoModels;
use crate::context_chips::display_chip::GitBranchTrackingStatus;

/// Public metadata exposed to consumers — the subset of diff metadata
/// that the git chip (prompt display, agent view footer) needs.
#[cfg_attr(not(feature = "local_fs"), allow(dead_code))]
#[derive(Debug, Clone)]
pub struct GitStatusMetadata {
    pub current_branch_name: String,
    /// What prompt chips show for the current branch; richer than `current_branch_name` in jj
    /// workspaces.
    pub prompt_label: String,
    pub main_branch_name: String,
    pub stats_against_head: DiffStats,
    pub branch_tracking_status: GitBranchTrackingStatus,
}

// ── GitRepoStatusModel ──────────────────────────────────────────────────────

#[derive(Debug)]
pub enum GitRepoStatusEvent {
    /// Emitted whenever the metadata changes (branch name, diff stats, etc.).
    MetadataChanged,
}

/// Per-repo git status model.
///
/// Consumers (prompt chips, tabs, code review, agent context) hold a
/// `ModelHandle<GitRepoStatusModel>` and subscribe to its [`GitRepoStatusEvent`]s.
pub struct GitRepoStatusModel {
    #[cfg(feature = "local_fs")]
    local: ModelHandle<LocalGitRepoStatusModel>,
}

impl Entity for GitRepoStatusModel {
    type Event = GitRepoStatusEvent;
}

#[cfg_attr(not(feature = "local_fs"), allow(dead_code, unused_variables))]
impl GitRepoStatusModel {
    #[cfg(feature = "local_fs")]
    fn new(local: ModelHandle<LocalGitRepoStatusModel>, ctx: &mut ModelContext<Self>) -> Self {
        ctx.subscribe_to_model(&local, |me, _, event, ctx| me.forward_event(event, ctx));
        Self { local }
    }

    fn forward_event(&mut self, event: &GitRepoStatusEvent, ctx: &mut ModelContext<Self>) {
        match event {
            GitRepoStatusEvent::MetadataChanged => ctx.emit(GitRepoStatusEvent::MetadataChanged),
        }
    }

    /// Mode-independent status metadata (branch names + HEAD diff stats).
    pub fn metadata<'a>(&self, ctx: &'a AppContext) -> Option<&'a GitStatusMetadata> {
        #[cfg(feature = "local_fs")]
        return self.local.as_ref(ctx).metadata();
        #[cfg(not(feature = "local_fs"))]
        None
    }

    /// Force a metadata refresh (branch names, diff stats).
    pub fn refresh_metadata(&self, ctx: &mut ModelContext<Self>) {
        #[cfg(feature = "local_fs")]
        self.local.update(ctx, |m, ctx| m.refresh_metadata(ctx));
    }
}

#[cfg(feature = "local_fs")]
pub(super) fn new_local_git_repo_status_model(
    repo_path: std::path::PathBuf,
    repository_model: ModelHandle<repo_metadata::Repository>,
    ctx: &mut ModelContext<GitRepoModels>,
) -> ModelHandle<GitRepoStatusModel> {
    let inner = ctx.add_model(|ctx| LocalGitRepoStatusModel::new(repo_path, repository_model, ctx));
    ctx.add_model(|ctx| GitRepoStatusModel::new(inner, ctx))
}

#[cfg(all(test, feature = "local_fs"))]
impl GitRepoStatusModel {
    /// Wraps a test model.
    pub(crate) fn new_local_for_test(
        repository: ModelHandle<repo_metadata::Repository>,
        metadata: Option<GitStatusMetadata>,
        ctx: &mut ModelContext<Self>,
    ) -> Self {
        let inner =
            ctx.add_model(move |_| LocalGitRepoStatusModel::new_for_test(repository, metadata));
        Self::new(inner, ctx)
    }

    pub(crate) fn set_metadata_for_test(
        &mut self,
        metadata: Option<GitStatusMetadata>,
        ctx: &mut ModelContext<Self>,
    ) {
        self.local
            .update(ctx, |m, ctx| m.set_metadata_for_test(metadata, ctx));
    }
}
