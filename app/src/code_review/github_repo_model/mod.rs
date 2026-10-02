use warpui::{AppContext, Entity, ModelContext, ModelHandle};

#[cfg(feature = "local_fs")]
mod local;
#[cfg(feature = "local_fs")]
pub use local::LocalGitHubRepoModel;

#[cfg(all(test, feature = "local_fs"))]
use crate::code_review::git_repo_model::GitRepoStatusModel;
use crate::util::git::{PrInfo, RepositoryInfo};

#[cfg_attr(not(feature = "local_fs"), allow(dead_code))]
#[derive(Debug)]
pub enum GitHubRepoEvent {
    /// Emitted when `pr_info` changes value (fetch result differs from
    /// cached, branch change cleared the cache, etc.).
    PrInfoChanged,
    /// Emitted when `repository_info` changes value.
    RepositoryInfoChanged,
}

/// Per-repo GitHub-info model.
///
/// Consumers (prompt chips, code review, agent context) hold a `ModelHandle<GitHubRepoModel>`
/// and subscribe to its [`GitHubRepoEvent`]s.
pub struct GitHubRepoModel {
    #[cfg(feature = "local_fs")]
    local: ModelHandle<LocalGitHubRepoModel>,
}
impl Entity for GitHubRepoModel {
    type Event = GitHubRepoEvent;
}
#[cfg_attr(not(feature = "local_fs"), allow(unused_variables))]
impl GitHubRepoModel {
    #[cfg(feature = "local_fs")]
    pub(crate) fn new(
        local: ModelHandle<LocalGitHubRepoModel>,
        ctx: &mut ModelContext<Self>,
    ) -> Self {
        ctx.subscribe_to_model(&local, |me, _, event, ctx| me.forward_event(event, ctx));
        Self { local }
    }

    fn forward_event(&mut self, event: &GitHubRepoEvent, ctx: &mut ModelContext<Self>) {
        match event {
            GitHubRepoEvent::PrInfoChanged => ctx.emit(GitHubRepoEvent::PrInfoChanged),
            GitHubRepoEvent::RepositoryInfoChanged => {
                ctx.emit(GitHubRepoEvent::RepositoryInfoChanged)
            }
        }
    }

    /// PR info for the current branch.
    pub fn pr_info<'a>(&self, ctx: &'a AppContext) -> Option<&'a PrInfo> {
        #[cfg(feature = "local_fs")]
        return self.local.as_ref(ctx).pr_info();
        #[cfg(not(feature = "local_fs"))]
        None
    }

    /// Repository info (name/owner) returned by `gh repo view`.
    pub fn repository_info<'a>(&self, ctx: &'a AppContext) -> Option<&'a RepositoryInfo> {
        #[cfg(feature = "local_fs")]
        return self.local.as_ref(ctx).repository_info();
        #[cfg(not(feature = "local_fs"))]
        None
    }

    /// Whether a `gh pr view` fetch is currently in flight.
    pub fn is_refreshing_pr_info(&self, ctx: &AppContext) -> bool {
        #[cfg(feature = "local_fs")]
        return self.local.as_ref(ctx).is_refreshing_pr_info();
        #[cfg(not(feature = "local_fs"))]
        false
    }

    /// Force a PR info refresh (e.g. after a `gh`/`gt` command completes).
    pub fn refresh_pr_info(&self, ctx: &mut ModelContext<Self>) {
        #[cfg(feature = "local_fs")]
        self.local.update(ctx, |m, ctx| m.refresh_pr_info(ctx));
    }

    /// Force a repository-info refresh.
    pub fn refresh_repository_info(&self, ctx: &mut ModelContext<Self>) {
        #[cfg(feature = "local_fs")]
        self.local
            .update(ctx, |m, ctx| m.refresh_repository_info(ctx));
    }
}

#[cfg(all(test, feature = "local_fs"))]
impl GitHubRepoModel {
    /// Wraps an inert test model.
    pub(crate) fn new_local_for_test(
        git_status: ModelHandle<GitRepoStatusModel>,
        ctx: &mut ModelContext<Self>,
    ) -> Self {
        let inner = ctx.add_model(move |_| LocalGitHubRepoModel::new_for_test(git_status));
        Self::new(inner, ctx)
    }

    pub(crate) fn set_pr_info_for_test(
        &mut self,
        pr_info: Option<PrInfo>,
        ctx: &mut ModelContext<Self>,
    ) {
        self.local
            .update(ctx, |m, ctx| m.set_pr_info_for_test(pr_info, ctx));
    }
}
