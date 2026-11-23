//! Телефонный интерфейс (Android): холст на весь экран, одна строка сверху и листы
//! снизу, которые выезжают по касанию.
//!
//! Устроен по образцу планшетных художников (ProCraft и подобных): поверх холста —
//! лишь самое нужное, всё остальное уходит в листы. Ползунки и сетки свои —
//! десктопные строки рисуются поверх всего и залезают друг в друга в узком листе;
//! под ними те же поля (`brushes[brush].brush.params`, `doc`, `color_views`), так
//! что настройки у двух интерфейсов общие.
//!
//! Касания: один палец — рисование (или сдвиг вида, если включена «Рука»),
//! два пальца — сдвиг, масштаб и поворот вида, три — отмена и возврат.

use crate::i18n::tr;
use crate::{sai_ui, PaintApp, Tool};
use eframe::egui::{self, Color32, Pos2, Rect, Vec2};
use std::time::{Duration, Instant};

/// Что показать в листе.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub(crate) enum Sheet {
    /// Ничего: холст во весь экран.
    #[default]
    None,
    /// Сетка инструментов, размеры и плотность кисти.
    Tools,
    /// Список слоёв и операции с ними.
    Layers,
    /// Цвет: круг, ползунки, образцы.
    Color,
    /// Разделы меню «Файл», «Правка», «Холст»…
    Menu,
}

impl Sheet {
    fn title(self) -> &'static str {
        match self {
            Sheet::None => "",
            Sheet::Tools => tr("Инструменты"),
            Sheet::Layers => tr("Слои"),
            Sheet::Color => tr("Цвет"),
            Sheet::Menu => tr("Меню"),
        }
    }

    /// Доля высоты экрана, которую занимает лист.
    fn height(self) -> f32 {
        match self {
            Sheet::None => 0.0,
            Sheet::Menu => 0.55,
            _ => 0.46,
        }
    }
}

/// Состояние телефонного интерфейса между кадрами.
#[derive(Default)]
pub(crate) struct Mobile {
    pub(crate) sheet: Sheet,
    /// Раздел меню, раскрытый в листе.
    pub(crate) menu: Option<usize>,
}

/// Активные касания пальцами в этом кадре.
pub(crate) fn touches(ctx: &egui::Context) -> Vec<(u64, egui::TouchPhase, Pos2)> {
    ctx.input(|i| {
        i.events
            .iter()
            .filter_map(|e| match e {
                egui::Event::Touch { id, phase, pos, .. } => Some((id.0, *phase, *pos)),
                _ => None,
            })
            .collect()
    })
}

/// Сдвиг одного пальца за кадр: на Android палец — это и указатель,
/// поэтому `pointer.delta()` и есть его движение.
pub(crate) fn touch_pan_delta(ui: &egui::Ui) -> Option<Vec2> {
    let p = ui.input(|i| i.pointer.delta());
    (p != Vec2::ZERO).then_some(p)
}

/// Разбор касаний двумя пальцами: сдвиг середины, масштаб и поворот отрезка.
#[derive(Default)]
pub(crate) struct Gestures {
    active: Vec<(u64, Pos2)>,
    /// Прошлые середина, радиус и угол отрезка между первыми двумя пальцами.
    prev: Option<(Pos2, f32, f32)>,
    was: usize,
    /// Три пальца: один — отменить (справа), два — вернуть (слева). Один раз.
    triple_fired: bool,
}

/// Итог разбора касаний.
#[derive(Default)]
pub(crate) struct GestureOut {
    /// Пальцев два и больше: холст они не рисуют, а двигают и приближают вид.
    pub(crate) multi: bool,
    pub(crate) pan: Vec2,
    pub(crate) zoom: f32,
    pub(crate) rotate: f32,
    /// Три пальца: отменить (`true`) или вернуть (`false`).
    pub(crate) undo: Option<bool>,
    /// Второй палец лёг в этом кадре.
    pub(crate) second_landed: bool,
}

