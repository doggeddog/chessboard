use std::fmt;
use std::time::{Duration, Instant};

use iced::widget::{
    button, canvas, checkbox, column, container, pick_list, row, scrollable, text,
};
use iced::{
    Alignment, Background, Border, Color, Element, Length, Point, Rectangle, Size, Subscription,
    Task,
};
use iced::{mouse, Theme};

use xq_core::{
    check_move_legality, map_pos_with_flip, Board, GameRecord, Move, MoveOutcome, Pos, Side,
    BOARD_COLS, BOARD_ROWS,
};

use xq_engine::{
    create_engine, query_chessdb, ChessDbResponse, EngineEvent, EngineInfo, EngineProfile,
    EngineProtocol, EngineScore, SearchParams,
};
use xq_link::{
    check_input_permission, EnigoInjector, ExternalUpdate, InputPermissionStatus, LinkRuntime,
    LinkRuntimeConfig, LinkWindow, LinkWindowInfo, PermissionCheck, SyncConfig, SyncPolicy,
    SyncState,
};
use xq_vision::model::ModelPaths;
use xq_vision::pipeline::{PipelineConfig, VisionPipeline};

use crate::resources::{ResourceCheckReport, ResourcePaths};

const SIDEBAR_WIDTH: f32 = 320.0;
const GRID_SPACING: f32 = 54.0;
const BOARD_PADDING: f32 = 24.0;
const PIECE_RADIUS: f32 = 20.0;
const HIGHLIGHT_RADIUS: f32 = 23.0;
const AI_MIN_INTERVAL_MS: u64 = 120;
const AI_MAX_INTERVAL_MS: u64 = 1500;
const AI_DEFAULT_INTERVAL_MS: u64 = 420;

pub struct App {
    resource_paths: Option<ResourcePaths>,
    resources: ResourceCheckReport,
    mode: Mode,
    record: GameRecord,
    redo_stack: Vec<MoveOutcome>,
    selection: Option<Pos>,
    suggested_move: Option<Move>,
    learning_enabled: bool,
    tips: TipsState,
    sidebar_tab: SidebarTab,
    sidebar_collapsed: bool,
    flip_board: bool,
    preserve_on_mode_switch: bool,
    status: String,
    ai_paused: bool,
    ai_interval: Duration,
    last_ai_tick: Option<Instant>,
    engine_busy: bool,
    engine_enabled: bool,
    cloud_enabled: bool,
    last_analysis: Option<EngineAnalysis>,
    link_windows: Vec<WindowOption>,
    selected_window: Option<WindowOption>,
    link_runtime: Option<LinkRuntime<EnigoInjector>>,
    link_running: bool,
    link_interval: Duration,
    link_last_error: Option<String>,
    input_permission: PermissionCheck,
}

pub fn run(settings: iced::Settings, window: iced::window::Settings) -> iced::Result {
    iced::application(App::boot, App::update, App::view)
        .subscription(App::subscription)
        .title(App::title)
        .settings(settings)
        .window(window)
        .run()
}

