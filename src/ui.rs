use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Gauge, List, ListItem, ListState, Paragraph, Wrap};
use ratatui::Frame;

use crate::app::{App, Dialog, Field, Focus};
use crate::model::{self, RowKind};
use crate::theme::{self, Theme};
use crate::util::{human_size, kernel_name};

pub fn draw(frame: &mut Frame, app: &App) {
    let theme = &app.theme;
    let area = frame.area();
    frame.render_widget(Block::default().style(theme.base()), area);
    if area.width < 30 || area.height < 10 {
        frame.render_widget(
            Paragraph::new(
                "platter · needs 30×10
Resize terminal; q quits",
            ),
            area,
        );
        return;
    }
    let [header, body, status] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Min(3),
        Constraint::Length(2),
    ])
    .areas(area);
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(" platter", Style::default().fg(theme.accent)),
            Span::styled(
                "  Disks / partitions / shares",
                Style::default().fg(theme.muted),
            ),
        ])),
        header,
    );
    if app.help {
        draw_help(frame, theme, body);
    } else if app.picking_theme {
        draw_themes(frame, app, theme, body);
    } else {
        draw_main(frame, app, theme, body);
    }
    if !matches!(app.dialog, Dialog::None) {
        draw_dialog(frame, app, theme, area);
    }
    draw_status(frame, app, theme, status);
    app.commands.draw(
        frame,
        &app.command_actions(),
        app.theme.bg,
        app.theme.fg,
        app.theme.accent,
        app.theme.bg_raised,
    );
}

fn draw_main(frame: &mut Frame, app: &App, theme: &Theme, area: Rect) {
    let compact = area.width < 70;
    let [list_area, detail_area] = if compact {
        if app.focus == Focus::Devices {
            [area, Rect::default()]
        } else {
            [Rect::default(), area]
        }
    } else {
        Layout::horizontal([
            Constraint::Length(if area.width >= 100 { 30 } else { 24 }),
            Constraint::Min(24),
        ])
        .areas(area)
    };
    draw_devices(frame, app, theme, list_area);
    let [top, actions] = Layout::vertical([
        Constraint::Min(8),
        Constraint::Length(if area.height >= 28 { 12 } else { 7 }),
    ])
    .areas(detail_area);
    if matches!(app.current_row(), Some(RowKind::Shares)) || app.focus == Focus::Shares {
        draw_shares(frame, app, theme, top);
    } else {
        draw_detail(frame, app, theme, top);
    }
    draw_actions(frame, app, theme, actions);
}

fn draw_devices(frame: &mut Frame, app: &App, theme: &Theme, area: Rect) {
    let block = panel("Devices", app.focus == Focus::Devices, theme);
    let items: Vec<ListItem> = app
        .rows
        .iter()
        .map(|row| {
            let indent = " ".repeat(row.depth as usize * 2);
            let mark = if row.mounted { "●" } else { "○" };
            let size = if row.size.is_empty() {
                String::new()
            } else {
                format!("  {}", row.size)
            };
            let style = if row.mounted {
                Style::default().fg(theme.green)
            } else {
                Style::default().fg(theme.fg)
            };
            ListItem::new(Line::from(vec![
                Span::styled(format!("{indent}{mark} "), style),
                Span::raw(row.label.clone()),
                Span::styled(size, Style::default().fg(theme.fg_dim)),
            ]))
        })
        .collect();
    let list = List::new(items)
        .block(block)
        .highlight_style(theme.selected());
    let mut state = ListState::default();
    state.select(Some(app.selected));
    frame.render_stateful_widget(list, area, &mut state);
}

fn draw_detail(frame: &mut Frame, app: &App, theme: &Theme, area: Rect) {
    let title = detail_title(app);
    let block = panel(&title, false, theme);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let [bar_area, text_area] =
        Layout::vertical([Constraint::Length(3), Constraint::Min(1)]).areas(inner);
    let usage = if let Some(RowKind::Volume { block }) = app.current_row() {
        app.model
            .block(&block)
            .and_then(|b| b.mount_points.first())
            .and_then(|path| model::fs_usage(path))
    } else {
        None
    };
    if let Some((used, total)) = usage {
        let ratio = (used as f64 / total.max(1) as f64).clamp(0.0, 1.0);
        frame.render_widget(
            Gauge::default()
                .ratio(ratio)
                .gauge_style(Style::default().fg(theme.cyan).bg(theme.bg_raised))
                .label(format!("{} used · {:.0}%", human_size(used), ratio * 100.0)),
            bar_area,
        );
    } else {
        draw_bar(frame, app, theme, bar_area);
    }
    let lines = detail_lines(app, theme);
    let max_scroll = lines
        .iter()
        .map(|line| {
            line.width()
                .div_ceil(text_area.width.max(1) as usize)
                .max(1)
        })
        .sum::<usize>()
        .saturating_sub(text_area.height as usize) as u16;
    let paragraph = Paragraph::new(lines).wrap(Wrap { trim: false });
    let paragraph = paragraph.scroll((app.detail_scroll.min(max_scroll), 0));
    frame.render_widget(paragraph, text_area);
}