impl Gestures {
    pub(crate) fn update(&mut self, ctx: &egui::Context) -> GestureOut {
        let mut out = GestureOut { zoom: 1.0, ..Default::default() };
        for (id, phase, pos) in touches(ctx) {
            match phase {
                egui::TouchPhase::Start => {
                    self.active.retain(|t| t.0 != id);
                    self.active.push((id, pos));
                }
                egui::TouchPhase::Move => {
                    if let Some(t) = self.active.iter_mut().find(|t| t.0 == id) {
                        t.1 = pos;
                    } else {
                        self.active.push((id, pos));
                    }
                }
                egui::TouchPhase::End | egui::TouchPhase::Cancel => self.active.retain(|t| t.0 != id),
            }
        }
        let n = self.active.len();
        if n >= 2 && self.was < 2 {
            out.second_landed = true;
        }
        if n >= 3 && !self.triple_fired {
            self.triple_fired = true;
            let x = self.active[..3].iter().map(|t| t.1.x).sum::<f32>() / 3.0;
            out.undo = Some(x >= ctx.screen_rect().center().x);
        }
        if n < 3 {
            self.triple_fired = false;
        }
        self.was = n;
        if n < 2 {
            self.prev = None;
            return out;
        }
        out.multi = true;
        let (a, b) = (self.active[0].1, self.active[1].1);
        let c = a + (b - a) / 2.0;
        let r = ((b - a).length() / 2.0).max(1.0);
        let angle = (b - a).angle();
        if let Some((pc, pr, pa)) = self.prev {
            out.pan = c - pc;
            out.zoom = (r / pr).clamp(0.5, 2.0);
            let mut d = angle - pa;
            while d > std::f32::consts::PI {
                d -= std::f32::consts::TAU;
            }
            while d < -std::f32::consts::PI {
                d += std::f32::consts::TAU;
            }
            // меньше трёх градусов — дрожание рук, а не поворот
            out.rotate = if d.abs() > 0.05 { d } else { 0.0 };
        }
        self.prev = Some((c, r, angle));
        out
    }

    /// Середина касаний — вокруг неё масштаб и поворот.
    pub(crate) fn center(&self) -> Option<Pos2> {
        let (a, b) = (self.active.first()?.1, self.active.get(1)?.1);
        Some(a + (b - a) / 2.0)
    }
}

impl PaintApp {
    /// Отступы выреза и системных полос: раз полсекунды, холст идёт под ними.
    #[cfg(target_os = "android")]
    pub(crate) fn android_update_insets(&mut self, _ctx: &egui::Context) {
        if self.android_insets_at.is_none_or(|t| t.elapsed() > Duration::from_millis(500)) {
            self.android_insets_at = Some(Instant::now());
            self.android_insets = crate::android_shell::safe_insets();
        }
    }
    #[cfg(not(target_os = "android"))]
    pub(crate) fn android_update_insets(&mut self, _ctx: &egui::Context) {}

    /// Телефон: холст на весь экран, сверху строка, снизу лист.
    pub(crate) fn android_phone_ui(&mut self, ctx: &egui::Context, _docked: bool) {
        self.android_quick_panel(ctx);
        egui::CentralPanel::default().frame(egui::Frame::none()).show(ctx, |ui| self.canvas(ui));
        if self.mobile.sheet != Sheet::None {
            self.android_sheet(ctx);
        }
    }