#[derive(Debug, Clone)]
enum Message {
    ModeSelected(ModeOption),
    NewGame,
    Undo,
    Redo,
    Hint,
    ToggleFlip,
    ToggleSidebar,
    SidebarTabSelected(SidebarTab),
    ToggleLearning(bool),
    CellPressed(Pos),
    AiTogglePause,
    AiStep,
    AiSpeedUp,
    AiSpeedDown,
    Tick(Instant),
    RequestWindowList,
    WindowListLoaded(Result<Vec<LinkWindowInfo>, String>),
    WindowSelected(WindowOption),
    ToggleLink,
    LinkTick(Instant),
    ToggleEngine(bool),
    ToggleCloud(bool),
    EngineFinished(EngineResult),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SidebarTab {
    Analysis,
    Record,
    Tips,
}

impl SidebarTab {
    fn label(self) -> &'static str {
        match self {
            Self::Analysis => "分析",
            Self::Record => "棋谱",
            Self::Tips => "提示",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModeOption {
    LinkAnalysis,
    LinkBattle,
    HumanRed,
    HumanBlack,
    AiVsAi,
}

impl ModeOption {
    fn to_mode(self) -> Mode {
        match self {
            Self::LinkAnalysis => Mode::LinkAnalysis,
            Self::LinkBattle => Mode::LinkBattle,
            Self::HumanRed => Mode::HumanVsAi { human_side: Side::Red },
            Self::HumanBlack => Mode::HumanVsAi { human_side: Side::Black },
            Self::AiVsAi => Mode::AiVsAi,
        }
    }
}

impl fmt::Display for ModeOption {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::LinkAnalysis => write!(f, "连线分析"),
            Self::LinkBattle => write!(f, "连线对战"),
            Self::HumanRed => write!(f, "人机对弈（我方红）"),
            Self::HumanBlack => write!(f, "人机对弈（我方黑）"),
            Self::AiVsAi => write!(f, "AI vs AI"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    LinkAnalysis,
    LinkBattle,
    HumanVsAi { human_side: Side },
    AiVsAi,
}

impl Mode {
    fn label(self) -> &'static str {
        match self {
            Self::LinkAnalysis => "连线分析",
            Self::LinkBattle => "连线对战",
            Self::HumanVsAi { .. } => "人机对弈",
            Self::AiVsAi => "AI vs AI",
        }
    }

    fn allows_flip(self) -> bool {
        !matches!(self, Self::LinkAnalysis | Self::LinkBattle)
    }

    fn allows_manual_move(self, side_to_move: Side) -> bool {
        match self {
            Self::LinkAnalysis => false,
            Self::LinkBattle => true,
            Self::HumanVsAi { human_side } => human_side == side_to_move,
            Self::AiVsAi => false,
        }
    }

    fn ai_side(self) -> Option<Side> {
        match self {
            Self::HumanVsAi { human_side } => Some(human_side.opposite()),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, Default)]
struct TipsState {
    red: Option<Move>,
    black: Option<Move>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowOption {
    info: LinkWindowInfo,
}

impl WindowOption {
    fn new(info: LinkWindowInfo) -> Self {
        Self { info }
    }

    fn info(&self) -> &LinkWindowInfo {
        &self.info
    }
}

impl fmt::Display for WindowOption {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} · {} ({}x{})",
            self.info.title, self.info.app_name, self.info.width, self.info.height
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EnginePurpose {
    Hint,
    AiMove,
    Analysis,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EngineSource {
    Cloud,
    Engine,
}

#[derive(Debug, Clone)]
struct EngineAnalysis {
    source: EngineSource,
    bestmove: Option<Move>,
    info: Option<EngineInfo>,
    error: Option<String>,
}

#[derive(Debug, Clone)]
struct EngineResult {
    purpose: EnginePurpose,
    analysis: EngineAnalysis,
}

impl App {
    fn boot() -> (Self, Task<Message>) {
        let (resource_paths, resources) = match ResourcePaths::detect() {
            Ok(paths) => {
                let report = paths.self_check();
                report.log();
                (Some(paths), report)
            }
            Err(err) => {
                let report = ResourceCheckReport::from_error(err);
                report.log();
                (None, report)
            }
        };

        let mut app = Self {
            resource_paths,
            resources,
            mode: Mode::HumanVsAi { human_side: Side::Red },
            record: GameRecord::startpos(),
            redo_stack: Vec::new(),
            selection: None,
            suggested_move: None,
            learning_enabled: true,
            tips: TipsState::default(),
            sidebar_tab: SidebarTab::Analysis,
            sidebar_collapsed: false,
            flip_board: false,
            preserve_on_mode_switch: true,
            status: "准备就绪".to_string(),
            ai_paused: false,
            ai_interval: Duration::from_millis(AI_DEFAULT_INTERVAL_MS),
            last_ai_tick: None,
            engine_busy: false,
            engine_enabled: true,
            cloud_enabled: true,
            last_analysis: None,
            link_windows: Vec::new(),
            selected_window: None,
            link_runtime: None,
            link_running: false,
            link_interval: Duration::from_millis(800),
            link_last_error: None,
            input_permission: check_input_permission(),
        };
        app.refresh_tips_and_suggestion();

        let task = Task::perform(async {
            xq_link::list_windows().map_err(|err| err.to_string())
        }, Message::WindowListLoaded);

        (app, task)
    }

    fn title(&self) -> String {
        format!("中国象棋助手 · {}", self.mode.label())
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        let mut task = Task::none();

        match message {
            Message::ModeSelected(option) => {
                task = self.switch_mode(option.to_mode());
            }
            Message::NewGame => task = self.reset_game(),
            Message::Undo => self.undo_move(),
            Message::Redo => self.redo_move(),
            Message::Hint => task = self.request_hint(),
            Message::ToggleFlip => {
                if self.mode.allows_flip() {
                    self.flip_board = !self.flip_board;
                    self.status = if self.flip_board {
                        "棋盘已反转".into()
                    } else {
                        "棋盘已恢复".into()
                    };
                }
            }
            Message::ToggleSidebar => {
                self.sidebar_collapsed = !self.sidebar_collapsed;
            }
            Message::SidebarTabSelected(tab) => self.sidebar_tab = tab,
            Message::ToggleLearning(enabled) => {
                self.learning_enabled = enabled;
                self.refresh_tips_and_suggestion();
                self.status = if enabled {
                    "学习提示已开启".into()
                } else {
                    "学习提示已关闭".into()
                };
            }
            Message::CellPressed(pos) => task = self.handle_cell_press(pos),
            Message::AiTogglePause => {
                if matches!(self.mode, Mode::AiVsAi) {
                    self.ai_paused = !self.ai_paused;
                    self.status = if self.ai_paused {
                        "AI 已暂停".into()
                    } else {
                        "AI 已继续".into()
                    };
                }
            }
            Message::AiStep => {
                if matches!(self.mode, Mode::AiVsAi) {
                    task = self.request_ai_move();
                }
            }
            Message::AiSpeedUp => {
                if matches!(self.mode, Mode::AiVsAi) {
                    let next = self.ai_interval.as_millis().saturating_sub(80) as u64;
                    self.ai_interval =
                        Duration::from_millis(next.clamp(AI_MIN_INTERVAL_MS, AI_MAX_INTERVAL_MS));
                    self.status = format!("AI 速度：{}ms/步", self.ai_interval.as_millis());
                }
            }
            Message::AiSpeedDown => {
                if matches!(self.mode, Mode::AiVsAi) {
                    let next = self.ai_interval.as_millis().saturating_add(120) as u64;
                    self.ai_interval =
                        Duration::from_millis(next.clamp(AI_MIN_INTERVAL_MS, AI_MAX_INTERVAL_MS));
                    self.status = format!("AI 速度：{}ms/步", self.ai_interval.as_millis());
                }
            }
            Message::Tick(now) => {
                if matches!(self.mode, Mode::AiVsAi) && !self.ai_paused {
                    let should_move = match self.last_ai_tick {
                        Some(last) => now.duration_since(last) >= self.ai_interval,
                        None => true,
                    };
                    if should_move {
                        self.last_ai_tick = Some(now);
                        task = self.request_ai_move();
                    }
                }
            }
            Message::RequestWindowList => {
                task = Task::perform(async {
                    xq_link::list_windows().map_err(|err| err.to_string())
                }, Message::WindowListLoaded);
            }
            Message::WindowListLoaded(result) => {
                match result {
                    Ok(list) => {
                        self.link_windows = list.into_iter().map(WindowOption::new).collect();
                        if self.selected_window.is_none() {
                            self.selected_window = self.link_windows.first().cloned();
                        }
                        self.status = format!("已加载 {} 个窗口", self.link_windows.len());
                    }
                    Err(err) => {
                        self.link_windows.clear();
                        self.selected_window = None;
                        self.status = format!("窗口枚举失败：{err}");
                    }
                }
            }
            Message::WindowSelected(option) => {
                self.selected_window = Some(option);
            }
            Message::ToggleLink => {
                if self.link_running {
                    self.stop_link();
                } else {
                    task = self.start_link();
                }
            }
            Message::LinkTick(_now) => {
                task = self.handle_link_tick();
            }
            Message::ToggleEngine(enabled) => {
                self.engine_enabled = enabled;
                self.status = if enabled {
                    "引擎已启用".into()
                } else {
                    "引擎已关闭".into()
                };
            }
            Message::ToggleCloud(enabled) => {
                self.cloud_enabled = enabled;
                self.status = if enabled {
                    "云库已启用".into()
                } else {
                    "云库已关闭".into()
                };
            }
            Message::EngineFinished(result) => {
                task = self.handle_engine_result(result);
            }
        }

        task
    }

    fn view(&self) -> Element<'_, Message> {
        let toolbar = self.view_toolbar();
        let board = self.view_board();
        let sidebar = self.view_sidebar();

        let content = row![board, sidebar]
            .spacing(12)
            .height(Length::Fill)
            .align_y(Alignment::Start);

        let status_bar = container(text(self.status.clone()).size(14))
            .padding(8)
            .width(Length::Fill)
            .style(|_| iced::widget::container::Style {
                text_color: Some(Color::from_rgb(0.25, 0.25, 0.25)),
                background: Some(Background::Color(Color::from_rgb(0.95, 0.95, 0.95))),
                border: Border {
                    color: Color::from_rgb(0.85, 0.85, 0.85),
                    width: 1.0,
                    radius: 0.0.into(),
                },
                ..Default::default()
            });

        column![toolbar, content, status_bar]
            .spacing(12)
            .padding(12)
            .into()
    }

    fn subscription(&self) -> Subscription<Message> {
        let mut subs = Vec::new();
        if matches!(self.mode, Mode::AiVsAi) {
            subs.push(iced::time::every(Duration::from_millis(100)).map(Message::Tick));
        }
        if self.link_running {
            subs.push(iced::time::every(self.link_interval).map(Message::LinkTick));
        }
        Subscription::batch(subs)
    }
}

impl App {
    fn view_toolbar(&self) -> Element<'_, Message> {
        let mode_option = ModeOption::from_mode(self.mode);
        let mode_pick = pick_list(mode_options(), Some(mode_option), Message::ModeSelected)
            .width(Length::Fixed(200.0));

        let new_game = button(text("新局")).on_press(Message::NewGame);
        let undo = button(text("悔棋")).on_press(Message::Undo);
        let redo = button(text("前进")).on_press(Message::Redo);
        let hint = button(text("提示")).on_press(Message::Hint);

        let flip = if self.mode.allows_flip() {
            button(text("翻转")).on_press(Message::ToggleFlip)
        } else {
            button(text("翻转(禁用)"))
        };

        let sidebar_toggle = button(text(if self.sidebar_collapsed { "展开侧栏" } else { "收起侧栏" }))
            .on_press(Message::ToggleSidebar);

        let learning = checkbox(self.learning_enabled)
            .label("学习提示")
            .on_toggle(Message::ToggleLearning);

        row![
            mode_pick,
            new_game,
            undo,
            redo,
            hint,
            flip,
            sidebar_toggle,
            learning
        ]
        .spacing(10)
        .align_y(Alignment::Center)
        .into()
    }

    fn view_board(&self) -> Element<'_, Message> {
        let mut board_column = column![
            text(format!(
                "{}行棋 · {}",
                side_label(self.record.board.side_to_move),
                self.mode.label()
            ))
            .size(18)
        ]
        .spacing(8);

        let canvas_size = board_canvas_size();
        let board_canvas = canvas(BoardCanvas {
            board: self.record.board.clone(),
            flipped: self.flip_board,
            selection: self.selection,
            suggested: self.suggested_move,
        })
        .width(Length::Fixed(canvas_size.width))
        .height(Length::Fixed(canvas_size.height));

        let board_container = container(board_canvas)
            .padding(4)
            .style(|_| iced::widget::container::Style {
                background: Some(Background::Color(Color::from_rgb(0.93, 0.88, 0.78))),
                border: Border {
                    color: Color::from_rgb(0.55, 0.4, 0.25),
                    width: 1.5,
                    radius: 4.0.into(),
                },
                ..Default::default()
            });

        board_column = board_column.push(board_container);

        if matches!(self.mode, Mode::AiVsAi) {
            board_column = board_column.push(self.view_ai_controls());
        }

        container(board_column)
            .width(Length::Shrink)
            .align_x(iced::alignment::Horizontal::Center)
            .into()
    }

    fn view_ai_controls(&self) -> Element<'_, Message> {
        let pause_label = if self.ai_paused { "继续" } else { "暂停" };
        let speed_label = format!("{}ms/步", self.ai_interval.as_millis());

        row![
            button(text(pause_label)).on_press(Message::AiTogglePause),
            button(text("单步")).on_press(Message::AiStep),
            button(text("加速")).on_press(Message::AiSpeedUp),
            button(text("减速")).on_press(Message::AiSpeedDown),
            text(speed_label).size(14),
        ]
        .spacing(8)
        .align_y(Alignment::Center)
        .into()
    }

    fn view_sidebar(&self) -> Element<'_, Message> {
        if self.sidebar_collapsed {
            return container(button(text("▶")).on_press(Message::ToggleSidebar))
                .width(Length::Fixed(40.0))
                .height(Length::Fill)
                .align_x(iced::alignment::Horizontal::Center)
                .align_y(iced::alignment::Vertical::Center)
                .into();
        }

        let tabs = row![
            self.tab_button(SidebarTab::Analysis),
            self.tab_button(SidebarTab::Record),
            self.tab_button(SidebarTab::Tips),
        ]
        .spacing(8)
        .align_y(Alignment::Center);

        let content = match self.sidebar_tab {
            SidebarTab::Analysis => self.view_analysis(),
            SidebarTab::Record => self.view_record(),
            SidebarTab::Tips => self.view_tips(),
        };

        let sidebar = column![tabs, content]
            .spacing(12)
            .padding(12)
            .width(Length::Fixed(SIDEBAR_WIDTH));

        container(sidebar)
            .height(Length::Fill)
            .style(|_| iced::widget::container::Style {
                background: Some(Background::Color(Color::from_rgb(0.97, 0.96, 0.94))),
                border: Border {
                    color: Color::from_rgb(0.82, 0.78, 0.7),
                    width: 1.0,
                    radius: 6.0.into(),
                },
                ..Default::default()
            })
            .into()
    }

