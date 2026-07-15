use crate::BACKGROUND;
use crate::net::send;
use crate::{SceneName, SceneResult};
use eb_crownfall_engine::ai::{CrownfallDifficulty, CrownfallPersonality};
use eb_crownfall_engine::{CrownfallBoardVariant, CrownfallRules, CrownfallRuleset};
use networking::packet::Packet;
use pixels_graphics_lib::prelude::SceneUpdateResult::Nothing;
use pixels_graphics_lib::prelude::*;
use pixels_graphics_lib::ui::PixelView;
use pixels_graphics_lib::ui::button::Button;
use pixels_graphics_lib::ui::helpers::ToggleButtonGroup;
use pixels_graphics_lib::ui::styles::UiStyle;
use pixels_graphics_lib::ui::toggle_button::ToggleButton;

/// What to do with the chosen `CrownfallRules` once "Start" is pressed -
/// either forwarded on to a local AI game, or sent to the server to create
/// a pending networked game.
#[derive(Clone, Debug, PartialEq)]
pub enum RulesPurpose {
    CreateGame,
    AiGame(CrownfallDifficulty, CrownfallPersonality),
}

/// Flattened, pickable stand-in for `CrownfallRuleset` - the `Custom` variant's
/// three bools are independent in the engine, but only these named
/// combinations have dedicated `CrownfallRules` constructors, so that's what's
/// exposed here rather than a combinatorial checkbox grid.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
enum RulesetOption {
    Standard,
    Archers,
    MandatoryCapture,
    AllCapturesProcessed,
    DiagonalKnights,
}

impl RulesetOption {
    fn to_ruleset(self) -> CrownfallRuleset {
        match self {
            RulesetOption::Standard => CrownfallRuleset::Custom {
                mandatory_capture: false,
                all_captures_processed: false,
                knights_move_diagonally: false,
            },
            RulesetOption::Archers => CrownfallRuleset::Archers,
            RulesetOption::MandatoryCapture => CrownfallRuleset::Custom {
                mandatory_capture: true,
                all_captures_processed: false,
                knights_move_diagonally: false,
            },
            RulesetOption::AllCapturesProcessed => CrownfallRuleset::Custom {
                mandatory_capture: false,
                all_captures_processed: true,
                knights_move_diagonally: false,
            },
            RulesetOption::DiagonalKnights => CrownfallRuleset::Custom {
                mandatory_capture: false,
                all_captures_processed: false,
                knights_move_diagonally: true,
            },
        }
    }
}

const BOARDS: [(CrownfallBoardVariant, &str); 3] = [
    (CrownfallBoardVariant::Mini, "Mini (5x5)"),
    (CrownfallBoardVariant::Normal, "Normal (7x7)"),
    (CrownfallBoardVariant::Grand, "Grand (9x9)"),
];

const RULESETS: [(RulesetOption, &str); 5] = [
    (RulesetOption::Standard, "Standard"),
    (RulesetOption::Archers, "Archers"),
    (RulesetOption::MandatoryCapture, "Mandatory Capture"),
    (RulesetOption::AllCapturesProcessed, "All Captures Processed"),
    (RulesetOption::DiagonalKnights, "Diagonal Knights"),
];

const DEFAULT_BOARD: CrownfallBoardVariant = CrownfallBoardVariant::Normal;
const DEFAULT_RULESET: RulesetOption = RulesetOption::Standard;

pub struct RulesSettingsScene {
    purpose: RulesPurpose,
    board_buttons: ToggleButtonGroup<CrownfallBoardVariant>,
    ruleset_buttons: ToggleButtonGroup<RulesetOption>,
    back_button: Button,
    start_button: Button,
    result: SceneUpdateResult<SceneResult, SceneName>,
}