fn draw_bar(frame: &mut Frame, app: &App, theme: &Theme, area: Rect) {
    let Some(disk) = bar_disk(app) else { return };
    let segments = app.model.segments_for(&disk);
    if segments.is_empty() || area.width == 0 {
        return;
    }
    let total: u64 = segments.iter().map(|s| s.size).sum::<u64>().max(1);
    let width = area.width as u64;
    let mut spans = Vec::new();
    let mut used = 0u16;
    for (index, segment) in segments.iter().enumerate() {
        let mut cells = ((segment.size.saturating_mul(width)) / total) as u16;
        if cells == 0 {
            cells = 1;
        }
        if index + 1 == segments.len() {
            cells = area.width.saturating_sub(used);
        }
        used = used.saturating_add(cells);
        let selected = segment_selected(app, segment);
        let bg = if segment.free {
            theme.bg_raised
        } else {
            theme.segment(index)
        };
        let fg = if segment.free {
            theme.muted
        } else {
            theme::on_color(bg)
        };
        let mut style = Style::default().bg(bg).fg(fg);
        if selected {
            style = style.add_modifier(Modifier::BOLD | Modifier::REVERSED);
        }
        let label = if cells > 4 {
            segment
                .label
                .chars()
                .take(cells as usize)
                .collect::<String>()
        } else {
            String::new()
        };
        let mut text = format!("{label: <width$}", width = cells as usize);
        if text.chars().count() > cells as usize {
            text = text.chars().take(cells as usize).collect();
        }
        spans.push(Span::styled(text, style));
    }
    frame.render_widget(Paragraph::new(Line::from(spans)), area);
}

fn draw_actions(frame: &mut Frame, app: &App, theme: &Theme, area: Rect) {
    let block = panel("Actions", app.focus == Focus::Actions, theme);
    let actions = app.actions();
    let items: Vec<ListItem> = actions
        .iter()
        .map(|action| {
            let key = action.id.key();
            let key = if key.is_empty() {
                " ".into()
            } else {
                key.to_string()
            };
            let style = if action.enabled {
                Style::default().fg(theme.fg)
            } else {
                theme.dimmed()
            };
            let reason = if action.enabled {
                String::new()
            } else {
                format!("  {}", action.reason)
            };
            ListItem::new(Line::from(vec![
                Span::styled(format!("{key:>2}  "), Style::default().fg(theme.accent)),
                Span::styled(action.id.label(), style),
                Span::styled(reason, Style::default().fg(theme.yellow)),
            ]))
        })
        .collect();
    let list = List::new(items)
        .block(block)
        .highlight_style(theme.selected());
    let mut state = ListState::default();
    if app.focus == Focus::Actions {
        state.select(Some(app.action_index));
    }
    frame.render_stateful_widget(list, area, &mut state);
}

fn draw_shares(frame: &mut Frame, app: &App, theme: &Theme, area: Rect) {
    let editing = app.focus == Focus::Shares;
    let block = panel("Network shares", editing, theme);
    let mut lines = Vec::new();
    if app.shares.is_empty() {
        lines.push(Line::from(Span::styled(
            "No network shares configured in /etc/fstab.",
            Style::default().fg(theme.fg),
        )));
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "Quick Start Guide:",
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
        )));
        lines.push(Line::from(
            "  1. Press 'n' (or select 'New share' in the Actions pane).",
        ));
        lines.push(Line::from("  2. Type your server address (e.g. \\\\192.168.1.100\\photos or //synology.local/data)."));
        lines.push(Line::from(
            "  3. Press Enter. The mount folder (/mnt/photos) will be created automatically.",
        ));
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "Works with Windows PC/laptop shares, Synology/QNAP/TrueNAS, Linux NFS, and SSHFS.",
            Style::default().fg(theme.muted),
        )));
    }
    for (index, share) in app.shares.iter().enumerate() {
        let is_selected = index == app.share_index;
        let mark = if share.mounted { "● " } else { "○ " };
        let status_text = if share.mounted {
            "Connected"
        } else {
            "Disconnected"
        };
        let status_color = if share.mounted {
            theme.green
        } else {
            theme.muted
        };

        let proto_badge = match share.fstype.as_str() {
            "cifs" | "smb" => "SMB",
            "nfs" | "nfs4" => "NFS",
            "fuse.sshfs" | "sshfs" => "SSHFS",
            other => other,
        };

        let style = if is_selected && editing {
            theme.selected()
        } else if is_selected {
            Style::default().fg(theme.fg).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(theme.fg)
        };

        lines.push(Line::from(vec![
            Span::styled(mark, Style::default().fg(status_color)),
            Span::styled(
                format!("[{proto_badge}] "),
                Style::default().fg(theme.accent),
            ),
            Span::styled(format!("{} ", share.source), style),
            Span::styled("→ ", Style::default().fg(theme.muted)),
            Span::styled(format!("{} ", share.target), Style::default().fg(theme.fg)),
            Span::styled(
                format!("({status_text})"),
                Style::default().fg(status_color),
            ),
        ]));
        lines.push(Line::from(vec![
            Span::styled("     Status: ", Style::default().fg(theme.muted)),
            Span::styled(
                if share.mounted {
                    "Connected (files ready in folder)"
                } else {
                    "Disconnected (press Enter to connect)"
                },
                Style::default().fg(if share.mounted {
                    theme.green
                } else {
                    theme.muted
                }),
            ),
            if !share.options.is_empty() {
                Span::styled(
                    format!("  |  opts: {}", share.options),
                    Style::default().fg(theme.muted),
                )
            } else {
                Span::raw("")
            },
        ]));
    }
    if editing && !app.shares.is_empty() {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "Enter connect/disconnect   e edit   d delete   n new share   Esc back to disks",
            Style::default().fg(theme.accent),
        )));
    }
    frame.render_widget(
        Paragraph::new(lines)
            .block(block)
            .wrap(Wrap { trim: false }),
        area,
    );
}

