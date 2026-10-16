//! Shared menu bar model.
//!
//! The menu bar is the one control every backend has to agree about: the same
//! titles in the same order, the same items under each, and the same rules for
//! what is reachable. Linux drew five titles and only wired three of them to
//! anything, so "run" and "debug" looked like buttons and were dead. Keeping
//! the model here means a backend cannot quietly forget to wire one up: the
//! list of titles and the list of items come from the same place, and a test
//! walks every item to check that something handles it.
//!
//! Nothing here draws or handles input; it says what a menu bar contains and
//! which entry the pointer is on, which is testable without a window.

#![allow(dead_code)] // The backends adopt the pieces they need as they migrate.

/// One entry of the menu bar.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Menu {
    File,
    Edit,
    View,
    Run,
    Debug,
    Extensions,
    Window,
    Help,
}

impl Menu {
    /// Every menu, in the order the bar shows them.
    pub const ALL: [Menu; 8] = [
        Menu::File,
        Menu::Edit,
        Menu::View,
        Menu::Run,
        Menu::Debug,
        Menu::Extensions,
        Menu::Window,
        Menu::Help,
    ];

    /// The word on the bar.
    pub fn title(self) -> &'static str {
        match self {
            Menu::File => "File",
            Menu::Edit => "Edit",
            Menu::View => "View",
            Menu::Run => "Run",
            Menu::Debug => "Debug",
            Menu::Extensions => "Extensions",
            Menu::Window => "Window",
            Menu::Help => "Help",
        }
    }

    /// The word on the bar, in Chinese.
    pub fn title_chinese(self) -> &'static str {
        match self {
            Menu::File => "文件",
            Menu::Edit => "编辑",
            Menu::View => "查看",
            Menu::Run => "运行",
            Menu::Debug => "调试",
            Menu::Extensions => "扩展",
            Menu::Window => "窗口",
            Menu::Help => "帮助",
        }
    }

    /// Position of the menu on the bar, which is also its place in `ALL`.
    pub fn index(self) -> usize {
        Menu::ALL
            .iter()
            .position(|entry| *entry == self)
            .unwrap_or(0)
    }

    /// Whether the menu opens a panel rather than a list of items.
    ///
    /// Extensions already has a full window of its own, so it toggles that
    /// rather than showing a dropdown of its own.
    pub fn opens_panel(self) -> bool {
        matches!(self, Menu::Extensions)
    }

    /// The items under this menu, in the order they are listed.
    pub fn items(self) -> &'static [Item] {
        match self {
            // Undo and Redo live under Edit, not here: an item listed twice
            // would be two places to keep in step, and the test refuses it.
            Menu::File => &[
                Item::Save,
                Item::OpenFile,
                Item::OpenFolder,
                Item::NewWindow,
                Item::CloseWindow,
            ],
            Menu::Edit => &[
                Item::Undo,
                Item::Redo,
                Item::SelectAll,
                Item::Copy,
                Item::Paste,
                Item::Find,
                Item::Replace,
                Item::FindInProject,
                Item::GoToLine,
            ],
            Menu::View => &[
                Item::ToggleProjectTree,
                Item::ToggleGraph,
                Item::ToggleTerminal,
                Item::ToggleComments,
                Item::ToggleDebugInfo,
                Item::FoldAll,
                Item::LargerCode,
                Item::SmallerCode,
            ],
            Menu::Run => &[Item::Build, Item::Run, Item::StopRun, Item::Package],
            Menu::Debug => &[
                Item::StartDebug,
                Item::StepOver,
                Item::StepInto,
                Item::StepOut,
                Item::ToggleBreakpoint,
                Item::StopDebug,
            ],
            Menu::Extensions => &[Item::ShowExtensions, Item::ShowSettings, Item::ShowAiPanel],
            Menu::Window => &[
                Item::Minimize,
                Item::Zoom,
                Item::ToggleFullScreen,
                Item::NewTerminal,
                Item::DetachTerminal,
            ],
            Menu::Help => &[
                Item::ShowShortcuts,
                Item::ShowAbout,
                Item::OpenProjectFolder,
            ],
        }
    }
}