    fn view_analysis(&self) -> Element<'_, Message> {
        let mut lines = column![
            text("引擎分析(占位)").size(16),
            text(format!("模式：{}", self.mode.label())).size(14),
            text(format!(
                "当前行棋：{}",
                side_label(self.record.board.side_to_move)
            ))
            .size(14),
        ]
        .spacing(6);

        if let Some(mv) = self.suggested_move {
            lines = lines.push(text(format!("推荐走法：{mv}")).size(14));
        } else {
            lines = lines.push(text("推荐走法：暂无").size(14));
        }

        let toggles = row![
            checkbox(self.engine_enabled)
                .label("引擎")
                .on_toggle(Message::ToggleEngine),
            checkbox(self.cloud_enabled)
                .label("云库")
                .on_toggle(Message::ToggleCloud),
        ]
        .spacing(12)
        .align_y(Alignment::Center);
        lines = lines.push(toggles);

        if self.engine_busy {
            lines = lines.push(text("引擎分析中...").size(12));
        }

        if let Some(analysis) = &self.last_analysis {
            let source = match analysis.source {
                EngineSource::Cloud => "云库",
                EngineSource::Engine => "本地引擎",
            };
            lines = lines.push(text(format!("分析来源：{source}")).size(12));
            if let Some(info) = &analysis.info {
                if let Some(depth) = info.depth {
                    lines = lines.push(text(format!("深度：{depth}")).size(12));
                }
                if let Some(score) = format_engine_score(info) {
                    lines = lines.push(text(format!("评分：{score}")).size(12));
                }
                if !info.pv.is_empty() {
                    let pv_text = info
                        .pv
                        .iter()
                        .take(6)
                        .map(|mv| mv.to_iccs())
                        .collect::<Vec<_>>()
                        .join(" ");
                    lines = lines.push(text(format!("PV：{pv_text}")).size(12));
                }
            }
            if let Some(bestmove) = analysis.bestmove {
                lines = lines.push(text(format!("bestmove：{bestmove}")).size(12));
            }
            if let Some(err) = &analysis.error {
                lines = lines.push(text(format!("分析错误：{err}")).size(12));
            }
        }