fn draw_dialog(frame: &mut Frame, app: &App, theme: &Theme, area: Rect) {
    match &app.dialog {
        Dialog::Form(form) => {
            let rect = centered(area, 84, 26);
            frame.render_widget(Clear, rect);
            let block = panel(&form.title, true, theme);
            let inner = block.inner(rect);
            frame.render_widget(block, rect);

            let has_guide = inner.height >= 12;
            let (body, guide_opt, hint) = if has_guide {
                let [b, g, h] = Layout::vertical([
                    Constraint::Min(6),
                    Constraint::Length(4),
                    Constraint::Length(1),
                ])
                .areas(inner);
                (b, Some(g), h)
            } else {
                let [b, h] =
                    Layout::vertical([Constraint::Min(4), Constraint::Length(1)]).areas(inner);
                (b, None, h)
            };

            let mut lines = Vec::new();
            if !form.blurb.is_empty() {
                lines.push(Line::from(Span::styled(
                    form.blurb.clone(),
                    Style::default().fg(theme.muted),
                )));
                lines.push(Line::from(""));
            }
            let width = body.width as usize;
            for (index, field) in form.fields.iter().enumerate() {
                lines.push(field_line(field, index == form.focus, width, theme));
            }
            let prefix = if form.blurb.is_empty() { 0 } else { 2 };
            let row = prefix + form.focus;
            let height = body.height as usize;
            let scroll = row.saturating_sub(height.saturating_sub(1)) as u16;
            frame.render_widget(Paragraph::new(lines).scroll((scroll, 0)), body);

            if let Some(guide_area) = guide_opt {
                if let Some(field) = form.fields.get(form.focus) {
                    let req_span = if field.required {
                        Span::styled(
                            "[MANDATORY] ",
                            Style::default()
                                .fg(theme.yellow)
                                .add_modifier(Modifier::BOLD),
                        )
                    } else {
                        Span::styled("[OPTIONAL] ", Style::default().fg(theme.muted))
                    };
                    let guide_block = Block::default()
                        .title(format!(" Help & Example: {} ", field.label))
                        .borders(Borders::ALL)
                        .border_style(Style::default().fg(theme.accent))
                        .style(theme.base());
                    let mut guide_lines = Vec::new();
                    if !field.hint.is_empty() {
                        guide_lines.push(Line::from(vec![
                            req_span,
                            Span::styled(field.hint.clone(), Style::default().fg(theme.fg)),
                        ]));
                    } else if field.required {
                        guide_lines.push(Line::from(vec![
                            req_span,
                            Span::styled("This field is required.", Style::default().fg(theme.fg)),
                        ]));
                    } else {
                        guide_lines.push(Line::from(vec![
                            req_span,
                            Span::styled(
                                "This field is optional.",
                                Style::default().fg(theme.muted),
                            ),
                        ]));
                    }
                    if !field.placeholder.is_empty() {
                        guide_lines.push(Line::from(vec![
                            Span::styled("Example: ", Style::default().fg(theme.accent)),
                            Span::styled(
                                field.placeholder.clone(),
                                Style::default().fg(theme.fg_dim),
                            ),
                        ]));
                    }
                    frame.render_widget(
                        Paragraph::new(guide_lines)
                            .block(guide_block)
                            .wrap(Wrap { trim: true }),
                        guide_area,
                    );
                }
            }

            frame.render_widget(
                Paragraph::new(Span::styled(
                    "Enter confirms/saves   Esc cancels   Tab/↑↓ navigate   ← → edit/toggle",
                    Style::default().fg(theme.muted),
                )),
                hint,
            );
        }
        Dialog::Confirm(confirm) => {
            let rect = centered(area, 64, 14);
            frame.render_widget(Clear, rect);
            let block = panel(&confirm.title, true, theme);
            let inner = block.inner(rect);
            frame.render_widget(block, rect);
            let mut lines = vec![
                Line::from(confirm.body.clone()),
                Line::from(""),
                Line::from(Span::styled(
                    format!("Type {} to confirm", confirm.kernel),
                    Style::default().fg(theme.yellow),
                )),
                typed_line("Device", &confirm.typed, confirm.which == 0, theme),
            ];
            if confirm.system {
                lines.push(Line::from("This is a system disk. Also type system."));
                lines.push(typed_line(
                    "Confirm",
                    &confirm.ack,
                    confirm.which == 1,
                    theme,
                ));
            }
            frame.render_widget(Paragraph::new(lines), inner);
        }
        Dialog::Text {
            title,
            body,
            scroll,
        } => {
            let rect = centered(area, 96, 34);
            frame.render_widget(Clear, rect);
            let block = panel(title, true, theme);
            frame.render_widget(
                Paragraph::new(body.as_str())
                    .block(block)
                    .scroll((*scroll, 0))
                    .wrap(Wrap { trim: false }),
                rect,
            );
        }
        Dialog::None => {}
    }
}

