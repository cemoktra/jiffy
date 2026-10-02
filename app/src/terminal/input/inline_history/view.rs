//! Inline history menu view for up-arrow command history.
use std::collections::HashSet;

use warpui::elements::ChildView;
use warpui::{AppContext, Element, Entity, ModelHandle, View, ViewContext, ViewHandle};

use crate::features::FeatureFlag;
use crate::search::data_source::{Query, QueryFilter};
use crate::search::mixer::SearchMixer;
use crate::terminal::input::buffer_model::InputBufferModel;
use crate::terminal::input::inline_history::data_source::{
    AcceptHistoryItem, InlineHistoryMenuDataSource,
};
use crate::terminal::input::inline_menu::{
    InlineMenuEvent, InlineMenuHeaderConfig, InlineMenuModel, InlineMenuPositioner,
    InlineMenuTabConfig, InlineMenuView,
};
use crate::terminal::input::suggestions_mode_model::{
    InputSuggestionsModeEvent, InputSuggestionsModeModel,
};
use crate::terminal::model::session::active_session::ActiveSession;

#[derive(Debug, Clone)]
pub enum InlineHistoryMenuEvent {
    AcceptCommand {
        command: String,
    },
    SelectCommand {
        command: String,
    },
    NoResults,
    /// Emitted when the inline menu should be closed and additionally restore the
    /// original input buffer contents.
    Close,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HistoryTab {
    All,
}

pub struct InlineHistoryMenuView {
    menu_view: ViewHandle<InlineMenuView<AcceptHistoryItem, HistoryTab>>,
    mixer: ModelHandle<SearchMixer<AcceptHistoryItem>>,
    model: ModelHandle<InlineMenuModel<AcceptHistoryItem, HistoryTab>>,
    buffer_model: ModelHandle<InputBufferModel>,
}

impl InlineHistoryMenuView {
    pub fn new(
        active_session: ModelHandle<ActiveSession>,
        input_suggestions_model: &ModelHandle<InputSuggestionsModeModel>,
        positioner: &ModelHandle<InlineMenuPositioner>,
        buffer_model: ModelHandle<InputBufferModel>,
        ctx: &mut ViewContext<Self>,
    ) -> Self {
        let data_source = ctx.add_model(|_| InlineHistoryMenuDataSource::new(active_session));
        let tab_configs = vec![InlineMenuTabConfig {
            id: HistoryTab::All,
            label: "All".to_string(),
            filters: HashSet::new(),
        }];

        let mixer = ctx.add_model(|ctx| {
            let mut mixer = SearchMixer::<AcceptHistoryItem>::new();
            mixer.add_sync_source(data_source, [QueryFilter::Commands]);
            mixer.run_query(
                Query {
                    text: String::new(),
                    filters: HashSet::new(),
                },
                ctx,
            );
            mixer
        });

        let menu_view = ctx.add_typed_action_view(|ctx| {
            let menu = InlineMenuView::new_with_tabs(
                mixer.clone(),
                positioner.clone(),
                input_suggestions_model,
                tab_configs,
                None,
                ctx,
            );
            if FeatureFlag::InlineMenuHeaders.is_enabled() {
                menu.with_header_config(InlineMenuHeaderConfig {
                    label: "History".to_string(),
                    trailing_element: None,
                })
            } else {
                menu
            }
        });
        let model = menu_view.as_ref(ctx).model().clone();

        ctx.subscribe_to_model(input_suggestions_model, |me, model, event, ctx| {
            let InputSuggestionsModeEvent::ModeChanged { .. } = event;
            if model.as_ref(ctx).is_inline_history_menu() {
                me.open_with_current_buffer(ctx);
            }
        });

        ctx.subscribe_to_view(&menu_view, |me, _, event, ctx| match event {
            InlineMenuEvent::AcceptedItem { item, .. } => match item {
                AcceptHistoryItem::Command { command } => {
                    ctx.emit(InlineHistoryMenuEvent::AcceptCommand {
                        command: command.clone(),
                    });
                }
            },
            InlineMenuEvent::SelectedItem { item } => match item {
                AcceptHistoryItem::Command { command } => {
                    ctx.emit(InlineHistoryMenuEvent::SelectCommand {
                        command: command.clone(),
                    });
                }
            },
            InlineMenuEvent::Dismissed => {
                ctx.emit(InlineHistoryMenuEvent::Close);
            }
            InlineMenuEvent::NoResults => {
                ctx.emit(InlineHistoryMenuEvent::NoResults);
            }
            InlineMenuEvent::TabChanged => {
                me.open_with_current_buffer(ctx);
            }
        });

        Self {
            menu_view,
            mixer,
            model,
            buffer_model,
        }
    }

    /// Returns the model handle for external use (e.g., by message bars).
    pub fn model(&self) -> &ModelHandle<InlineMenuModel<AcceptHistoryItem, HistoryTab>> {
        &self.model
    }

    pub fn result_count(&self, app: &AppContext) -> usize {
        self.menu_view.as_ref(app).result_count()
    }

    pub fn select_up(&self, ctx: &mut ViewContext<Self>) {
        self.menu_view.update(ctx, |v, ctx| v.select_up(ctx));
    }

    pub fn select_down(&self, ctx: &mut ViewContext<Self>) {
        let should_close = self.menu_view.read(ctx, |v, _| {
            let result_count = v.result_count();
            let is_last_item_selected =
                result_count > 0 && v.selected_idx().is_some_and(|idx| idx == result_count - 1);
            is_last_item_selected || result_count == 0
        });

        if should_close {
            ctx.emit(InlineHistoryMenuEvent::Close);
        } else {
            self.menu_view.update(ctx, |v, ctx| v.select_down(ctx));
        }
    }

    pub fn accept_selected_item(&self, ctx: &mut ViewContext<Self>) {
        self.menu_view
            .update(ctx, |v, ctx| v.accept_selected_item(false, ctx));
    }

    fn open_with_current_buffer(&mut self, ctx: &mut ViewContext<Self>) {
        let query_text = self.buffer_model.as_ref(ctx).current_value().to_owned();
        let filters = self.model.as_ref(ctx).active_tab_filters();
        self.mixer.update(ctx, |mixer, ctx| {
            mixer.run_query(
                Query {
                    text: query_text,
                    filters,
                },
                ctx,
            );
        });
    }
}

impl View for InlineHistoryMenuView {
    fn ui_name() -> &'static str {
        "InlineHistoryMenuView"
    }

    fn render(&self, _app: &warpui::AppContext) -> Box<dyn Element> {
        ChildView::new(&self.menu_view).finish()
    }
}

impl Entity for InlineHistoryMenuView {
    type Event = InlineHistoryMenuEvent;
}