    /// Верхняя строка: отмена, повтор, «Рука», цвет, размер кисти и кнопки листов.
    fn android_quick_panel(&mut self, ctx: &egui::Context) {
        // верхнюю строку сдвигаем из-под выреза камеры (там раньше была чёрная полоса)
        let top = self.android_insets[0] / self.ppp.max(0.01);
        let frame = egui::Frame::side_top_panel(&ctx.style()).inner_margin(egui::Margin::symmetric(4.0, 2.0 + top));
        egui::TopBottomPanel::top("android_quick").frame(frame).show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 3.0;
                let b = Vec2::splat(26.0);
                if sai_ui::icon_button(ui, b, false, self.doc.can_undo(), tr("Отменить"), |p, r, c| sai_ui::glyph(p, r, c, "undo")).clicked() {
                    self.undo();
                }
                if sai_ui::icon_button(ui, b, false, self.doc.can_redo(), tr("Повторить"), |p, r, c| sai_ui::glyph(p, r, c, "redo")).clicked() {
                    self.redo();
                }
                // «Рука»: одним пальцем двигать холст, не рисуя (снимается после отрыва)
                let mut pan = self.android_pan_toggle;
                if sai_ui::icon_button(ui, b, pan, true, tr("Передвигать холст одним пальцем"), |p, r, c| sai_ui::glyph(p, r, c, "hand")).clicked() {
                    pan = !pan;
                    self.android_pan_toggle = pan;
                }
                // текущий цвет: касание — лист цвета, маленькая стрелка — обмен
                let (rect, resp) = ui.allocate_exact_size(Vec2::splat(26.0), egui::Sense::click());
                let p = ui.painter();
                if self.transparent {
                    sai_ui::checker(p, rect.shrink(3.0), 5.0);
                } else {
                    p.rect_filled(rect.shrink(3.0), 2.0, self.color());
                }
                p.rect_stroke(rect.shrink(3.0), 2.0, egui::Stroke::new(1.0_f32, sai_ui::border()));
                let swap = Rect::from_center_size(rect.right_top() + Vec2::new(-5.0, 5.0), Vec2::splat(11.0));
                sai_ui::glyph(p, swap, sai_ui::ink(), "cw");
                if resp.clicked() {
                    if resp.interact_pointer_pos().is_some_and(|q| swap.expand(4.0).contains(q)) {
                        self.swap_colors();
                    } else {
                        self.mobile.sheet = if self.mobile.sheet == Sheet::Color { Sheet::None } else { Sheet::Color };
                    }
                }
                if self.tool == Tool::Brush {
                    // размер и плотность выбранной кисти — всегда под рукой
                    let width = (ui.available_width() - 120.0).max(120.0);
                    let brush = &mut self.brushes[self.brush].brush;
                    let mut size = brush.params.size.sqrt();
                    let text = format!("{:.0}", brush.params.size);
                    if sai_ui::bar(ui, &mut size, 0.6f32.sqrt(), 5000f32.sqrt(), text, width * 0.55, false).changed() {
                        brush.params.size = (size * size).clamp(0.6, 5000.0);
                    }
                    let mut d = brush.params.density * 100.0;
                    let text = format!("{d:.0}%");
                    if sai_ui::bar(ui, &mut d, 0.0, 100.0, text, width * 0.45, false).changed() {
                        brush.params.density = d / 100.0;
                    }
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    self.android_sheet_buttons(ui);
                });
            });
        });
    }

    /// Кнопки листов вверху справа: инструменты, слои, меню.
    fn android_sheet_buttons(&mut self, ui: &mut egui::Ui) {
        let b = Vec2::splat(26.0);
        let mut target = None;
        // порядок справа налево — как лежат кнопки на экране
        for (sheet, tip, g) in [
            (Sheet::Menu, tr("Меню"), "menu"),
            (Sheet::Layers, tr("Слои"), "layers"),
            (Sheet::Tools, tr("Инструменты"), "tools"),
        ] {
            let on = self.mobile.sheet == sheet;
            if sai_ui::icon_button(ui, b, on, true, tip, |p, r, c| sai_ui::glyph(p, r, c, g)).clicked() {
                target = Some(if on { Sheet::None } else { sheet });
            }
        }
        if let Some(s) = target {
            self.mobile.sheet = s;
        }
    }

    /// Лист снизу: заголовок, «спрятать» и содержимое.
    fn android_sheet(&mut self, ctx: &egui::Context) {
        let sheet = self.mobile.sheet;
        let h = (ctx.screen_rect().height() * sheet.height()).max(200.0);
        // лист не заходит под панель навигации и горизонтальный вырез
        let bottom = self.android_insets[1] / self.ppp.max(0.01);
        let frame = egui::Frame::side_top_panel(&ctx.style())
            .inner_margin(egui::Margin { left: 6.0, right: 6.0, top: 4.0, bottom: 4.0 + bottom })
            .stroke(egui::Stroke::new(1.0_f32, sai_ui::border()));
        egui::TopBottomPanel::bottom("android_sheet").exact_height(h).frame(frame).show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.strong(sheet.title());
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("▾").on_hover_text(tr("Спрятать")).clicked() {
                        self.mobile.sheet = Sheet::None;
                    }
                });
            });
            ui.separator();
            // полосы прокрутки нет: лист и так во всю ширину, а её кнопки под ней мешали
            ui.spacing_mut().scroll = egui::style::ScrollStyle::solid();
            egui::ScrollArea::vertical()
                .id_salt(("android_sheet_scroll", sheet as u8))
                .auto_shrink([false, false])
                .show(ui, |ui| match sheet {
                    Sheet::Tools => self.phone_tools(ui),
                    Sheet::Layers => self.phone_layers(ui),
                    Sheet::Color => self.phone_color(ui),
                    Sheet::Menu => self.phone_menu(ui),
                    Sheet::None => {}
                });
        });
    }

    /// Инструменты: общие (выделение, перемещение, вид), сетка кистей и плотность.
    fn phone_tools(&mut self, ui: &mut egui::Ui) {
        let b = Vec2::splat(34.0);
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = Vec2::new(4.0, 4.0);
            for (tip, tool, g) in [
                (tr("Выделение (M)"), Tool::Marquee, "select"),
                (tr("Лассо (L)"), Tool::Lasso, "lasso"),
                (tr("Волшебная палочка (W)"), Tool::Wand, "wand"),
                (tr("Заливка"), Tool::Fill, "bucket"),
                (tr("Перемещение"), Tool::Move, "move"),
                (tr("Масштаб"), Tool::Zoom, "zoom"),
                (tr("Поворот вида"), Tool::RotateView, "rotate"),
                (tr("Рука"), Tool::Hand, "hand"),
                (tr("Пипетка"), Tool::Picker, "picker"),
                (tr("Текст"), Tool::Text, "text"),
            ] {
                if sai_ui::icon_button(ui, b, self.tool == tool, true, tip, |p, r, c| sai_ui::glyph(p, r, c, g)).clicked() {
                    self.tool = tool;
                    // выбрали инструмент — лист дальше не нужен
                    if tool != Tool::Brush {
                        self.mobile.sheet = Sheet::None;
                    }
                }
            }
        });
        ui.add_space(4.0);
        ui.separator();
        // группы кистей — вкладками
        self.tool_group_tabs(ui);
        ui.add_space(2.0);
        // сетка кистей открытой группы: 3 в ряд, ячейка крупная под палец
        let shown: Vec<usize> = (0..self.brushes.len()).filter(|&i| self.brushes[i].group == self.tool_group).collect();
        let w = ((ui.available_width() - 8.0) / 3.0).max(60.0);
        let mut pick = None;
        for row in shown.chunks(3) {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 4.0;
                for &i in row {
                    let tool = &self.brushes[i];
                    let (title, icon, enabled) = (tool.title(), tool.icon, tool.enabled);
                    let selected = self.tool == Tool::Brush && self.brush == i;
                    let r = sai_ui::icon_button(ui, Vec2::new(w, 50.0), selected, enabled, &title, |p, r, c| {
                        sai_ui::fit_text(p, r.left_top() + Vec2::new(4.0, 4.0), egui::Align2::LEFT_TOP, &title, r.width() - 26.0, 12.0, c);
                        sai_ui::glyph(p, Rect::from_min_size(r.right_bottom() - Vec2::splat(22.0), Vec2::splat(22.0)), c, icon);
                    });
                    if r.clicked() {
                        pick = Some(i);
                    }
                }
            });
        }
        if let Some(i) = pick {
            self.tool = Tool::Brush;
            self.brush = i;
        }
        ui.add_space(4.0);
        // плотность кисти — сглаживание SAI — прямая линия — линейка
        ui.horizontal(|ui| {
            self.smoothing_picker(ui);
            if sai_ui::icon_button(ui, Vec2::splat(24.0), self.straight, true, tr("Прямая линия"), |p, r, c| sai_ui::glyph(p, r, c, "line")).clicked() {
                self.straight = !self.straight;
            }
            if sai_ui::icon_button(ui, Vec2::splat(24.0), self.ruler.ignore, self.ruler.visible, tr("Не замечать линейку"), |p, r, c| sai_ui::glyph(p, r, c, "ruler")).clicked() {
                self.ruler.ignore = !self.ruler.ignore;
            }
        });
    }

    /// Сглаживание: список уровней по касанию (как «Сглаживание» в быстрой панели).
    fn smoothing_picker(&mut self, ui: &mut egui::Ui) {
        egui::ComboBox::from_id_salt("phone_stab").selected_text(self.stabilizer.label()).width(96.0).height(360.0).show_ui(ui, |ui| {
            for v in paint_core::stabilizer::Level::all() {
                ui.selectable_value(&mut self.stabilizer, v, v.label());
            }
        });
    }

    /// Слои: список и работа с ним.
    fn phone_layers(&mut self, ui: &mut egui::Ui) {
        let b = Vec2::splat(30.0);
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = Vec2::new(4.0, 4.0);
            for (tip, g, act) in [
                (tr("Новый слой"), "new_layer", Some(None)),
                (tr("Новая папка"), "new_folder", Some(Some(true))),
                (tr("Слой выше"), "up", None),
                (tr("Слой ниже"), "lower", None),
                (tr("Слить с нижним"), "merge", None),
                (tr("Очистить слой"), "clear", None),
                (tr("Удалить слой"), "delete", None),
            ] {
                if sai_ui::icon_button(ui, b, false, true, tip, |p, r, c| sai_ui::glyph(p, r, c, g)).clicked() {
                    match act {
                        Some(folder) => self.add_layer(folder.unwrap_or(false)),
                        None => {
                            let a = match g {
                                "up" => crate::LayerAction::Up,
                                "lower" => crate::LayerAction::Down,
                                "merge" => crate::LayerAction::Merge,
                                "clear" => crate::LayerAction::Clear,
                                _ => crate::LayerAction::Delete,
                            };
                            self.layer_action(a);
                        }
                    }
                }
            }
        });
        // непрозрачность и режим выбранного слоя
        let path = self.selected.clone();
        if let Some(layer) = self.doc.layer_mut(&path) {
            ui.horizontal(|ui| {
                ui.label(tr("Непроз."));
                let mut v = layer.opacity * 100.0;
                let text = format!("{v:.0}%");
                if sai_ui::bar(ui, &mut v, 0.0, 100.0, text, ui.available_width() - 8.0, true).changed() {
                    layer.opacity = v.round() / 100.0;
                    self.dirty = true;
                }
            });
            let current = crate::MODES.iter().find(|(m, _)| *m == layer.mode).map_or(tr("Обычный"), |(_, n)| tr(n));
            let mut mode = layer.mode;
            egui::ComboBox::from_id_salt("phone_mode").selected_text(current).width(ui.available_width() - 8.0).height(400.0).show_ui(ui, |ui| {
                for (m, n) in crate::MODES {
                    ui.selectable_value(&mut mode, m, tr(n));
                }
            });
            if mode != layer.mode {
                layer.mode = mode;
                self.dirty = true;
                self.refresh_all();
            }
        }
        ui.separator();
        // список слоёв — те же строки, что на компьютере
        let mut changed = false;
        egui::Frame::none().fill(sai_ui::field()).stroke(egui::Stroke::new(1.0_f32, sai_ui::border())).inner_margin(2.0).show(ui, |ui| {
            egui::ScrollArea::vertical().id_salt("phone_layer_list").auto_shrink([false, true]).max_height(ui.available_height() - 8.0).show(ui, |ui| {
                ui.spacing_mut().item_spacing.y = 1.0;
                let mut p = Vec::new();
                self.layer_rects.clear();
                self.layer_rows(ui, &mut p, 0, &mut changed);
            });
        });
        if changed {
            self.dirty = true;
            self.refresh_all();
        }
    }

    /// Цвет: круг, оттенок и насыщенность, образцы; ползунки RGB — в свёрнутом виде.
    fn phone_color(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            let size = (ui.available_width().min(190.0)).max(120.0);
            sai_ui::color_wheel(ui, &mut self.hsv, size);
            ui.vertical(|ui| {
                let mut rgb = [self.color().r(), self.color().g(), self.color().b()];
                for (i, name) in ["R", "G", "B"].into_iter().enumerate() {
                    ui.horizontal(|ui| {
                        ui.label(name);
                        let mut v = rgb[i] as f32;
                        let text = format!("{v:.0}");
                        if sai_ui::bar(ui, &mut v, 0.0, 255.0, text, ui.available_width() - 8.0, true).changed() {
                            rgb[i] = v.round() as u8;
                        }
                    });
                }
                let c = Color32::from_rgb(rgb[0], rgb[1], rgb[2]);
                if c != self.color() {
                    self.set_color(c);
                }
                let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 22.0), egui::Sense::hover());
                ui.painter().rect_filled(rect, 2.0, self.color());
                ui.painter().rect_stroke(rect, 2.0, egui::Stroke::new(1.0_f32, sai_ui::border()));
            });
        });
        ui.separator();
        ui.label(tr("Образцы"));
        let mut swatch = None;
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = Vec2::new(2.0, 2.0);
            for c in self.recent_colors.iter().copied() {
                let (rect, resp) = ui.allocate_exact_size(Vec2::new(22.0, 22.0), egui::Sense::click());
                ui.painter().rect_filled(rect, 1.0, c);
                ui.painter().rect_stroke(rect, 1.0, egui::Stroke::new(1.0_f32, sai_ui::border()));
                if resp.clicked() {
                    swatch = Some(c);
                }
            }
        });
        if let Some(c) = swatch {
            self.set_color(c);
            self.mobile.sheet = Sheet::None;
        }
    }

    /// Меню: разделы наверху, пункты выбранного ниже — без всплывающих окон.
    fn phone_menu(&mut self, ui: &mut egui::Ui) {
        const SECTIONS: [&str; 7] = ["Файл", "Правка", "Холст", "Слой", "Выделение", "Вид", "Окно"];
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing.x = 2.0;
            for (i, name) in SECTIONS.iter().enumerate() {
                if ui.selectable_label(self.mobile.menu == Some(i), tr(name)).clicked() {
                    self.mobile.menu = if self.mobile.menu == Some(i) { None } else { Some(i) };
                }
            }
        });
        let Some(sec) = self.mobile.menu else { return };
        ui.separator();
        let close = match sec {
            0 => self.phone_menu_file(ui),
            1 => self.phone_menu_edit(ui),
            2 => self.phone_menu_canvas(ui),
            3 => self.phone_menu_layer(ui),
            4 => self.phone_menu_selection(ui),
            5 => self.phone_menu_view(ui),
            _ => self.phone_menu_window(ui),
        };
        if close {
            self.mobile.sheet = Sheet::None;
            self.mobile.menu = None;
        }
    }

    fn phone_menu_file(&mut self, ui: &mut egui::Ui) -> bool {
        let mut close = false;
        for (name, tip) in [
            ("Создать холст…", ""),
            ("Открыть…", ""),
            ("Сохранить", ""),
            ("Сохранить как…", ""),
            ("Сохранить с номером", ""),
        ] {
            if ui.add(egui::Button::new(tr(name)).min_size(Vec2::new(ui.available_width(), 30.0))).on_hover_text(tip).clicked() {
                match name {
                    "Создать холст…" => self.show_new = true,
                    "Открыть…" => self.ask_open(),
                    "Сохранить" => self.save(),
                    "Сохранить как…" => self.ask_save_as(),
                    _ => self.save_numbered(),
                }
                close = true;
            }
        }
        close
    }

    fn phone_menu_edit(&mut self, ui: &mut egui::Ui) -> bool {
        let mut close = false;
        for (name, on, act) in [
            ("Отменить", self.doc.can_undo(), 0u8),
            ("Повторить", self.doc.can_redo(), 1),
            ("Вырезать", true, 2),
            ("Копировать", true, 3),
            ("Вставить", true, 4),
        ] {
            if ui.add_enabled(on, egui::Button::new(tr(name)).min_size(Vec2::new(ui.available_width(), 30.0))).clicked() {
                match act {
                    0 => self.undo(),
                    1 => self.redo(),
                    2 => self.copy(true, false),
                    3 => self.copy(false, false),
                    _ => self.paste(),
                }
                close = act < 2;
            }
        }
        close
    }

    fn phone_menu_canvas(&mut self, ui: &mut egui::Ui) -> bool {
        let mut close = false;
        for (name, act) in [
            ("Изменить разрешение…", 0u8),
            ("Изменить размер…", 1),
            ("Отразить по горизонтали", 2),
            ("Отразить по вертикали", 3),
            ("Повернуть на 90° против часовой", 4),
            ("Повернуть на 90° по часовой", 5),
        ] {
            if ui.add(egui::Button::new(tr(name)).min_size(Vec2::new(ui.available_width(), 30.0))).clicked() {
                match act {
                    0 => self.open_canvas_dialog(true),
                    1 => self.open_canvas_dialog(false),
                    2..=5 => {
                        self.rec(crate::actions::Step::Canvas(act - 2));
                        self.canvas_op(|d| match act {
                            2 => d.flip_canvas(true),
                            3 => d.flip_canvas(false),
                            4 => d.rotate_canvas(false),
                            _ => d.rotate_canvas(true),
                        });
                    }
                    _ => {}
                }
                close = true;
            }
        }
        close
    }

    fn phone_menu_layer(&mut self, ui: &mut egui::Ui) -> bool {
        let mut close = false;
        for (name, act) in [
            ("Новый слой", 0u8),
            ("Новая папка", 1),
            ("Слой выше", 2),
            ("Слой ниже", 3),
            ("Слить с нижним", 4),
            ("Очистить слой", 5),
            ("Удалить слой", 6),
        ] {
            if ui.add(egui::Button::new(tr(name)).min_size(Vec2::new(ui.available_width(), 30.0))).clicked() {
                match act {
                    0 => self.add_layer(false),
                    1 => self.add_layer(true),
                    2 => self.layer_action(crate::LayerAction::Up),
                    3 => self.layer_action(crate::LayerAction::Down),
                    4 => self.layer_action(crate::LayerAction::Merge),
                    5 => self.layer_action(crate::LayerAction::Clear),
                    _ => self.layer_action(crate::LayerAction::Delete),
                }
                close = act < 2;
            }
        }
        close
    }

    fn phone_menu_selection(&mut self, ui: &mut egui::Ui) -> bool {
        let mut close = false;
        for (name, act) in [("Выделить всё", 0u8), ("Снять выделение", 1), ("Инвертировать", 2)] {
            if ui.add(egui::Button::new(tr(name)).min_size(Vec2::new(ui.available_width(), 30.0))).clicked() {
                match act {
                    0 => self.select_all(),
                    1 => self.deselect(),
                    _ => self.invert_selection(),
                }
                close = true;
            }
        }
        close
    }

    fn phone_menu_view(&mut self, ui: &mut egui::Ui) -> bool {
        let mut close = false;
        for (name, act) in [
            ("По окну", 0u8),
            ("Масштаб 100%", 1),
            ("Сбросить поворот", 2),
            ("Отразить вид", 3),
            ("Показать выделение", 4),
        ] {
            if ui.add(egui::Button::new(tr(name)).min_size(Vec2::new(ui.available_width(), 30.0))).clicked() {
                match act {
                    0 => self.fit_pending = true,
                    1 => self.set_zoom(1.0 / self.ppp),
                    2 => self.rotate_view(0.0),
                    3 => self.view_flip = !self.view_flip,
                    _ => self.show_selection = !self.show_selection,
                }
                close = true;
            }
        }
        ui.horizontal(|ui| {
            ui.label(tr("Сглаживание"));
            self.smoothing_picker(ui);
        });
        close
    }

    fn phone_menu_window(&mut self, ui: &mut egui::Ui) -> bool {
        ui.horizontal(|ui| {
            ui.label(tr("Масштаб"));
            for s in crate::UI_SCALES {
                if ui.selectable_label((self.win.ui_scale - s).abs() < 0.01, format!("{:.0}%", s * 100.0)).clicked() {
                    self.win.ui_scale = s;
                    self.save_settings();
                }
            }
        });
        ui.separator();
        // десктопные панели на телефоне: вкладки и панели как на компьютере
        if ui.checkbox(&mut self.android_phone, tr("Телефонный интерфейс")).on_hover_text(tr("Выключить — показать панели как на компьютере")).changed() {
            // дерево вкладок собирается заново: на телефоне его подменял лист
            if !self.android_phone {
                self.win.dock = true;
                self.dock = crate::dock::default_layout(self.win.tools_right, self.win.layers_right);
                self.dock_saved = self.dock_text();
            }
            self.save_settings();
            return true;
        }
        false
    }
}