fn draw_themes(frame: &mut Frame, app: &App, theme: &Theme, area: Rect) {
    let block = panel("Theme", true, theme);
    let items: Vec<ListItem> = theme::CHOICES
        .iter()
        .enumerate()
        .map(|(index, choice)| {
            let mark = if choice.id == app.config.theme {
                "● "
            } else {
                "  "
            };
            let preview = app.preview_name(choice.id);
            ListItem::new(Line::from(vec![
                Span::styled(mark, Style::default().fg(theme.accent)),
                Span::raw(choice.label),
                Span::styled(format!("  {preview}"), Style::default().fg(theme.muted)),
            ]))
            .style(if index == app.theme_index {
                theme.selected()
            } else {
                theme.base()
            })
        })
        .collect();
    let mut state = ListState::default();
    state.select(Some(app.theme_index));
    frame.render_stateful_widget(
        List::new(items)
            .block(block)
            .highlight_style(theme.selected()),
        area,
        &mut state,
    );
}

fn draw_help(frame: &mut Frame, theme: &Theme, area: Rect) {
    let text = "\
Devices          j/k or arrows move. Enter mounts, unmounts, or unlocks
Actions          Tab, then Enter. The letter at the left runs that action
m mount/unlock   u unmount   U force unmount   e mount options
n new partition  f format    d delete          r resize    l label
s SMART data     c self-test   b benchmark     i create image
o restore image  p power off   g drive settings  a attach image
t theme          x cancel job  2 network shares  ? help
q quit           1 first disk

Shares           The actions pane lists the commands, same as a disk.
                 Tab moves between disks, the share list, and actions.
                 Esc or ← returns to the disks. ↑ on the first share does too.
Destructive work asks you to type the kernel name. A system disk also asks for the word system.
Network shares are written to /etc/fstab with sudo. Every other line is copied through unchanged.
Esc closes a dialog. Esc during a benchmark or image copy cancels it.
a aborts a SMART self-test while the SMART page is open.
";
    frame.render_widget(
        Paragraph::new(text)
            .block(panel("Keys", true, theme))
            .wrap(Wrap { trim: false }),
        area,
    );
}

fn draw_status(frame: &mut Frame, app: &App, theme: &Theme, area: Rect) {
    let [keys, status] =
        Layout::vertical([Constraint::Length(1), Constraint::Length(1)]).areas(area);
    frame.render_widget(
        Paragraph::new(" Tab pane  PgUp/Dn details  ^K actions  ? help  q quit")
            .style(theme.status()),
        keys,
    );
    let area = status;
    let (text, style) = if !app.error.is_empty() {
        (
            app.error.clone(),
            Style::default().fg(theme.red).bg(theme.bg_deep),
        )
    } else if let Some(progress) = &app.progress {
        let label = if progress.cancel {
            format!("{}    Esc cancels", progress.label)
        } else {
            progress.label.clone()
        };
        let line = progress_line(&label, progress.ratio, app.busy_tick, area.width, theme);
        frame.render_widget(Paragraph::new(line), area);
        return;
    } else if let Some(job) = app.model.jobs.first() {
        let ratio = if job.progress_valid {
            job.progress
        } else {
            -1.0
        };
        let label = if job.cancelable {
            format!("{}    x cancel", job.operation)
        } else {
            job.operation.clone()
        };
        let line = progress_line(&label, ratio, app.busy_tick, area.width, theme);
        frame.render_widget(Paragraph::new(line), area);
        return;
    } else if !app.info.is_empty() {
        (app.info.clone(), theme.status())
    } else if !app.samples.is_empty() {
        (
            sparkline(&app.samples, area.width.saturating_sub(2) as usize),
            theme.status(),
        )
    } else {
        (
            if app.focus == Focus::Shares {
                "↑↓ shares    Tab actions    Esc disks".into()
            } else if matches!(app.current_row(), Some(RowKind::Shares))
                && app.focus == Focus::Actions
            {
                "letter runs the action    Tab share list    Esc disks".into()
            } else if matches!(app.current_row(), Some(RowKind::Shares)) {
                "↑↓ disks    Tab shares    ? help    q quit".into()
            } else {
                "? help   q quit   t theme   2 shares".into()
            },
            theme.status(),
        )
    };
    frame.render_widget(Paragraph::new(text).style(style), area);
}

