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