        if matches!(self.mode, Mode::LinkAnalysis | Mode::LinkBattle) {
            lines = lines.push(text("连线窗口").size(14));
            let pick = pick_list(
                self.link_windows.clone(),
                self.selected_window.clone(),
                Message::WindowSelected,
            )
            .width(Length::Fixed(220.0));
            let refresh = button(text("刷新")).on_press(Message::RequestWindowList);
            let toggle = button(text(if self.link_running { "停止连线" } else { "开始连线" }))
                .on_press(Message::ToggleLink);
            lines = lines.push(row![pick, refresh, toggle].spacing(8));

            let permission_text = match self.input_permission.status {
                InputPermissionStatus::Granted => "输入权限：已授权",
                InputPermissionStatus::Denied => "输入权限：未授权",
                InputPermissionStatus::NotSupported => "输入权限：不支持",
            };
            lines = lines.push(text(permission_text).size(12));
            if let Some(guidance) = self.input_permission.guidance {
                lines = lines.push(text(guidance).size(12));
            }
            if let Some(err) = &self.link_last_error {
                lines = lines.push(text(format!("连线错误：{err}")).size(12));
            }
        }

        if let Some(err) = &self.resources.error {
            lines = lines.push(text(format!("资源检测失败：{err}")).size(12));
        } else if self.resources.has_missing() {
            lines = lines.push(text("资源缺失：").size(12));
            for item in &self.resources.missing {
                lines = lines.push(text(format!("- {item}")).size(12));
            }
        } else {
            lines = lines.push(text("资源自检：通过").size(12));
        }

        container(lines)
            .width(Length::Fill)
            .into()
    }

    fn view_record(&self) -> Element<'_, Message> {
        if self.record.history.is_empty() {
            return container(text("暂无棋谱记录").size(14))
                .width(Length::Fill)
                .into();
        }

        let mut list = column!().spacing(4);
        for (idx, outcome) in self.record.history.iter().enumerate() {
            let label = format!("{:>2}. {}", idx + 1, outcome.mv.to_iccs());
            list = list.push(text(label).size(14));
        }