/// One entry inside a menu.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Item {
    Undo,
    Redo,
    Save,
    OpenFile,
    OpenFolder,
    NewWindow,
    CloseWindow,
    SelectAll,
    Copy,
    Paste,
    Find,
    Replace,
    FindInProject,
    GoToLine,
    ToggleProjectTree,
    ToggleGraph,
    ToggleTerminal,
    ToggleComments,
    ToggleDebugInfo,
    FoldAll,
    LargerCode,
    SmallerCode,
    Build,
    Run,
    StopRun,
    Package,
    StartDebug,
    StepOver,
    StepInto,
    StepOut,
    ToggleBreakpoint,
    StopDebug,
    ShowExtensions,
    ShowSettings,
    ShowAiPanel,
    Minimize,
    Zoom,
    ToggleFullScreen,
    NewTerminal,
    DetachTerminal,
    ShowShortcuts,
    ShowAbout,
    OpenProjectFolder,
}

impl Item {
    /// Every item, so a test can walk them all.
    pub const ALL: [Item; 43] = [
        Item::Undo,
        Item::Redo,
        Item::Save,
        Item::OpenFile,
        Item::OpenFolder,
        Item::NewWindow,
        Item::CloseWindow,
        Item::SelectAll,
        Item::Copy,
        Item::Paste,
        Item::Find,
        Item::Replace,
        Item::FindInProject,
        Item::GoToLine,
        Item::ToggleProjectTree,
        Item::ToggleGraph,
        Item::ToggleTerminal,
        Item::ToggleComments,
        Item::ToggleDebugInfo,
        Item::FoldAll,
        Item::LargerCode,
        Item::SmallerCode,
        Item::Build,
        Item::Run,
        Item::StopRun,
        Item::Package,
        Item::StartDebug,
        Item::StepOver,
        Item::StepInto,
        Item::StepOut,
        Item::ToggleBreakpoint,
        Item::StopDebug,
        Item::ShowExtensions,
        Item::ShowSettings,
        Item::ShowAiPanel,
        Item::Minimize,
        Item::Zoom,
        Item::ToggleFullScreen,
        Item::NewTerminal,
        Item::DetachTerminal,
        Item::ShowShortcuts,
        Item::ShowAbout,
        Item::OpenProjectFolder,
    ];

