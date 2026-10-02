//! Discovery of agent skills (`SKILL.md` files in provider directories such as
//! `.claude/skills`) for the CLI agent rich input's skill and slash-command menus.

cfg_if::cfg_if! {
    if #[cfg(not(feature = "local_fs"))] {
        mod dummy_skill_manager;
        pub use dummy_skill_manager::SkillManager;
    }
}

pub use agent_context::skills::SkillDescriptor;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(target_family = "wasm", allow(dead_code))]
pub enum SkillManagerEvent {
    SkillsChanged { home_skills_changed: bool },
}

mod skill_utils;
pub use skill_utils::icon_override_for_skill_name;

cfg_if::cfg_if! {
    if #[cfg(feature = "local_fs")] {
        mod skill_manager;
        pub use skill_manager::SkillManager;
    }
}