        scrollable(list).height(Length::Fill).into()
    }

    fn view_tips(&self) -> Element<'_, Message> {
        let mut lines = column![
            text("学习提示").size(16),
            text(format!("状态：{}", if self.learning_enabled { "开启" } else { "关闭" }))
                .size(14),
        ]
        .spacing(6);

        let red_tip = self
            .tips
            .red
            .map(|mv| mv.to_iccs())
            .unwrap_or_else(|| "暂无".to_string());
        let black_tip = self
            .tips
            .black
            .map(|mv| mv.to_iccs())
            .unwrap_or_else(|| "暂无".to_string());

        lines = lines.push(text(format!("红方建议：{red_tip}")).size(14));
        lines = lines.push(text(format!("黑方建议：{black_tip}")).size(14));

        container(lines)
            .width(Length::Fill)
            .into()
    }

    fn tab_button(&self, tab: SidebarTab) -> Element<'_, Message> {
        let is_active = self.sidebar_tab == tab;
        let label = if is_active {
            format!("[{}]", tab.label())
        } else {
            tab.label().to_string()
        };

        let btn = button(text(label)).on_press(Message::SidebarTabSelected(tab));
        btn.into()
    }

    fn handle_cell_press(&mut self, pos: Pos) -> Task<Message> {
        let mut task = Task::none();
        if !self.mode.allows_manual_move(self.record.board.side_to_move) {
            self.status = "当前模式不允许手动落子".into();
            return task;
        }

        if let Some(selected) = self.selection {
            if selected == pos {
                self.selection = None;
                return task;
            }

            let mv = Move::new(selected, pos);
            if matches!(self.mode, Mode::LinkBattle) && self.link_running {
                self.selection = None;
                self.status = "正在注入外部落子...".into();
                self.inject_link_move(mv);
                return task;
            }

            match self.record.board.apply_move_if_legal(mv) {
                Ok(outcome) => {
                    self.record.history.push(outcome);
                    self.redo_stack.clear();
                    self.selection = None;
                    self.suggested_move = None;
                    self.refresh_tips_and_suggestion();
                    self.status = format!("已落子：{mv}");

                    if let Some(ai_side) = self.mode.ai_side() {
                        if self.record.board.side_to_move == ai_side {
                            task = self.request_ai_move();
                        }
                    }
                }
                Err(err) => {
                    self.status = format!("非法走子：{err}");
                }
            }
            return task;
        }

        if let Some(piece) = self.record.board.get(pos) {
            if piece.side == self.record.board.side_to_move {
                self.selection = Some(pos);
                self.status = format!("已选择 {}", pos.to_iccs_square());
            } else {
                self.status = "请选择己方棋子".into();
            }
        } else {
            self.status = "该位置无棋子".into();
        }

        task
    }

    fn reset_game(&mut self) -> Task<Message> {
        self.record = GameRecord::startpos();
        self.redo_stack.clear();
        self.selection = None;
        self.suggested_move = None;
        self.status = "已开新局".into();
        self.last_ai_tick = None;
        self.refresh_tips_and_suggestion();
        self.maybe_trigger_ai_turn()
    }

    fn undo_move(&mut self) {
        let Some(outcome) = self.record.history.pop() else {
            self.status = "无可悔棋".into();
            return;
        };

        self.record.board.side_to_move = outcome.moved.side;
        self.record.board.set(outcome.mv.from, Some(outcome.moved));
        self.record.board.set(outcome.mv.to, outcome.captured);
        self.redo_stack.push(outcome);
        self.selection = None;
        self.suggested_move = None;
        self.status = "已悔棋".into();
        self.refresh_tips_and_suggestion();
    }

    fn redo_move(&mut self) {
        let Some(outcome) = self.redo_stack.pop() else {
            self.status = "无可前进".into();
            return;
        };

        let mv = outcome.mv;
        if let Some(next_outcome) = self.record.board.apply_move_unchecked(mv) {
            self.record.history.push(next_outcome);
            self.status = "已前进".into();
        } else {
            self.status = "前进失败".into();
        }
        self.selection = None;
        self.suggested_move = None;
        self.refresh_tips_and_suggestion();
    }

    fn request_hint(&mut self) -> Task<Message> {
        if self.engine_enabled {
            return self.request_engine_task(EnginePurpose::Hint);
        }
        self.suggested_move = pick_first_legal_move(&self.record.board);
        if let Some(mv) = self.suggested_move {
            self.status = format!("提示走法：{mv}");
        } else {
            self.status = "暂无可用走法".into();
        }
        Task::none()
    }

    fn request_ai_move(&mut self) -> Task<Message> {
        if self.engine_enabled {
            return self.request_engine_task(EnginePurpose::AiMove);
        }
        self.apply_fallback_ai_move();
        Task::none()
    }

    fn apply_fallback_ai_move(&mut self) {
        let mv = match pick_first_legal_move(&self.record.board) {
            Some(mv) => mv,
            None => {
                self.status = "AI 无合法走法".into();
                return;
            }
        };

        match self.record.board.apply_move_if_legal(mv) {
            Ok(outcome) => {
                self.record.history.push(outcome);
                self.redo_stack.clear();
                self.suggested_move = None;
                self.refresh_tips_and_suggestion();
                self.status = format!("AI 落子：{mv}");
            }
            Err(err) => {
                self.status = format!("AI 落子失败：{err}");
            }
        }
    }

    fn request_engine_task(&mut self, purpose: EnginePurpose) -> Task<Message> {
        if self.engine_busy {
            return Task::none();
        }

        let movetime_ms = match purpose {
            EnginePurpose::Hint => 300,
            EnginePurpose::AiMove => 500,
            EnginePurpose::Analysis => 400,
        };

        let Some(profile) = self.make_engine_profile(movetime_ms) else {
            self.status = "引擎资源不可用".into();
            return Task::none();
        };

        let fen = self.record.board.to_fen();
        let cloud_enabled = self.cloud_enabled;
        self.engine_busy = true;
        Task::perform(
            async move { run_engine_analysis(profile, fen, cloud_enabled, purpose).await },
            Message::EngineFinished,
        )
    }

    fn handle_engine_result(&mut self, result: EngineResult) -> Task<Message> {
        self.engine_busy = false;
        let analysis = result.analysis.clone();
        self.last_analysis = Some(analysis.clone());

        if let Some(err) = &analysis.error {
            self.status = format!("引擎错误：{err}");
        }

        match result.purpose {
            EnginePurpose::Hint => {
                self.suggested_move = analysis.bestmove;
                if let Some(mv) = analysis.bestmove {
                    self.status = format!("提示走法：{mv}");
                }
            }
            EnginePurpose::AiMove => {
                if let Some(mv) = analysis.bestmove {
                    match self.record.board.apply_move_if_legal(mv) {
                        Ok(outcome) => {
                            self.record.history.push(outcome);
                            self.redo_stack.clear();
                            self.suggested_move = None;
                            self.refresh_tips_and_suggestion();
                            self.status = format!("AI 落子：{mv}");
                        }
                        Err(err) => {
                            self.status = format!("AI 落子失败：{err}");
                            self.apply_fallback_ai_move();
                        }
                    }
                } else {
                    self.apply_fallback_ai_move();
                }
            }
            EnginePurpose::Analysis => {
                if let Some(mv) = analysis.bestmove {
                    self.suggested_move = Some(mv);
                }
            }
        }

        Task::none()
    }

    fn make_engine_profile(&self, movetime_ms: u64) -> Option<EngineProfile> {
        let paths = self.resource_paths.as_ref()?;
        if !paths.pikafish_bin().is_file() {
            return None;
        }

        let mut profile =
            EngineProfile::new("Pikafish", EngineProtocol::Uci, paths.pikafish_bin());
        if paths.pikafish_nnue().is_file() {
            profile = profile.with_eval_file(paths.pikafish_nnue());
        }
        profile.search = SearchParams::with_movetime(movetime_ms);
        Some(profile)
    }

    fn start_link(&mut self) -> Task<Message> {
        self.input_permission = check_input_permission();
        let Some(selected) = self.selected_window.clone() else {
            self.status = "未选择外部窗口".into();
            return Task::none();
        };
        let Some(paths) = self.resource_paths.as_ref() else {
            self.status = "模型资源不可用，无法连线".into();
            return Task::none();
        };

        let window = match LinkWindow::from_info(selected.info()) {
            Ok(win) => win,
            Err(err) => {
                self.status = format!("打开窗口失败：{err}");
                return Task::none();
            }
        };

        let model_paths = ModelPaths::from_libs_dir(paths.libs_dir());
        let pipeline = match VisionPipeline::new(
            &model_paths,
            PipelineConfig {
                side_to_move: self.record.board.side_to_move,
                ..PipelineConfig::default()
            },
        ) {
            Ok(pipeline) => pipeline,
            Err(err) => {
                self.status = format!("加载模型失败：{err}");
                return Task::none();
            }
        };

        let policy = match self.mode {
            Mode::LinkAnalysis => SyncPolicy::ExternalDriven,
            _ => SyncPolicy::Bidirectional,
        };
        let sync = SyncState::new(
            self.record.board.clone(),
            SyncConfig {
                policy,
                verify_legality: true,
            },
        );

        let runtime = LinkRuntime::new(
            window,
            pipeline,
            sync,
            EnigoInjector::new(),
            LinkRuntimeConfig::default(),
        );

        self.link_runtime = Some(runtime);
        self.link_running = true;
        self.link_last_error = None;
        self.status = "连线已启动".into();
        self.handle_link_tick()
    }

    fn stop_link(&mut self) {
        if self.link_running {
            self.link_running = false;
            self.link_runtime = None;
            self.link_last_error = None;
            self.status = "连线已停止".into();
        }
    }

    fn handle_link_tick(&mut self) -> Task<Message> {
        let Some(runtime) = self.link_runtime.as_mut() else {
            self.link_running = false;
            return Task::none();
        };

        let step = match runtime.capture_step(true) {
            Ok(step) => step,
            Err(err) => {
                self.link_last_error = Some(err.to_string());
                self.status = format!("连线失败：{err}");
                return Task::none();
            }
        };
        self.link_last_error = None;

        if let Some(obs) = &step.output.observation {
            if matches!(self.mode, Mode::LinkAnalysis | Mode::LinkBattle) {
                self.flip_board = obs.flipped;
            }
        }

        if let Some(update) = step.update {
            let board = runtime.sync().local().clone();
            let changed = self.apply_external_update(update, board);
            if changed && self.engine_enabled {
                return self.request_engine_task(EnginePurpose::Analysis);
            }
        }

        Task::none()
    }

    fn inject_link_move(&mut self, mv: Move) {
        let Some(runtime) = self.link_runtime.as_mut() else {
            self.status = "连线未启动".into();
            return;
        };

        match runtime.inject_move(mv) {
            Ok(result) => {
                if let Some(update) = result.update {
                    let board = runtime.sync().local().clone();
                    self.apply_external_update(update, board);
                }
                if result.confirmed == Some(true) {
                    self.status = format!("已注入走法：{mv}");
                } else if result.confirmed == Some(false) {
                    self.status = "注入确认失败，请检查外部棋盘".into();
                } else {
                    self.status = "注入完成，等待外部确认".into();
                }
            }
            Err(err) => {
                self.status = format!("注入失败：{err}");
            }
        }
    }

    fn apply_external_update(&mut self, update: ExternalUpdate, board: Board) -> bool {
        match update {
            ExternalUpdate::NoChange => return false,
            ExternalUpdate::CandidateMove { diff, applied_to_local } => {
                if applied_to_local {
                    if let Some(candidate) = diff.candidate {
                        if let Some(outcome) = self.record.board.apply_move_unchecked(candidate.mv) {
                            self.record.history.push(outcome);
                            self.redo_stack.clear();
                            self.refresh_tips_and_suggestion();
                            return true;
                        }
                    }
                }
            }
            ExternalUpdate::PendingConfirmed { mv } => {
                if let Some(outcome) = self.record.board.apply_move_unchecked(mv) {
                    self.record.history.push(outcome);
                    self.redo_stack.clear();
                    self.refresh_tips_and_suggestion();
                    return true;
                }
            }
            _ => {}
        }

        self.record.board = board;
        self.record.history.clear();
        self.redo_stack.clear();
        self.refresh_tips_and_suggestion();
        true
    }

    fn refresh_tips_and_suggestion(&mut self) {
        if self.learning_enabled {
            self.suggested_move = pick_first_legal_move(&self.record.board);
        } else {
            self.suggested_move = None;
        }
        self.tips.red = pick_move_for_side(&self.record.board, Side::Red);
        self.tips.black = pick_move_for_side(&self.record.board, Side::Black);
    }

    fn switch_mode(&mut self, mode: Mode) -> Task<Message> {
        let previous = self.mode;
        if previous == mode {
            return Task::none();
        }

        self.mode = mode;
        self.selection = None;
        self.suggested_move = None;
        self.last_ai_tick = None;
        self.ai_paused = false;
        if !matches!(self.mode, Mode::LinkAnalysis | Mode::LinkBattle) {
            self.stop_link();
        }
        if !self.mode.allows_flip() {
            self.flip_board = false;
        }

        let task = if !self.preserve_on_mode_switch {
            self.reset_game()
        } else {
            self.refresh_tips_and_suggestion();
            self.maybe_trigger_ai_turn()
        };

        self.status = format!("已切换模式：{}", self.mode.label());
        task
    }

    fn maybe_trigger_ai_turn(&mut self) -> Task<Message> {
        if let Some(ai_side) = self.mode.ai_side() {
            if self.record.board.side_to_move == ai_side {
                return self.request_ai_move();
            }
        }
        Task::none()
    }
}