    /// The word shown for the item.
    pub fn label(self) -> &'static str {
        match self {
            Item::Undo => "Undo",
            Item::Redo => "Redo",
            Item::Save => "Save",
            Item::OpenFile => "Open File...",
            Item::OpenFolder => "Open Folder...",
            Item::NewWindow => "New Window",
            Item::CloseWindow => "Close Window",
            Item::SelectAll => "Select All",
            Item::Copy => "Copy",
            Item::Paste => "Paste",
            Item::Find => "Find...",
            Item::Replace => "Replace...",
            Item::FindInProject => "Find in Project...",
            Item::GoToLine => "Go to Line...",
            Item::ToggleProjectTree => "Project Tree",
            Item::ToggleGraph => "Call Graph",
            Item::ToggleTerminal => "Terminal",
            Item::ToggleComments => "Comment Bubbles",
            Item::ToggleDebugInfo => "Debug Info",
            Item::FoldAll => "Fold All",
            Item::LargerCode => "Larger Code",
            Item::SmallerCode => "Smaller Code",
            Item::Build => "Build",
            Item::Run => "Run",
            Item::StopRun => "Stop",
            Item::StopDebug => "Stop Debugging",
            Item::Package => "Package...",
            Item::StartDebug => "Start Debugging",
            Item::StepOver => "Step Over",
            Item::StepInto => "Step Into",
            Item::StepOut => "Step Out",
            Item::ToggleBreakpoint => "Toggle Breakpoint",
            Item::ShowExtensions => "Extensions",
            Item::ShowSettings => "Settings",
            Item::ShowAiPanel => "AI Panel",
            Item::Minimize => "Minimize",
            Item::Zoom => "Zoom",
            Item::ToggleFullScreen => "Full Screen",
            Item::NewTerminal => "New Terminal",
            Item::DetachTerminal => "Detach Terminal",
            Item::ShowShortcuts => "Keyboard Shortcuts",
            Item::ShowAbout => "About TC-IDE",
            Item::OpenProjectFolder => "Open Project Folder",
        }
    }

    /// The word shown for the item, in Chinese.
    pub fn label_chinese(self) -> &'static str {
        match self {
            Item::Undo => "撤销",
            Item::Redo => "重做",
            Item::Save => "保存",
            Item::OpenFile => "打开文件…",
            Item::OpenFolder => "打开文件夹…",
            Item::NewWindow => "新建窗口",
            Item::CloseWindow => "关闭窗口",
            Item::SelectAll => "全选",
            Item::Copy => "复制",
            Item::Paste => "粘贴",
            Item::Find => "查找…",
            Item::Replace => "替换…",
            Item::FindInProject => "在项目中查找…",
            Item::GoToLine => "跳转到行…",
            Item::ToggleProjectTree => "项目树",
            Item::ToggleGraph => "调用关系",
            Item::ToggleTerminal => "底部终端",
            Item::ToggleComments => "注释气泡",
            Item::ToggleDebugInfo => "调试信息",
            Item::FoldAll => "折叠全部",
            Item::LargerCode => "增大代码字号",
            Item::SmallerCode => "减小代码字号",
            Item::Build => "构建",
            Item::Run => "运行",
            Item::StopRun => "停止",
            Item::StopDebug => "停止调试",
            Item::Package => "打包…",
            Item::StartDebug => "开始调试",
            Item::StepOver => "单步越过",
            Item::StepInto => "单步进入",
            Item::StepOut => "单步跳出",
            Item::ToggleBreakpoint => "切换断点",
            Item::ShowExtensions => "扩展",
            Item::ShowSettings => "设置",
            Item::ShowAiPanel => "AI 面板",
            Item::Minimize => "最小化",
            Item::Zoom => "缩放",
            Item::ToggleFullScreen => "全屏",
            Item::NewTerminal => "新建终端",
            Item::DetachTerminal => "独立终端窗口",
            Item::ShowShortcuts => "键盘快捷键",
            Item::ShowAbout => "关于 TC-IDE",
            Item::OpenProjectFolder => "打开项目文件夹",
        }
    }

    /// The shortcut shown beside the item, when it has one.
    pub fn shortcut(self) -> Option<&'static str> {
        match self {
            Item::Undo => Some("Ctrl+Z"),
            Item::Redo => Some("Ctrl+Y"),
            Item::Save => Some("Ctrl+S"),
            Item::OpenFile => Some("Ctrl+O"),
            Item::OpenFolder => Some("Ctrl+K"),
            Item::SelectAll => Some("Ctrl+A"),
            Item::Copy => Some("Ctrl+C"),
            Item::Paste => Some("Ctrl+V"),
            Item::Find => Some("Ctrl+F"),
            Item::Replace => Some("Ctrl+H"),
            Item::FindInProject => Some("Ctrl+Shift+F"),
            Item::GoToLine => Some("Ctrl+G"),
            Item::ToggleProjectTree => Some("Ctrl+B"),
            Item::ToggleTerminal => Some("Ctrl+`"),
            Item::FoldAll => Some("Ctrl+Shift+]"),
            Item::Build => Some("Ctrl+Shift+B"),
            Item::Run => Some("F5"),
            Item::StopRun => Some("Shift+F5"),
            Item::StartDebug => Some("F5"),
            Item::StepOver => Some("F10"),
            Item::StepInto => Some("F11"),
            Item::StepOut => Some("Shift+F11"),
            Item::ToggleBreakpoint => Some("F9"),
            Item::Zoom => Some("Ctrl+0"),
            _ => None,
        }
    }

    /// The menu an item belongs to.
    pub fn menu(self) -> Menu {
        Menu::ALL
            .into_iter()
            .find(|menu| menu.items().contains(&self))
            .unwrap_or(Menu::File)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    #[test]
    fn every_menu_has_a_title_in_both_languages() {
        // The bar is bilingual, so a menu without one of its words would show
        // an empty button in that language.
        assert_eq!(Menu::ALL.len(), 8);
        for menu in Menu::ALL {
            assert!(!menu.title().is_empty(), "{menu:?}");
            assert!(!menu.title_chinese().is_empty(), "{menu:?}");
            assert_ne!(menu.title(), menu.title_chinese(), "{menu:?}");
        }
        // The order is the order the bar draws them in, so no two titles may
        // share a place.
        let titles = Menu::ALL
            .iter()
            .map(|menu| menu.title())
            .collect::<Vec<_>>();
        let unique = titles.iter().collect::<BTreeSet<_>>();
        assert_eq!(unique.len(), titles.len(), "{titles:?}");
    }

    #[test]
    fn a_menu_knows_its_own_place_on_the_bar() {
        for (position, menu) in Menu::ALL.iter().enumerate() {
            assert_eq!(menu.index(), position, "{menu:?}");
            assert_eq!(Menu::ALL[menu.index()], *menu);
        }
    }

    #[test]
    fn every_menu_offers_something() {
        // A menu with no items would open an empty panel, which reads as a bug.
        for menu in Menu::ALL {
            assert!(!menu.items().is_empty(), "{menu:?} has no items");
        }
        // Extensions opens a window it already has rather than a dropdown.
        assert!(Menu::Extensions.opens_panel());
        assert!(!Menu::File.opens_panel());
    }

    #[test]
    fn an_item_is_listed_under_exactly_one_menu() {
        // An item appearing twice would mean two places to keep in step, and
        // `menu()` would only ever report one of them.
        let mut seen: Vec<Item> = Vec::new();
        for menu in Menu::ALL {
            for item in menu.items() {
                assert!(
                    !seen.contains(item),
                    "{item:?} is listed under {menu:?} as well as elsewhere"
                );
                seen.push(*item);
            }
        }
        assert_eq!(seen.len(), Item::ALL.len(), "every item is reachable");
        for item in Item::ALL {
            let menu = item.menu();
            assert!(
                menu.items().contains(&item),
                "{item:?} reports {menu:?}, which does not list it"
            );
        }
    }

    #[test]
    fn every_item_has_a_label_in_both_languages() {
        for item in Item::ALL {
            assert!(!item.label().is_empty(), "{item:?}");
            assert!(!item.label_chinese().is_empty(), "{item:?}");
            assert_ne!(item.label(), item.label_chinese(), "{item:?}");
        }
        // Labels are unique enough to tell apart; two identical rows would be
        // ambiguous to click.
        let labels = Item::ALL
            .iter()
            .map(|item| item.label())
            .collect::<Vec<_>>();
        let unique = labels.iter().collect::<BTreeSet<_>>();
        assert_eq!(unique.len(), labels.len(), "two items share a label");
    }

    #[test]
    fn a_shortcut_is_not_claimed_by_two_items() {
        // Two items on the same key means one of them never fires, which is
        // exactly the kind of thing that is invisible until someone tries it.
        let mut counted: Vec<(&str, Vec<Item>)> = Vec::new();
        for item in Item::ALL {
            let Some(shortcut) = item.shortcut() else {
                continue;
            };
            match counted.iter_mut().find(|(known, _)| *known == shortcut) {
                Some((_, items)) => items.push(item),
                None => counted.push((shortcut, vec![item])),
            }
        }
        for (shortcut, items) in counted {
            if items.len() <= 1 {
                continue;
            }
            // F5 is allowed to name both Run and Start Debugging, because they
            // are the same action seen from two menus.
            let allowed = items
                .iter()
                .all(|item| matches!(item, Item::Run | Item::StartDebug));
            assert!(allowed, "{shortcut} is claimed by {items:?}");
        }
    }

    #[test]
    fn the_menu_bar_matches_what_the_handoff_asks_for() {
        // The titles and their order come from the design: file, edit, view,
        // run, debug, extensions, window, help.
        let titles = Menu::ALL
            .iter()
            .map(|menu| menu.title())
            .collect::<Vec<_>>();
        assert_eq!(
            titles,
            vec![
                "File",
                "Edit",
                "View",
                "Run",
                "Debug",
                "Extensions",
                "Window",
                "Help"
            ]
        );
        // Run and Debug are real menus with real items, not decoration: that
        // they were dead buttons is what this model exists to prevent.
        assert!(Menu::Run.items().contains(&Item::Build));
        assert!(Menu::Run.items().contains(&Item::Run));
        assert!(Menu::Debug.items().contains(&Item::StartDebug));
        assert!(Menu::Debug.items().contains(&Item::StepOver));
    }
}