fn progress_line(label: &str, ratio: f64, tick: u64, width: u16, theme: &Theme) -> Line<'static> {
    let label_width = label.chars().count() as u16 + 2;
    let bar_w = width.saturating_sub(label_width + 6).clamp(8, 24);
    let mut spans = vec![Span::styled(
        format!("{label}  "),
        Style::default().fg(theme.fg).bg(theme.bg_deep),
    )];
    let filled = if ratio < 0.0 {
        None
    } else {
        Some(((ratio.clamp(0.0, 1.0) * f64::from(bar_w)).round() as u16).min(bar_w))
    };
    let window = (tick as u16) % bar_w.max(1);
    for i in 0..bar_w {
        let on = match filled {
            Some(n) => i < n,
            None => {
                let end = window + 4;
                (i >= window && i < end) || (end > bar_w && i < end % bar_w)
            }
        };
        spans.push(Span::styled(
            "█",
            Style::default()
                .fg(if on { theme.accent } else { theme.bg_raised })
                .bg(theme.bg_deep),
        ));
    }
    if ratio >= 0.0 {
        spans.push(Span::styled(
            format!(" {:3.0}%", ratio * 100.0),
            Style::default().fg(theme.accent).bg(theme.bg_deep),
        ));
    }
    Line::from(spans)
}

fn detail_title(app: &App) -> String {
    match app.current_row() {
        Some(RowKind::Drive { drive }) => app
            .model
            .drive(&drive)
            .map(|d| {
                format!("{} {}", d.vendor, d.model)
                    .split_whitespace()
                    .collect::<Vec<_>>()
                    .join(" ")
            })
            .unwrap_or_else(|| "Drive".into()),
        Some(RowKind::Volume { block }) => app
            .model
            .block(&block)
            .map(|b| kernel_name(b.device_path()).to_string())
            .unwrap_or_else(|| "Volume".into()),
        Some(RowKind::Free { size, .. }) => format!("Free {}", human_size(size)),
        Some(RowKind::Raid { raid }) => app
            .model
            .raid(&raid)
            .map(|r| format!("{} {}", r.level, r.name))
            .unwrap_or_else(|| "RAID".into()),
        _ => "Disks".into(),
    }
}

fn detail_lines(app: &App, theme: &Theme) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    let push = |lines: &mut Vec<Line>, k: &str, v: String| {
        if v.is_empty() {
            return;
        }
        lines.push(Line::from(vec![
            Span::styled(format!("{k:<14}"), Style::default().fg(theme.muted)),
            Span::raw(v),
        ]));
    };
    match app.current_row() {
        Some(RowKind::Drive { drive }) => {
            if let Some(drive) = app.model.drive(&drive) {
                push(&mut lines, "Size", human_size(drive.size));
                push(&mut lines, "Serial", drive.serial.clone());
                push(&mut lines, "Revision", drive.revision.clone());
                push(&mut lines, "Bus", drive.connection.clone());
                push(&mut lines, "Media", drive.media.clone());
                if drive.rotation > 0 {
                    push(&mut lines, "Rotation", format!("{} rpm", drive.rotation));
                } else if drive.rotation == 0 {
                    push(&mut lines, "Rotation", "solid state".into());
                }
                if let Some(disk) = app.model.disk_for_drive(&drive.path) {
                    push(&mut lines, "Device", disk.device_path().to_string());
                    if let Some(table) = &disk.table {
                        push(&mut lines, "Table", table.kind.clone());
                    }
                }
                if let Some(ata) = &drive.ata {
                    push(
                        &mut lines,
                        "SMART",
                        if !ata.smart_supported {
                            "not supported".into()
                        } else if ata.smart_failing {
                            "FAILING".into()
                        } else if ata.smart_enabled {
                            "enabled".into()
                        } else {
                            "disabled".into()
                        },
                    );
                }
                if drive.removable {
                    push(&mut lines, "Removable", "yes".into());
                }
                if drive.optical {
                    push(&mut lines, "Optical", "yes".into());
                }
                if drive.nvme {
                    push(&mut lines, "NVMe", "controller".into());
                    if let Some(temp) = drive.nvme_temp {
                        push(&mut lines, "Temperature", format!("{temp}°C"));
                    }
                    if let Some(hours) = drive.nvme_hours {
                        let days = hours / 24;
                        push(
                            &mut lines,
                            "Power-on",
                            format!("{hours} hours ({days} days)"),
                        );
                    }
                    if let Some(selftest) = &drive.nvme_selftest {
                        push(&mut lines, "Self-test", selftest.clone());
                    }
                    if let Some(sanitize) = &drive.nvme_sanitize {
                        push(&mut lines, "Sanitize", sanitize.clone());
                    }
                }
                if let Some(ata) = &drive.ata {
                    if ata.security_frozen {
                        push(&mut lines, "Security", "frozen".into());
                    }
                    if ata.secure_erase_enhanced_minutes > 0 {
                        push(
                            &mut lines,
                            "Enhanced erase",
                            format!("{} min", ata.secure_erase_enhanced_minutes),
                        );
                    }
                    let mut features = Vec::new();
                    if ata.apm_supported {
                        features.push("APM");
                    }
                    if ata.write_cache_supported {
                        features.push("write cache");
                    }
                    if ata.lookahead_supported {
                        features.push("look-ahead");
                    }
                    if !features.is_empty() {
                        push(&mut lines, "Features", features.join(", "));
                    }
                }
                if app
                    .model
                    .disk_for_drive(&drive.path)
                    .is_some_and(|d| app.model.is_system_tree(&d.path))
                {
                    push(&mut lines, "System", "yes".into());
                }
            }
        }
        Some(RowKind::Volume { block }) => {
            if let Some(block) = app.model.block(&block) {
                volume_lines(&mut lines, app, block, &push);
            }
        }
        Some(RowKind::Free { disk, start, size }) => {
            push(
                &mut lines,
                "Disk",
                app.model
                    .block(&disk)
                    .map(|b| kernel_name(b.device_path()).to_string())
                    .unwrap_or_default(),
            );
            push(&mut lines, "Start", human_size(start));
            push(&mut lines, "Size", human_size(size));
            push(&mut lines, "", "Press n to create a partition here.".into());
        }
        Some(RowKind::Raid { raid }) => {
            if let Some(raid) = app.model.raid(&raid) {
                push(&mut lines, "Level", raid.level.clone());
                push(&mut lines, "Name", raid.name.clone());
                push(&mut lines, "UUID", raid.uuid.clone());
                push(&mut lines, "Size", human_size(raid.size));
                push(
                    &mut lines,
                    "State",
                    if raid.running {
                        "running".into()
                    } else {
                        "stopped".into()
                    },
                );
                if raid.degraded {
                    push(&mut lines, "Degraded", "yes".into());
                }
                if !raid.sync_action.is_empty() {
                    push(
                        &mut lines,
                        "Sync",
                        format!("{} {:.0}%", raid.sync_action, raid.sync_completed * 100.0),
                    );
                }
                push(&mut lines, "Members", raid.members.len().to_string());
            }
        }
        _ => {}
    }
    if !app.samples.is_empty() {
        lines.push(Line::from(""));
        lines.push(Line::from(sparkline(&app.samples, 48)));
    }
    lines
}