impl ModeOption {
    fn from_mode(mode: Mode) -> Self {
        match mode {
            Mode::LinkAnalysis => Self::LinkAnalysis,
            Mode::LinkBattle => Self::LinkBattle,
            Mode::HumanVsAi { human_side } => match human_side {
                Side::Red => Self::HumanRed,
                Side::Black => Self::HumanBlack,
            },
            Mode::AiVsAi => Self::AiVsAi,
        }
    }
}

fn mode_options() -> &'static [ModeOption] {
    &[
        ModeOption::LinkAnalysis,
        ModeOption::LinkBattle,
        ModeOption::HumanRed,
        ModeOption::HumanBlack,
        ModeOption::AiVsAi,
    ]
}

fn piece_label(piece: xq_core::Piece) -> &'static str {
    match (piece.side, piece.kind) {
        (Side::Red, xq_core::PieceKind::King) => "帅",
        (Side::Red, xq_core::PieceKind::Advisor) => "仕",
        (Side::Red, xq_core::PieceKind::Elephant) => "相",
        (Side::Red, xq_core::PieceKind::Horse) => "马",
        (Side::Red, xq_core::PieceKind::Rook) => "车",
        (Side::Red, xq_core::PieceKind::Cannon) => "炮",
        (Side::Red, xq_core::PieceKind::Pawn) => "兵",
        (Side::Black, xq_core::PieceKind::King) => "将",
        (Side::Black, xq_core::PieceKind::Advisor) => "士",
        (Side::Black, xq_core::PieceKind::Elephant) => "象",
        (Side::Black, xq_core::PieceKind::Horse) => "马",
        (Side::Black, xq_core::PieceKind::Rook) => "车",
        (Side::Black, xq_core::PieceKind::Cannon) => "砲",
        (Side::Black, xq_core::PieceKind::Pawn) => "卒",
    }
}

fn piece_color(side: Side) -> Color {
    match side {
        Side::Red => Color::from_rgb(0.72, 0.12, 0.12),
        Side::Black => Color::from_rgb(0.1, 0.1, 0.1),
    }
}

fn side_label(side: Side) -> &'static str {
    match side {
        Side::Red => "红方",
        Side::Black => "黑方",
    }
}

fn pick_first_legal_move(board: &Board) -> Option<Move> {
    let moves = generate_legal_moves(board);
    moves.into_iter().next()
}

fn pick_move_for_side(board: &Board, side: Side) -> Option<Move> {
    let mut cloned = board.clone();
    cloned.side_to_move = side;
    pick_first_legal_move(&cloned)
}