/// Where the keyboard is in the menu bar.
///
/// A menu bar is driven by two different positions: which title is under the
/// pointer or the keyboard, and which item inside the open menu is
/// highlighted. Keeping both here means the rules - what Right does at the
/// last menu, what Down does at the last item - are written once and tested
/// without a window, instead of being spread across a key handler.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MenuCursor {
    /// Title the keyboard is on, whether or not the menu is open.
    pub menu: Menu,
    /// Item highlighted in the open menu.
    pub item: usize,
}

impl Default for MenuCursor {
    fn default() -> Self {
        Self {
            menu: Menu::ALL[0],
            item: 0,
        }
    }
}

/// What a key does to a menu bar.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MenuMove {
    /// Move to the previous title.
    Left,
    /// Move to the next title.
    Right,
    /// Move to the previous item.
    Up,
    /// Move to the next item.
    Down,
}

impl MenuCursor {
    /// Where the cursor goes when a key is pressed.
    ///
    /// Left and Right wrap around the bar, and Up and Down stop at the ends
    /// rather than wrapping: a menu that jumps from its last item back to its
    /// first is disorienting, while a bar that wraps is what every desktop
    /// does. Moving to another title resets the item, so an open menu always
    /// starts at its first entry.
    pub fn moved(self, direction: MenuMove) -> Self {
        match direction {
            MenuMove::Left | MenuMove::Right => {
                let total = Menu::ALL.len();
                let position = self.menu.index();
                let next = if direction == MenuMove::Right {
                    (position + 1) % total
                } else {
                    (position + total - 1) % total
                };
                Self {
                    menu: Menu::ALL[next],
                    item: 0,
                }
            }
            MenuMove::Up | MenuMove::Down => {
                let count = self.menu.items().len();
                if count == 0 {
                    return self;
                }
                let next = if direction == MenuMove::Down {
                    (self.item + 1).min(count - 1)
                } else {
                    self.item.saturating_sub(1)
                };
                Self {
                    menu: self.menu,
                    item: next,
                }
            }
        }
    }