fn volume_lines(
    lines: &mut Vec<Line<'static>>,
    app: &App,
    block: &model::Block,
    push: &impl Fn(&mut Vec<Line>, &str, String),
) {
    push(lines, "Device", block.device_path().to_string());
    push(lines, "Size", human_size(block.size));
    push(lines, "Usage", block.id_usage.clone());
    push(lines, "Type", block.id_type.clone());
    push(lines, "Label", block.id_label.clone());
    push(lines, "UUID", block.id_uuid.clone());
    if let Some(part) = &block.partition {
        push(
            lines,
            "Partition",
            format!(
                "{}  {}",
                part.number,
                model::partition_type_label(&part.type_code)
            ),
        );
        push(lines, "Part name", part.name.clone());
        push(lines, "Part UUID", part.uuid.clone());
        push(lines, "Flags", format!("0x{:x}", part.flags));
        if part.container {
            push(lines, "Layout", "extended container".into());
        }
        if part.contained {
            push(lines, "Layout", "logical partition".into());
        }
    }
    if block.encrypted {
        push(
            lines,
            "Encryption",
            if block.hint_encryption.is_empty() {
                "LUKS".into()
            } else {
                block.hint_encryption.clone()
            },
        );
    }
    if !block.mount_points.is_empty() {
        push(lines, "Mounted", block.mount_points.join(", "));
        if let Some(target) = block.mount_points.first() {
            if let Some((used, total)) = model::fs_usage(target) {
                let pct = if total > 0 {
                    (used as f64 / total as f64 * 100.0).round() as u64
                } else {
                    0
                };
                let free = total.saturating_sub(used);
                push(
                    lines,
                    "Space",
                    format!(
                        "{} / {} used ({}%) — {} free",
                        human_size(used),
                        human_size(total),
                        pct,
                        human_size(free)
                    ),
                );
            }
        }
    }
    if block.swap {
        push(
            lines,
            "Swap",
            if block.swap_active {
                "active".into()
            } else {
                "inactive".into()
            },
        );
    }
    if block.loopback {
        push(lines, "Backing", block.backing_file.clone());
        push(
            lines,
            "Autoclear",
            if block.loop_autoclear {
                "yes".into()
            } else {
                "no".into()
            },
        );
    }
    if block.mdraid_member != "/" && !block.mdraid_member.is_empty() {
        push(lines, "RAID member", "yes".into());
    }
    if let Some(item) = block.fstab_items().into_iter().next() {
        push(lines, "fstab", model::fstab_field(item, "dir"));
        push(lines, "Options", model::fstab_field(item, "opts"));
    }
    if app.model.is_system_block(block) {
        push(lines, "System", "yes".into());
    }
}

