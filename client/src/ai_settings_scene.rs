use crate::BACKGROUND;
use crate::{SceneName, SceneResult};
use game::ai::{Difficulty, Personality};
use pixels_graphics_lib::prelude::SceneUpdateResult::Nothing;
use pixels_graphics_lib::prelude::*;
use pixels_graphics_lib::ui::button::Button;
use pixels_graphics_lib::ui::helpers::ToggleButtonGroup;
use pixels_graphics_lib::ui::styles::UiStyle;
use pixels_graphics_lib::ui::toggle_button::ToggleButton;
use pixels_graphics_lib::ui::PixelView;

const DIFFICULTIES: [(Difficulty, &str); 4] = [
    (Difficulty::Easy, "Easy"),
    (Difficulty::Medium, "Medium"),
    (Difficulty::Hard, "Hard"),
    (Difficulty::VeryHard, "Very Hard"),
];

const PERSONALITIES: [(Personality, &str); 3] = [
    (Personality::Defensive, "Defensive"),
    (Personality::Balanced, "Balanced"),
    (Personality::Aggressive, "Aggressive"),
];

const DEFAULT_DIFFICULTY: Difficulty = Difficulty::Hard;
const DEFAULT_PERSONALITY: Personality = Personality::Balanced;

pub struct AiSettingsScene {
    difficulty_buttons: ToggleButtonGroup<Difficulty>,
    personality_buttons: ToggleButtonGroup<Personality>,
    back_button: Button,
    start_button: Button,
    result: SceneUpdateResult<SceneResult, SceneName>,
}

impl AiSettingsScene {
    pub fn new(style: &UiStyle) -> Box<AiSettingsScene> {
        let mut difficulty_buttons = ToggleButtonGroup::new(
            DIFFICULTIES
                .iter()
                .enumerate()
                .map(|(i, (difficulty, label))| {
                    (
                        *difficulty,
                        ToggleButton::new(
                            coord!(16, 32 + i * 26),
                            label,
                            Some(110),
                            &style.toggle_button,
                        ),
                    )
                })
                .collect(),
        );
        difficulty_buttons.set_selected(DEFAULT_DIFFICULTY);

        let mut personality_buttons = ToggleButtonGroup::new(
            PERSONALITIES
                .iter()
                .enumerate()
                .map(|(i, (personality, label))| {
                    (
                        *personality,
                        ToggleButton::new(
                            coord!(220, 32 + i * 26),
                            label,
                            Some(110),
                            &style.toggle_button,
                        ),
                    )
                })
                .collect(),
        );
        personality_buttons.set_selected(DEFAULT_PERSONALITY);

        Box::new(Self {
            difficulty_buttons,
            personality_buttons,
            back_button: Button::new(coord!(16, 270), "Back", Some(100), &style.button),
            start_button: Button::new(coord!(160, 270), "Start", Some(100), &style.button),
            result: Nothing,
        })
    }
}

impl Scene<SceneResult, SceneName> for AiSettingsScene {
    fn render(&self, graphics: &mut Graphics, mouse: &MouseData, _: &FxHashSet<KeyCode>) {
        graphics.clear(BACKGROUND);
        graphics.draw_text(
            "Difficulty",
            TextPos::px(coord!(16, 16)),
            (WHITE, PixelFont::Standard6x7),
        );
        graphics.draw_text(
            "Personality",
            TextPos::px(coord!(220, 16)),
            (WHITE, PixelFont::Standard6x7),
        );
        self.difficulty_buttons.render(graphics, mouse);
        self.personality_buttons.render(graphics, mouse);
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
        self.difficulty_buttons.on_mouse_click(down_at, mouse.xy);
        self.personality_buttons.on_mouse_click(down_at, mouse.xy);

        if self.back_button.on_mouse_click(down_at, mouse.xy) {
            self.result = SceneUpdateResult::Pop(None);
        }
        if self.start_button.on_mouse_click(down_at, mouse.xy) {
            self.result = SceneUpdateResult::Push(
                false,
                SceneName::AiGame(
                    *self.difficulty_buttons.get_selected(),
                    *self.personality_buttons.get_selected(),
                ),
            );
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