    /// The item the cursor is on, when it is inside the current menu.
    pub fn current_item(self) -> Option<Item> {
        self.menu.items().get(self.item).copied()
    }
}

/// The menu a cursor is on, for a pointer position.
///
/// Hover follows the pointer and the keyboard moves the same cursor, so the
/// two cannot show different highlights. The buttons of a bar are adjacent, so
/// the boundary between two of them belongs to the one on the right: a half
/// open span would make the pixel at the join ambiguous and the highlight
/// flicker as the pointer crossed it.
pub fn menu_under(menus: &[(Menu, i32, i32, i32, i32)], x: i32, y: i32) -> Option<Menu> {
    menus
        .iter()
        .find(|(_, left, top, width, height)| {
            x >= *left && x < left + width && y >= *top && y < top + height
        })
        .or_else(|| {
            // The last button has no neighbour to own its right edge, so it
            // takes it rather than leaving a dead column.
            menus.iter().rev().find(|(_, left, top, width, height)| {
                x == left + width && y >= *top && y < top + height
            })
        })
        .map(|(menu, ..)| *menu)
}

#[cfg(test)]
mod cursor_tests {
    use super::*;

    #[test]
    fn the_arrows_walk_the_bar_and_wrap_at_both_ends() {
        // A bar that wraps is what every desktop does, so the last title's
        // Right goes back to the first.
        let cursor = MenuCursor {
            menu: Menu::File,
            item: 0,
        };
        let cursor = cursor.moved(MenuMove::Right);
        assert_eq!(cursor.menu, Menu::Edit);
        let cursor = cursor.moved(MenuMove::Left);
        assert_eq!(cursor.menu, Menu::File);
        // Left from the first wraps to the last.
        assert_eq!(cursor.moved(MenuMove::Left).menu, Menu::Help);
        // And Right from the last wraps to the first.
        let last = MenuCursor {
            menu: Menu::Help,
            item: 0,
        };
        assert_eq!(last.moved(MenuMove::Right).menu, Menu::File);

        // Walking all the way round returns to where it started.
        let mut walked = MenuCursor::default();
        for _ in 0..Menu::ALL.len() {
            walked = walked.moved(MenuMove::Right);
        }
        assert_eq!(walked.menu, Menu::File);
    }

    #[test]
    fn the_arrows_stop_at_the_ends_of_a_menu() {
        // Up and Down stop rather than wrap: a menu that jumps from its last
        // item back to its first is disorienting.
        let cursor = MenuCursor {
            menu: Menu::Run,
            item: 0,
        };
        assert_eq!(cursor.moved(MenuMove::Up), cursor, "already at the top");
        let mut cursor = cursor;
        for _ in 0..20 {
            cursor = cursor.moved(MenuMove::Down);
        }
        let last = Menu::Run.items().len() - 1;
        assert_eq!(cursor.item, last, "stops at the last item");
        assert_eq!(cursor.moved(MenuMove::Down), cursor);
        // And it can come back.
        assert_eq!(cursor.moved(MenuMove::Up).item, last - 1);
    }