fn field_line(field: &Field, focused: bool, width: usize, theme: &Theme) -> Line<'static> {
    let mut spans = Vec::new();

    let label_style = if focused {
        theme.selected().add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(theme.fg)
    };
    let marker_style = if focused {
        theme.selected().add_modifier(Modifier::BOLD)
    } else if field.required {
        Style::default()
            .fg(theme.yellow)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(theme.muted)
    };

    let label_str = format!("{:<19}", field.label);
    spans.push(Span::styled(label_str, label_style));
    if field.required {
        spans.push(Span::styled("*  ", marker_style));
    } else {
        spans.push(Span::styled("   ", marker_style));
    }

    let is_empty = field.value.is_empty();
    if field.toggle {
        let toggle_str = if field.value == "yes" {
            "[x] Yes"
        } else {
            "[ ] No "
        };
        let style = if focused {
            theme.selected()
        } else if field.value == "yes" {
            Style::default().fg(theme.green)
        } else {
            Style::default().fg(theme.muted)
        };
        spans.push(Span::styled(toggle_str, style));
    } else if !field.choices.is_empty() {
        let choice_str = format!("< {} >", field.value);
        let style = if focused {
            theme.selected()
        } else {
            Style::default().fg(theme.accent)
        };
        spans.push(Span::styled(choice_str, style));
    } else if is_empty {
        if focused {
            spans.push(Span::styled("█", theme.selected()));
            if !field.placeholder.is_empty() {
                spans.push(Span::styled(
                    format!("  (e.g. {})", field.placeholder),
                    Style::default().fg(theme.muted),
                ));
            }
        } else if !field.placeholder.is_empty() {
            spans.push(Span::styled(
                format!("e.g. {}", field.placeholder),
                Style::default().fg(theme.muted),
            ));
        }
    } else {
        let mut display_val = if field.secret {
            "•".repeat(field.value.chars().count())
        } else {
            field.value.clone()
        };

        if focused {
            let chars: Vec<char> = display_val.chars().collect();
            let cursor_idx = field.cursor.min(chars.len());
            let mut with_cursor = String::new();
            for (i, c) in chars.iter().enumerate() {
                if i == cursor_idx {
                    with_cursor.push('▏');
                }
                with_cursor.push(*c);
            }
            if cursor_idx >= chars.len() {
                with_cursor.push('█');
            }
            display_val = with_cursor;
            spans.push(Span::styled(display_val, theme.selected()));
        } else {
            spans.push(Span::styled(display_val, Style::default().fg(theme.fg)));
        }
    }

    if focused {
        let total_chars: usize = spans.iter().map(|s| s.content.chars().count()).sum();
        if total_chars < width {
            spans.push(Span::styled(
                " ".repeat(width - total_chars),
                theme.selected(),
            ));
        }
    }

    Line::from(spans)
}

fn typed_line(label: &str, value: &str, focused: bool, theme: &Theme) -> Line<'static> {
    let style = if focused {
        theme.selected()
    } else {
        Style::default().fg(theme.fg)
    };
    Line::from(vec![
        Span::styled(format!("{label:<10}"), Style::default().fg(theme.muted)),
        Span::styled(value.to_string(), style),
    ])
}

fn sparkline(samples: &[(f64, f64)], width: usize) -> String {
    const GLYPHS: [&str; 8] = ["▁", "▂", "▃", "▄", "▅", "▆", "▇", "█"];
    if width == 0 || samples.is_empty() {
        return String::new();
    }
    let max = samples.iter().fold(0.0_f64, |m, s| m.max(s.1)).max(1.0);
    let mut cols = vec![0.0f64; width];
    let mut seen = vec![false; width];
    for (pos, mbps) in samples {
        let index = ((*pos).clamp(0.0, 0.999) * width as f64) as usize;
        cols[index] = cols[index].max(*mbps);
        seen[index] = true;
    }
    cols.iter()
        .zip(seen)
        .map(|(value, seen)| {
            if !seen {
                " "
            } else {
                let slot = ((*value / max) * (GLYPHS.len() as f64 - 1.0)).round() as usize;
                GLYPHS[slot.min(GLYPHS.len() - 1)]
            }
        })
        .collect()
}

fn panel<'a>(title: &'a str, focused: bool, theme: &Theme) -> Block<'a> {
    let border = if focused { theme.accent } else { theme.border };
    Block::default()
        .title(format!(" {title} "))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border))
        .style(theme.base())
}

fn centered(area: Rect, width: u16, height: u16) -> Rect {
    let width = width.min(area.width.saturating_sub(2)).max(20);
    let height = height.min(area.height.saturating_sub(2)).max(8);
    let x = area.x + (area.width.saturating_sub(width)) / 2;
    let y = area.y + (area.height.saturating_sub(height)) / 2;
    Rect::new(x, y, width, height)
}

fn bar_disk(app: &App) -> Option<String> {
    match app.current_row()? {
        RowKind::Drive { drive } => app.model.disk_for_drive(&drive).map(|b| b.path.clone()),
        RowKind::Volume { block } => {
            let block = app.model.block(&block)?;
            if let Some(part) = &block.partition {
                Some(part.table.clone())
            } else {
                Some(block.path.clone())
            }
        }
        RowKind::Free { disk, .. } => Some(disk),
        _ => None,
    }
}