impl RulesSettingsScene {
    pub fn new(style: &UiStyle, purpose: RulesPurpose) -> Box<RulesSettingsScene> {
        let mut board_buttons = ToggleButtonGroup::new(
            BOARDS
                .iter()
                .enumerate()
                .map(|(i, (variant, label))| {
                    (
                        *variant,
                        ToggleButton::new(
                            coord!(16, 32 + i * 26),
                            label,
                            Some(140),
                            &style.toggle_button,
                        ),
                    )
                })
                .collect(),
        );
        board_buttons.set_selected(DEFAULT_BOARD);

        let mut ruleset_buttons = ToggleButtonGroup::new(
            RULESETS
                .iter()
                .enumerate()
                .map(|(i, (ruleset, label))| {
                    (
                        *ruleset,
                        ToggleButton::new(
                            coord!(220, 32 + i * 26),
                            label,
                            Some(220),
                            &style.toggle_button,
                        ),
                    )
                })
                .collect(),
        );
        ruleset_buttons.set_selected(DEFAULT_RULESET);

        Box::new(Self {
            purpose,
            board_buttons,
            ruleset_buttons,
            back_button: Button::new(coord!(16, 270), "Back", Some(100), &style.button),
            start_button: Button::new(coord!(160, 270), "Start", Some(100), &style.button),
            result: Nothing,
        })
    }

    fn selected_rules(&self) -> CrownfallRules {
        CrownfallRules {
            board: *self.board_buttons.get_selected(),
            ruleset: self.ruleset_buttons.get_selected().to_ruleset(),
        }
    }
}

impl Scene<SceneResult, SceneName> for RulesSettingsScene {
    fn render(&self, graphics: &mut Graphics, mouse: &MouseData, _: &FxHashSet<KeyCode>) {
        graphics.clear(BACKGROUND);
        graphics.draw_text(
            "Board size",
            TextPos::px(coord!(16, 16)),
            (WHITE, PixelFont::Standard6x7),
        );
        graphics.draw_text(
            "Rules",
            TextPos::px(coord!(220, 16)),
            (WHITE, PixelFont::Standard6x7),
        );
        self.board_buttons.render(graphics, mouse);
        self.ruleset_buttons.render(graphics, mouse);
        self.back_button.render(graphics, mouse);
        self.start_button.render(graphics, mouse);
    }

    fn on_mouse_click(
        &mut self,
        down_at: Coord,
        mouse: &MouseData,
        mouse_button: MouseButton,
        _: &FxHashSet<KeyCode>,
    ) {
        if mouse_button != MouseButton::Left {
            return;
        }
        self.board_buttons.on_mouse_click(down_at, mouse.xy);
        self.ruleset_buttons.on_mouse_click(down_at, mouse.xy);

        if self.back_button.on_mouse_click(down_at, mouse.xy) {
            self.result = SceneUpdateResult::Pop(None);
        }
        if self.start_button.on_mouse_click(down_at, mouse.xy) {
            let rules = self.selected_rules();
            self.result = match &self.purpose {
                RulesPurpose::AiGame(difficulty, personality) => SceneUpdateResult::Push(
                    false,
                    SceneName::AiGame(*difficulty, *personality, rules),
                ),
                RulesPurpose::CreateGame => match send(Packet::CreateGameRequest(rules)) {
                    Ok(_) => SceneUpdateResult::Pop(Some(SceneResult::GameCreationRequested)),
                    Err(e) => {
                        SceneUpdateResult::Pop(Some(SceneResult::GameCreationFailed(format!(
                            "{e:?}"
                        ))))
                    }
                },
            };
        }
    }

    fn update(
        &mut self,
        timing: &Timing,
        _: &MouseData,
        _: &FxHashSet<KeyCode>,
        _: &Window,
    ) -> SceneUpdateResult<SceneResult, SceneName> {
        self.back_button.update(timing);
        self.start_button.update(timing);
        self.result.clone()
    }

    fn resuming(&mut self, _: Option<SceneResult>) {
        self.result = Nothing;
    }
}