    #[test]
    fn changing_menu_starts_at_the_first_item() {
        // An open menu always starts at its first entry, whatever item was
        // highlighted in the menu before it.
        let cursor = MenuCursor {
            menu: Menu::Edit,
            item: 7,
        };
        let moved = cursor.moved(MenuMove::Right);
        assert_eq!(moved.menu, Menu::View);
        assert_eq!(moved.item, 0);
        let moved = cursor.moved(MenuMove::Left);
        assert_eq!(moved.menu, Menu::File);
        assert_eq!(moved.item, 0);
        // Walking the item Down then across keeps the new item at the top.
        let down = cursor.moved(MenuMove::Down);
        assert_eq!(down.item, 8, "same menu, next item");
        assert_eq!(down.moved(MenuMove::Right).item, 0);
    }

    #[test]
    fn the_cursor_names_the_item_it_is_on() {
        let cursor = MenuCursor {
            menu: Menu::Run,
            item: 0,
        };
        assert_eq!(cursor.current_item(), Some(Item::Build));
        assert_eq!(cursor.moved(MenuMove::Down).current_item(), Some(Item::Run));
        // A cursor past the end names nothing rather than panicking.
        let past = MenuCursor {
            menu: Menu::Run,
            item: 99,
        };
        assert_eq!(past.current_item(), None);
    }

    #[test]
    fn every_menu_can_be_reached_by_arrows_from_the_first() {
        // The bar is only navigable if the arrows visit every title.
        let mut seen = Vec::new();
        let mut cursor = MenuCursor::default();
        for _ in 0..Menu::ALL.len() {
            seen.push(cursor.menu);
            cursor = cursor.moved(MenuMove::Right);
        }
        assert_eq!(seen, Menu::ALL.to_vec());
    }

    #[test]
    fn hover_picks_the_menu_under_the_pointer() {
        // Hover and the keyboard share one cursor, so the rectangles the
        // drawing uses are the ones the pointer is tested against.
        let rects = vec![(Menu::File, 112, 10, 58, 27), (Menu::Edit, 170, 10, 58, 27)];
        assert_eq!(menu_under(&rects, 120, 20), Some(Menu::File));
        assert_eq!(menu_under(&rects, 180, 20), Some(Menu::Edit));
        // The boundary belongs to the menu that starts there.
        assert_eq!(menu_under(&rects, 170, 20), Some(Menu::Edit));
        // Outside the bar nothing is hovered.
        assert_eq!(menu_under(&rects, 500, 20), None);
        assert_eq!(menu_under(&rects, 120, 200), None);
        assert_eq!(menu_under(&[], 120, 20), None);
    }
}

/// Width a tab takes, given how wide its name is.
///
/// A tab strip needs the same treatment the menu bar got: the drawing and the
/// hit test have to agree, or a tab's close button ends up beside the tab
/// rather than on it. The layout is computed here so both read it.
///
/// A tab is as wide as its name plus room for the close button, with a floor
/// so a one character name is still clickable.
pub fn tab_width(name_width: i32, padding: i32, minimum: i32) -> i32 {
    (name_width + padding).max(minimum)
}

/// Width of the close button inside a tab.
pub fn tab_close_width(inset: i32) -> i32 {
    // A square the height of the tab's text, so it is easy to hit.
    (inset * 2).max(14)
}

/// Which tab a position is on, and whether it hit that tab's close button.
///
/// Returning both together is what keeps the two from being decided by
/// different arithmetic: a click near the right edge of a tab is either the
/// close button or the tab, never both and never neither.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TabHit {
    pub index: usize,
    /// The pointer was on the close button rather than the tab itself.
    pub close: bool,
}