fn segment_selected(app: &App, segment: &model::Segment) -> bool {
    match app.current_row() {
        Some(RowKind::Volume { block }) => segment.block.as_deref() == Some(block.as_str()),
        Some(RowKind::Free { start, .. }) => segment.free && segment.start == start,
        Some(RowKind::Drive { .. }) => false,
        _ => false,
    }
}

impl App {
    pub fn current_row(&self) -> Option<RowKind> {
        self.rows.get(self.selected).map(|row| row.kind.clone())
    }

    pub fn preview_name(&self, id: &str) -> String {
        theme::resolve(id, self.omarchy.as_ref(), self.custom.as_ref()).name
    }
}

#[cfg(test)]
mod aurora_audit {
    use super::*;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use ratatui::{backend::TestBackend, Terminal};
    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }
    fn commands() -> KeyEvent {
        KeyEvent::new(KeyCode::Char('k'), KeyModifiers::CONTROL)
    }
    use crate::model::{Block, MIB};
    fn sample_block(path: &str, device: &str) -> Block {
        Block {
            path: path.into(),
            device: device.into(),
            preferred: device.into(),
            id_usage: String::new(),
            id_type: "ext4".into(),
            id_label: String::new(),
            id_uuid: String::new(),
            size: 100 * MIB,
            read_only: false,
            hint_system: false,
            hint_ignore: false,
            drive: "/drives/d".into(),
            crypto_backing: "/".into(),
            mdraid: "/".into(),
            mdraid_member: "/".into(),
            symlinks: Vec::new(),
            partition: None,
            table: None,
            filesystem: true,
            mount_points: Vec::new(),
            swap: false,
            swap_active: false,
            encrypted: false,
            cleartext: "/".into(),
            hint_encryption: String::new(),
            loopback: false,
            backing_file: String::new(),
            loop_autoclear: false,
            config: Vec::new(),
        }
    }

    fn fixture(path: std::path::PathBuf) -> App {
        let mut app = App::new(
            crate::config::Config::default(),
            path,
            crate::theme::aurora(),
            None,
            None,
            None,
        );
        let mut block = sample_block(
            "/org/freedesktop/UDisks2/block_devices/nvme0n1p2",
            "/dev/nvme0n1p2",
        );
        block.id_label = "System".into();
        block.id_type = "ext4".into();
        block.size = 950_000_000_000;
        block.mount_points = vec!["/".into()];
        block.filesystem = true;
        app.model.blocks.push(block);
        app.rows = app.model.rows();
        app.info.clear();
        app
    }
    #[test]
    fn forms_own_commands_and_image_copy_does_not_execute() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = fixture(dir.path().join("config.toml"));
        app.model.blocks[0].mount_points.clear();
        app.on_key(key(KeyCode::Char('i')));
        assert!(matches!(app.dialog, Dialog::Form(_)));
        app.on_key(commands());
        assert!(!app.commands.open);
        app.on_key(key(KeyCode::Esc));
        app.on_key(commands());
        assert!(app.commands.open);
        assert!(matches!(app.pending, crate::app::Effect::None));
    }
    #[test]
    fn responsive_render_matrix() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = fixture(dir.path().join("config.toml"));
        // One terminal exercises shrink/grow transitions, rather than independent initial frames.
        let mut resizing = Terminal::new(TestBackend::new(120, 40)).unwrap();
        for &(w, h) in &[
            (120, 40),
            (60, 20),
            (80, 24),
            (30, 10),
            (100, 30),
            (120, 40),
        ] {
            resizing.backend_mut().resize(w, h);
            resizing.draw(|f| draw(f, &app)).unwrap();
            assert_eq!(resizing.backend().buffer().area.width, w);
        }
        for &(w, h) in crate::audit::SIZES {
            let mut term = Terminal::new(TestBackend::new(w, h)).unwrap();
            term.draw(|f| draw(f, &app)).unwrap();
            let buffer = term.backend().buffer();
            if w >= 30 && h >= 10 {
                assert!(crate::audit::text(buffer).contains("platter"));
            }
            crate::audit::capture("platter", "main", buffer);
        }
        app.focus = Focus::Actions;
        for &(w, h) in &crate::audit::SIZES[..4] {
            let mut term = Terminal::new(TestBackend::new(w, h)).unwrap();
            term.draw(|f| draw(f, &app)).unwrap();
            crate::audit::capture("platter", "focused", term.backend().buffer());
        }
        app.on_key(commands());
        for &(w, h) in &crate::audit::SIZES[..4] {
            let mut term = Terminal::new(TestBackend::new(w, h)).unwrap();
            term.draw(|f| draw(f, &app)).unwrap();
            let buffer = term.backend().buffer();
            assert!(crate::audit::text(buffer).contains("Actions"));
            crate::audit::capture("platter", "actions", buffer);
        }
        app.commands.open = false;
        app.model = Default::default();
        app.rows = app.model.rows();
        app.focus = Focus::Devices;
        for &(w, h) in &crate::audit::SIZES[..4] {
            let mut term = Terminal::new(TestBackend::new(w, h)).unwrap();
            term.draw(|f| draw(f, &app)).unwrap();
            crate::audit::capture("platter", "offline", term.backend().buffer());
        }
    }
}
