use super::*;
use gpui::{App, Entity, Modifiers, PlatformInput, TestAppContext, VisualTestContext, point};
use std::cell::RefCell;

#[derive(Default)]
struct Delegate {
    sorts: Vec<(usize, ColumnSort)>,
}

impl TableDelegate for Delegate {
    fn columns_count(&self, _: &App) -> usize {
        2
    }
    fn rows_count(&self, _: &App) -> usize {
        0
    }
    fn column(&self, ix: usize, _: &App) -> Column {
        Column {
            key: format!("column-{ix}").into(),
            name: format!("Column {ix}").into(),
            width: px(100.),
            min_width: px(80.),
            max_width: px(220.),
            sort: Some(ColumnSort::Default),
            ..Default::default()
        }
    }
    fn render_td(
        &mut self,
        _: usize,
        _: usize,
        _: &mut Window,
        _: &mut Context<TableState<Self>>,
    ) -> impl IntoElement {
        div()
    }
    fn render_empty(
        &mut self,
        _: &mut Window,
        _: &mut Context<TableState<Self>>,
    ) -> impl IntoElement {
        div()
    }
    fn perform_sort(
        &mut self,
        ix: usize,
        sort: ColumnSort,
        _: &mut Window,
        _: &mut Context<TableState<Self>>,
    ) {
        self.sorts.push((ix, sort));
    }
}

type State = Entity<TableState<Delegate>>;
type Changes = Rc<RefCell<Vec<(usize, Vec<Pixels>)>>>;
struct Harness {
    tables: [State; 2],
}
impl Render for Harness {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .w(px(400.))
            .h(px(320.))
            .children(self.tables.iter().map(|state| {
                div()
                    .w(px(400.))
                    .h(px(160.))
                    .flex_none()
                    .child(DataTable::new(state).bordered(false))
            }))
    }
}
fn harness(cx: &mut TestAppContext) -> (&mut VisualTestContext, [State; 2], Changes) {
    cx.update(crate::init);
    let changes = Changes::default();
    let (view, cx) = cx.add_window_view({
        let changes = changes.clone();
        move |window, cx| {
            let tables = std::array::from_fn(|ix| {
                let table = cx.new(|cx| TableState::new(Delegate::default(), window, cx));
                let changes = changes.clone();
                cx.subscribe(&table, move |_, _, event, _| {
                    if let TableEvent::ColumnWidthsChanged(widths) = event {
                        changes.borrow_mut().push((ix, widths.clone()));
                    }
                })
                .detach();
                table
            });
            Harness { tables }
        }
    });
    draw(cx);
    let tables = view.read_with(cx, |view, _| view.tables.clone());
    (cx, tables, changes)
}
fn draw(cx: &mut VisualTestContext) {
    cx.run_until_parked();
    cx.update(|window, cx| {
        window.draw(cx).clear(cx);
    });
}
fn bounds(cx: &VisualTestContext, table: &State) -> Bounds<Pixels> {
    table.read_with(cx, |table, _| table.col_groups[0].bounds)
}
fn width(cx: &VisualTestContext, table: &State) -> Pixels {
    table.read_with(cx, |table, _| table.col_groups[0].width)
}
fn begin(cx: &mut VisualTestContext, table: &State, offset: f32) -> Bounds<Pixels> {
    let b = bounds(cx, table);
    cx.simulate_mouse_down(
        point(b.right() + px(offset), b.center().y),
        MouseButton::Left,
        Modifiers::default(),
    );
    b
}
fn move_raw(window: &mut Window, cx: &mut App, position: Point<Pixels>) {
    window.dispatch_event(
        PlatformInput::MouseMove(gpui::MouseMoveEvent {
            position,
            pressed_button: Some(MouseButton::Left),
            modifiers: Modifiers::default(),
        }),
        cx,
    );
}
fn up_raw(window: &mut Window, cx: &mut App, position: Point<Pixels>, button: MouseButton) {
    window.dispatch_event(
        PlatformInput::MouseUp(gpui::MouseUpEvent {
            position,
            button,
            modifiers: Modifiers::default(),
            click_count: 1,
        }),
        cx,
    );
}

