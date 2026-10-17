//! Motion: shared spring stepping for overlays and panels.

use super::*;

impl WindowsAppState {
    pub(super) fn update_motion(&mut self) {
        let now = Instant::now();
        let dt = now.duration_since(self.last_frame).as_secs_f32().min(0.04);
        self.last_frame = now;
        self.update_tree_rows(dt);
        Self::step_overlay_motion(
            &mut self.relation_focus_motion,
            &mut self.relation_focus_velocity,
            self.hovered_symbol.is_some(),
            dt,
        );
        if self.hovered_symbol.is_none() && self.relation_focus_motion <= 0.001 {
            self.relation_symbol = None;
        }
        Self::step_overlay_motion(
            &mut self.comment_bubbles_motion,
            &mut self.comment_bubbles_velocity,
            self.comment_bubbles_target,
            dt,
        );
        let lines = self
            .expanded_comments
            .iter()
            .copied()
            .chain(self.comment_expansion.keys().copied())
            .collect::<std::collections::BTreeSet<_>>();
        for line in lines {
            let motion = self.comment_expansion.entry(line).or_insert((0.0, 0.0));
            Self::step_overlay_motion(
                &mut motion.0,
                &mut motion.1,
                self.expanded_comments.contains(&line),
                dt,
            );
        }
        self.comment_expansion
            .retain(|line, motion| self.expanded_comments.contains(line) || motion.0 > 0.001);
        let target = if self.terminal_open { 1.0 } else { 0.0 };
        self.terminal_velocity += (target - self.terminal_motion) * 235.0 * dt;
        self.terminal_velocity *= (-17.0 * dt).exp();
        self.terminal_motion += self.terminal_velocity * dt;
        self.terminal_motion = self.terminal_motion.clamp(-0.05, 1.05);
        if (target - self.terminal_motion).abs() < 0.001 && self.terminal_velocity.abs() < 0.001 {
            self.terminal_motion = target;
            self.terminal_velocity = 0.0;
        }
        Self::step_overlay_motion(
            &mut self.settings_motion,
            &mut self.settings_velocity,
            self.settings_open,
            dt,
        );
        Self::step_overlay_motion(
            &mut self.extensions_motion,
            &mut self.extensions_velocity,
            self.extensions_open,
            dt,
        );
        Self::step_overlay_motion(
            &mut self.package_motion,
            &mut self.package_velocity,
            self.package_open,
            dt,
        );
        Self::step_overlay_motion(
            &mut self.create_motion,
            &mut self.create_velocity,
            self.create_kind.is_some(),
            dt,
        );
        if self.package_running {
            // Follow the real build stages; never advance without progress.
            let reported = self.package_output_progress().unwrap_or(0.0);
            self.package_progress = self.package_progress.max(reported).min(0.95);
            let done = self.terminals.first().is_some_and(|tab| {
                tab.session.view.lock().ok().is_some_and(|view| {
                    view.lines
                        .iter()
                        .any(|line| line.contains(&self.package_marker))
                        || view.current.contains(&self.package_marker)
                })
            });
            let failure_line = self.terminals.first().and_then(|tab| {
                tab.session.view.lock().ok().and_then(|view| {
                    view.lines
                        .iter()
                        .rev()
                        .find(|line| {
                            !line.contains(&self.package_failure_marker) && !line.is_empty()
                        })
                        .cloned()
                })
            });
            let failed = self.terminals.first().is_some_and(|tab| {
                tab.session.view.lock().ok().is_some_and(|view| {
                    view.lines
                        .iter()
                        .any(|line| line.contains(&self.package_failure_marker))
                        || view.current.contains(&self.package_failure_marker)
                })
            });
            if failed {
                self.package_failed_command = failure_line;
            }
            if failed {
                self.package_running = false;
                self.package_progress = 0.0;
                self.package_started = None;
                let command = self.package_failed_command.clone();
                self.finish_package(command);
            } else if done {
                self.package_running = false;
                self.package_progress = 1.0;
                self.package_reveal_pending = true;
                self.package_started = Some(Instant::now());
                self.finish_package(None);
                self.status = "Packaging finished; opening the artifact folder".into();
            }
        } else if self.package_progress >= 1.0
            && self
                .package_started
                .is_some_and(|started| started.elapsed().as_secs_f32() > 1.8)
        {
            self.package_progress = 0.0;
            self.package_started = None;
        }
    }

    /// Slide the project tree rows to the place the current list gives them.
    ///
    /// Folding a folder drops the rows below it, and a list drawn straight from
    /// row indices would snap them up. The shared row shift keeps one spring per
    /// entry path, so the rows that survive the fold slide into place and the
    /// rows of an unfolded folder push the list down again.
    pub(super) fn update_tree_rows(&mut self, dt: f32) {
        let query = self.workspace_search.to_ascii_lowercase();
        let mut positions = Vec::with_capacity(self.project_entries.len());
        let mut row = 0usize;
        for (index, entry) in self.project_entries.iter().enumerate() {
            if !query.is_empty() && !entry.to_ascii_lowercase().contains(&query) {
                continue;
            }
            let key = self.project_entry_paths[index]
                .to_string_lossy()
                .into_owned();
            positions.push(self.tree_row_shift.place(&key, row as f32, dt));
            row += 1;
        }
        self.tree_row_shift.prune();
        self.tree_row_positions = positions;
    }

    /// Position the project tree rows are drawn at, or their list index.
    pub(super) fn tree_row_position(&self, row: usize) -> f32 {
        self.tree_row_positions
            .get(row)
            .copied()
            .unwrap_or(row as f32)
    }

    /// Shared spring step, so overlays move the same way on every backend.
    pub(super) fn step_overlay_motion(value: &mut f32, velocity: &mut f32, open: bool, dt: f32) {
        let target = if open { 1.0 } else { 0.0 };
        motion_core::step(value, velocity, target, dt, motion_core::OVERLAY);
    }
}