fn generate_legal_moves(board: &Board) -> Vec<Move> {
    let mut moves = Vec::new();
    for row in 0..BOARD_ROWS {
        for col in 0..BOARD_COLS {
            let Some(from) = Pos::from_index(row, col) else {
                continue;
            };
            let Some(piece) = board.get(from) else {
                continue;
            };
            if piece.side != board.side_to_move {
                continue;
            }
            for to_row in 0..BOARD_ROWS {
                for to_col in 0..BOARD_COLS {
                    let Some(to) = Pos::from_index(to_row, to_col) else {
                        continue;
                    };
                    let mv = Move::new(from, to);
                    if matches!(check_move_legality(board, mv), xq_core::MoveLegality::Legal) {
                        moves.push(mv);
                    }
                }
            }
        }
    }
    moves
}

async fn run_engine_analysis(
    profile: EngineProfile,
    fen: String,
    cloud_enabled: bool,
    purpose: EnginePurpose,
) -> EngineResult {
    if cloud_enabled {
        match query_chessdb(&fen, Duration::from_millis(800)) {
            Ok(ChessDbResponse::Hit(hit)) => {
                let info = hit.to_engine_info();
                let bestmove = info.pv.first().cloned();
                return EngineResult {
                    purpose,
                    analysis: EngineAnalysis {
                        source: EngineSource::Cloud,
                        bestmove,
                        info: Some(info),
                        error: None,
                    },
                };
            }
            Ok(ChessDbResponse::InvalidBoard(reason)) => {
                return EngineResult {
                    purpose,
                    analysis: EngineAnalysis {
                        source: EngineSource::Cloud,
                        bestmove: None,
                        info: None,
                        error: Some(reason),
                    },
                };
            }
            Ok(ChessDbResponse::NotResult) => {}
            Err(_) => {
                // 云库失败时回退本地引擎。
            }
        }
    }

    let mut analysis = EngineAnalysis {
        source: EngineSource::Engine,
        bestmove: None,
        info: None,
        error: None,
    };

    let mut adapter = match create_engine(&profile) {
        Ok(adapter) => adapter,
        Err(err) => {
            analysis.error = Some(err.to_string());
            return EngineResult { purpose, analysis };
        }
    };

    if let Err(err) = adapter.init().and_then(|_| adapter.apply_profile(&profile)) {
        analysis.error = Some(err.to_string());
        let _ = adapter.quit();
        return EngineResult { purpose, analysis };
    }

    if let Err(err) = adapter.position_fen(&fen).and_then(|_| adapter.go(&profile.search)) {
        analysis.error = Some(err.to_string());
        let _ = adapter.quit();
        return EngineResult { purpose, analysis };
    }

    let deadline = Instant::now() + Duration::from_millis(1200);
    let mut last_info: Option<EngineInfo> = None;
    let mut bestmove: Option<Move> = None;

    while Instant::now() < deadline {
        if let Some(event) = adapter.recv_event_timeout(Duration::from_millis(80)) {
            match event {
                EngineEvent::Info(info) => last_info = Some(info),
                EngineEvent::BestMove(bm) => {
                    bestmove = bm.bestmove.or_else(|| {
                        last_info
                            .as_ref()
                            .and_then(|info| info.pv.first().cloned())
                    });
                    break;
                }
                EngineEvent::RawLine(_) => {}
            }
        }
    }

    let _ = adapter.stop();
    let _ = adapter.quit();

    analysis.bestmove = bestmove;
    analysis.info = last_info;
    EngineResult { purpose, analysis }
}

fn format_engine_score(info: &EngineInfo) -> Option<String> {
    match info.score? {
        EngineScore::Cp(value) => {
            let score = value as f32 / 100.0;
            Some(format!("{score:.2}"))
        }
        EngineScore::Mate(value) => Some(format!("M{value}")),
    }
}

fn board_canvas_size() -> Size {
    let grid_width = (BOARD_COLS as f32 - 1.0) * GRID_SPACING;
    let grid_height = (BOARD_ROWS as f32 - 1.0) * GRID_SPACING;
    Size::new(grid_width + BOARD_PADDING * 2.0, grid_height + BOARD_PADDING * 2.0)
}

#[derive(Debug, Clone)]
struct BoardCanvas {
    board: Board,
    flipped: bool,
    selection: Option<Pos>,
    suggested: Option<Move>,
}

impl canvas::Program<Message> for BoardCanvas {
    type State = ();

    fn update(
        &self,
        _state: &mut Self::State,
        event: &canvas::Event,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Option<canvas::Action<Message>> {
        let canvas::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) = event else {
            return None;
        };
        let Some(position) = cursor.position_in(bounds) else {
            return None;
        };
        let logical = point_to_logical_pos(position, self.flipped)?;
        Some(canvas::Action::publish(Message::CellPressed(logical)))
    }

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &iced::Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        let mut frame = canvas::Frame::new(renderer, bounds.size());
        draw_board_grid(&mut frame);

        if let Some(selected) = self.selection {
            draw_highlight(&mut frame, selected, self.flipped, Color::from_rgb(0.25, 0.55, 0.9));
        }

        if let Some(mv) = self.suggested {
            draw_highlight(&mut frame, mv.from, self.flipped, Color::from_rgb(0.2, 0.7, 0.35));
            draw_highlight(&mut frame, mv.to, self.flipped, Color::from_rgb(0.2, 0.7, 0.35));
        }

        for row in 0..BOARD_ROWS {
            for col in 0..BOARD_COLS {
                let Some(pos) = Pos::from_index(row, col) else {
                    continue;
                };
                let Some(piece) = self.board.get(pos) else {
                    continue;
                };
                draw_piece(&mut frame, pos, piece, self.flipped);
            }
        }

        vec![frame.into_geometry()]
    }

    fn mouse_interaction(
        &self,
        _state: &Self::State,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> mouse::Interaction {
        if cursor.position_in(bounds).is_some() {
            mouse::Interaction::Pointer
        } else {
            mouse::Interaction::default()
        }
    }
}

fn board_origin() -> Point {
    Point::new(BOARD_PADDING, BOARD_PADDING)
}

fn grid_dimensions() -> (f32, f32) {
    (
        (BOARD_COLS as f32 - 1.0) * GRID_SPACING,
        (BOARD_ROWS as f32 - 1.0) * GRID_SPACING,
    )
}