#[gpui::test]
fn short_drag_uses_first_move_and_final_release_from_both_handle_halves(cx: &mut TestAppContext) {
    let (cx, tables, changes) = harness(cx);
    for (offset, target) in [(-2., 160.), (2., 190.)] {
        let b = begin(cx, &tables[0], offset);
        cx.update(|window, cx| {
            // 首次移动与松手之间不画帧，覆盖实际短拖时序。
            move_raw(
                window,
                cx,
                point(b.left() + px(target - 10.) + HANDLE_SIZE, b.center().y),
            );
            assert_eq!(tables[0].read(cx).col_groups[0].width, px(target - 10.));
            up_raw(
                window,
                cx,
                point(b.left() + px(target) + HANDLE_SIZE, b.center().y),
                MouseButton::Left,
            );
            assert_eq!(tables[0].read(cx).col_groups[0].width, px(target));
            assert!(tables[0].read(cx).resizing_col.is_none());
            assert!(!cx.has_active_drag());
        });
        draw(cx);
    }
    assert_eq!(changes.borrow().len(), 2);
    assert_eq!(width(cx, &tables[1]), px(100.));
    cx.simulate_mouse_move(point(px(50.), px(15.)), None, Modifiers::default());
    assert_eq!(width(cx, &tables[0]), px(190.));
    assert_eq!(changes.borrow().len(), 2);
}

#[gpui::test]
fn release_inside_handle_and_secondary_release_clean_state_without_cross_table_drift(
    cx: &mut TestAppContext,
) {
    let (cx, tables, changes) = harness(cx);
    let b = begin(cx, &tables[0], -2.);
    cx.simulate_mouse_move(
        point(b.left() + px(152.), b.center().y),
        Some(MouseButton::Left),
        Modifiers::default(),
    );
    draw(cx);
    let moved = bounds(cx, &tables[0]);
    cx.simulate_mouse_up(
        point(moved.right(), moved.center().y),
        MouseButton::Left,
        Modifiers::default(),
    );
    assert_eq!(width(cx, &tables[0]), px(148.));
    draw(cx);
    let b = begin(cx, &tables[0], -2.);
    cx.update(|window, cx| {
        move_raw(window, cx, point(b.left() + px(172.), b.center().y));
        up_raw(
            window,
            cx,
            point(b.left() + px(210.), b.center().y),
            MouseButton::Right,
        );
        assert_eq!(tables[0].read(cx).col_groups[0].width, px(170.));
        assert!(tables[0].read(cx).resizing_col.is_none());
    });
    draw(cx);
    let b = begin(cx, &tables[1], 2.);
    cx.update(|window, cx| {
        move_raw(window, cx, point(b.left() + px(182.), b.center().y));
        up_raw(
            window,
            cx,
            point(b.left() + px(192.), b.center().y),
            MouseButton::Left,
        );
    });
    draw(cx);
    assert_eq!(width(cx, &tables[0]), px(170.));
    assert_eq!(width(cx, &tables[1]), px(190.));
    assert_eq!(
        changes
            .borrow()
            .iter()
            .map(|(ix, _)| *ix)
            .collect::<Vec<_>>(),
        vec![0, 0, 1]
    );
}

#[gpui::test]
fn drag_respects_limits_and_header_click_still_sorts(cx: &mut TestAppContext) {
    let (cx, tables, changes) = harness(cx);
    for (pointer_width, expected) in [(500., 220.), (1., 80.)] {
        let b = begin(cx, &tables[0], -2.);
        cx.update(|window, cx| {
            move_raw(
                window,
                cx,
                point(b.left() + px(pointer_width) + HANDLE_SIZE, b.center().y),
            );
            up_raw(
                window,
                cx,
                point(b.left() + px(pointer_width) + HANDLE_SIZE, b.center().y),
                MouseButton::Left,
            );
        });
        draw(cx);
        assert_eq!(width(cx, &tables[0]), px(expected));
    }
    let b = bounds(cx, &tables[0]);
    cx.simulate_mouse_down(
        point(b.left() + px(20.), b.center().y),
        MouseButton::Left,
        Modifiers::default(),
    );
    cx.simulate_mouse_up(
        point(b.left() + px(20.), b.center().y),
        MouseButton::Left,
        Modifiers::default(),
    );
    draw(cx);
    tables[0].read_with(cx, |table, _| {
        assert_eq!(table.selected_col, Some(0));
        assert!(table.delegate.sorts.is_empty());
    });
    // 表头正文用于选择列；排序按钮在右侧内边距内，首次排序为降序。
    let padding = tables[0].read_with(cx, |table, _| table.options.size.table_cell_padding().right);
    let sort_position = point(b.right() - padding - px(8.), b.center().y);
    cx.simulate_mouse_down(sort_position, MouseButton::Left, Modifiers::default());
    cx.simulate_mouse_up(sort_position, MouseButton::Left, Modifiers::default());
    draw(cx);
    tables[0].read_with(cx, |table, _| {
        assert_eq!(table.delegate.sorts, vec![(0, ColumnSort::Descending)])
    });
    assert_eq!(changes.borrow().len(), 2);
}