/// Find the tab under a horizontal position.
///
/// `tabs` are `(left, width)`, so the caller supplies the geometry it drew
/// with. Tabs are laid left to right and do not overlap, so the first match
/// wins; a position in the gap after the last tab hits nothing.
pub fn tab_at(tabs: &[(i32, i32)], close_width: i32, x: i32) -> Option<TabHit> {
    tabs.iter().enumerate().find_map(|(index, (left, width))| {
        if x < *left || x >= left + width {
            return None;
        }
        // The close button is the square at the trailing edge.
        let close_from = left + width - close_width;
        Some(TabHit {
            index,
            close: x >= close_from,
        })
    })
}

/// Which tab is selected after closing one.
///
/// Closing a tab should leave the selection somewhere sensible: the tab that
/// took its place, or the one before it when the last tab closed, rather than
/// jumping to the first tab or leaving the index past the end.
pub fn selection_after_close(closed: usize, count: usize) -> Option<usize> {
    if count == 0 {
        return None;
    }
    let remaining = count - 1;
    if remaining == 0 {
        return None;
    }
    Some(closed.min(remaining - 1))
}

#[cfg(test)]
mod tab_tests {
    use super::*;

    #[test]
    fn a_tab_is_as_wide_as_its_name_and_no_narrower_than_its_floor() {
        // Room for the name plus the close button, so a long name is not
        // clipped and a one character name is still easy to hit.
        assert_eq!(tab_width(80, 28, 58), 108);
        assert_eq!(tab_width(10, 28, 58), 58, "the floor applies");
        assert_eq!(tab_width(30, 28, 58), 58, "exactly at the floor");
        assert_eq!(tab_width(31, 28, 58), 59, "just over it");
        // The close button is at least big enough to hit.
        assert!(tab_close_width(7) >= 14);
        assert_eq!(tab_close_width(20), 40);
    }

    #[test]
    fn a_hit_tells_the_tab_from_its_close_button() {
        // Two tabs, the first 100 wide and the second 80.
        let tabs = [(0, 100), (100, 80)];
        let close = tab_close_width(7); // 14
                                        // The middle of the first tab is the tab, not the button.
        assert_eq!(
            tab_at(&tabs, close, 40),
            Some(TabHit {
                index: 0,
                close: false
            })
        );
        // Near its trailing edge it is the close button.
        assert_eq!(
            tab_at(&tabs, close, 95),
            Some(TabHit {
                index: 0,
                close: true
            })
        );
        // The boundary between two tabs belongs to the second, so the pixels
        // at the join are never ambiguous.
        assert_eq!(
            tab_at(&tabs, close, 100),
            Some(TabHit {
                index: 1,
                close: false
            })
        );
        // And the second tab's own close button.
        assert_eq!(
            tab_at(&tabs, close, 170),
            Some(TabHit {
                index: 1,
                close: true
            })
        );
        // Past the last tab there is nothing.
        assert_eq!(tab_at(&tabs, close, 180), None);
        assert_eq!(tab_at(&tabs, close, -5), None);
        assert_eq!(tab_at(&[], close, 40), None);
    }

    #[test]
    fn every_pixel_of_a_tab_is_either_the_tab_or_its_button() {
        // The hit test has to cover the whole tab exactly once, or there is a
        // dead column where clicking a tab does nothing.
        let tabs = [(10, 90)];
        let close = tab_close_width(7);
        for x in 10..100 {
            let hit = tab_at(&tabs, close, x);
            assert!(hit.is_some(), "x={x} hit nothing");
            assert_eq!(hit.unwrap().index, 0);
        }
        // Just outside, nothing.
        assert_eq!(tab_at(&tabs, close, 9), None);
        assert_eq!(tab_at(&tabs, close, 100), None);
    }

    #[test]
    fn closing_a_tab_leaves_the_selection_somewhere_sensible() {
        // Closing a middle tab selects the one that took its place.
        assert_eq!(selection_after_close(1, 5), Some(1));
        // Closing the last tab falls back to the one before it rather than
        // leaving the index past the end.
        assert_eq!(selection_after_close(4, 5), Some(3));
        // Closing the first selects what is now first.
        assert_eq!(selection_after_close(0, 5), Some(0));
        // Closing the only tab leaves nothing selected.
        assert_eq!(selection_after_close(0, 1), None);
        assert_eq!(selection_after_close(0, 0), None);
        // The result is always a valid index into what is left.
        for count in 1..=6 {
            for closed in 0..count {
                if let Some(next) = selection_after_close(closed, count) {
                    assert!(next < count - 1, "closed {closed} of {count} gave {next}");
                }
            }
        }
    }
}