fn point_for_view_pos(pos: Pos) -> Point {
    let (row, col) = pos.to_index();
    let origin = board_origin();
    Point::new(
        origin.x + col as f32 * GRID_SPACING,
        origin.y + row as f32 * GRID_SPACING,
    )
}

fn point_for_pos(pos: Pos, flipped: bool) -> Point {
    let view_pos = map_pos_with_flip(pos, flipped);
    point_for_view_pos(view_pos)
}

fn point_to_logical_pos(point: Point, flipped: bool) -> Option<Pos> {
    let origin = board_origin();
    let (grid_w, grid_h) = grid_dimensions();

    let rel_x = point.x - origin.x;
    let rel_y = point.y - origin.y;
    if rel_x < -GRID_SPACING * 0.3
        || rel_y < -GRID_SPACING * 0.3
        || rel_x > grid_w + GRID_SPACING * 0.3
        || rel_y > grid_h + GRID_SPACING * 0.3
    {
        return None;
    }

    let col_f = rel_x / GRID_SPACING;
    let row_f = rel_y / GRID_SPACING;
    let col = col_f.round();
    let row = row_f.round();

    if col < 0.0 || col > (BOARD_COLS - 1) as f32 || row < 0.0 || row > (BOARD_ROWS - 1) as f32 {
        return None;
    }

    let grid_x = col * GRID_SPACING;
    let grid_y = row * GRID_SPACING;
    let dx = rel_x - grid_x;
    let dy = rel_y - grid_y;
    let dist = (dx * dx + dy * dy).sqrt();
    if dist > HIGHLIGHT_RADIUS {
        return None;
    }

    let row_idx = row as usize;
    let col_idx = col as usize;
    let view_pos = Pos::from_index(row_idx, col_idx)?;
    Some(map_pos_with_flip(view_pos, flipped))
}

fn draw_board_grid(frame: &mut canvas::Frame<iced::Renderer>) {
    let origin = board_origin();
    let (grid_w, grid_h) = grid_dimensions();
    let line_color = Color::from_rgb(0.45, 0.32, 0.2);
    let stroke = canvas::Stroke::default().with_color(line_color).with_width(1.2);

    // Outer border
    frame.stroke_rectangle(origin, Size::new(grid_w, grid_h), stroke);

    // Horizontal lines
    for row in 0..BOARD_ROWS {
        let y = origin.y + row as f32 * GRID_SPACING;
        let from = Point::new(origin.x, y);
        let to = Point::new(origin.x + grid_w, y);
        frame.stroke(&canvas::Path::line(from, to), stroke);
    }

    // Vertical lines (split at river for middle files)
    let river_top = origin.y + 4.0 * GRID_SPACING;
    let river_bottom = origin.y + 5.0 * GRID_SPACING;
    for col in 0..BOARD_COLS {
        let x = origin.x + col as f32 * GRID_SPACING;
        if col == 0 || col == BOARD_COLS - 1 {
            let from = Point::new(x, origin.y);
            let to = Point::new(x, origin.y + grid_h);
            frame.stroke(&canvas::Path::line(from, to), stroke);
        } else {
            let from_top = Point::new(x, origin.y);
            let to_top = Point::new(x, river_top);
            frame.stroke(&canvas::Path::line(from_top, to_top), stroke);
            let from_bottom = Point::new(x, river_bottom);
            let to_bottom = Point::new(x, origin.y + grid_h);
            frame.stroke(&canvas::Path::line(from_bottom, to_bottom), stroke);
        }
    }

    // Palace diagonals
    let palace_color = Color::from_rgb(0.55, 0.4, 0.25);
    let palace_stroke = canvas::Stroke::default().with_color(palace_color).with_width(1.2);
    let top_left = Point::new(origin.x + 3.0 * GRID_SPACING, origin.y);
    let top_right = Point::new(origin.x + 5.0 * GRID_SPACING, origin.y + 2.0 * GRID_SPACING);
    let top_left_2 = Point::new(origin.x + 5.0 * GRID_SPACING, origin.y);
    let top_right_2 = Point::new(origin.x + 3.0 * GRID_SPACING, origin.y + 2.0 * GRID_SPACING);
    frame.stroke(&canvas::Path::line(top_left, top_right), palace_stroke);
    frame.stroke(&canvas::Path::line(top_left_2, top_right_2), palace_stroke);

    let bottom_origin_y = origin.y + 7.0 * GRID_SPACING;
    let bottom_left = Point::new(origin.x + 3.0 * GRID_SPACING, bottom_origin_y);
    let bottom_right = Point::new(origin.x + 5.0 * GRID_SPACING, bottom_origin_y + 2.0 * GRID_SPACING);
    let bottom_left_2 = Point::new(origin.x + 5.0 * GRID_SPACING, bottom_origin_y);
    let bottom_right_2 = Point::new(origin.x + 3.0 * GRID_SPACING, bottom_origin_y + 2.0 * GRID_SPACING);
    frame.stroke(&canvas::Path::line(bottom_left, bottom_right), palace_stroke);
    frame.stroke(&canvas::Path::line(bottom_left_2, bottom_right_2), palace_stroke);
}

fn draw_piece(frame: &mut canvas::Frame<iced::Renderer>, pos: Pos, piece: xq_core::Piece, flipped: bool) {
    let center = point_for_pos(pos, flipped);
    let fill = if piece.side == Side::Red {
        Color::from_rgb(0.98, 0.96, 0.92)
    } else {
        Color::from_rgb(0.96, 0.96, 0.96)
    };
    let border = piece_color(piece.side);
    let circle = canvas::Path::circle(center, PIECE_RADIUS);
    frame.fill(&circle, fill);
    frame.stroke(&circle, canvas::Stroke::default().with_color(border).with_width(2.0));

    let text = canvas::Text {
        content: piece_label(piece).to_string(),
        position: center,
        color: border,
        size: 20.0.into(),
        max_width: PIECE_RADIUS * 2.0,
        line_height: iced::widget::text::LineHeight::Relative(1.0),
        font: iced::Font::default(),
        align_x: iced::widget::text::Alignment::Center,
        align_y: iced::alignment::Vertical::Center,
        shaping: iced::widget::text::Shaping::Advanced,
    };
    frame.fill_text(text);
}

fn draw_highlight(frame: &mut canvas::Frame<iced::Renderer>, pos: Pos, flipped: bool, color: Color) {
    let center = point_for_pos(pos, flipped);
    let ring = canvas::Path::circle(center, HIGHLIGHT_RADIUS);
    frame.stroke(&ring, canvas::Stroke::default().with_color(color).with_width(2.2));
}
