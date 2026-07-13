use super::*;
use termy_plugin_runtime::PluginInventory;

struct PluginSettingsSnapshot {
    plugins: Vec<InstalledPlugin>,
    inventory_errors: Vec<String>,
    bun_path: Option<PathBuf>,
    bun_error: Option<String>,
}

impl SettingsWindow {
    fn load_plugin_settings_snapshot(runtime: &PluginRuntime) -> PluginSettingsSnapshot {
        let (plugins, inventory_errors) = match runtime.installed_plugins() {
            Ok(PluginInventory { plugins, errors }) => (plugins, errors),
            Err(error) => (Vec::new(), vec![error]),
        };
        let (bun_path, bun_error) = match runtime.bun_path() {
            Ok(path) => (path, None),
            Err(error) => (None, Some(error)),
        };
        PluginSettingsSnapshot {
            plugins,
            inventory_errors,
            bun_path,
            bun_error,
        }
    }

    fn apply_plugin_settings_snapshot(&mut self, snapshot: PluginSettingsSnapshot) {
        self.installed_plugins = snapshot.plugins;
        self.plugin_inventory_errors = snapshot.inventory_errors;
        self.plugin_bun_path = snapshot.bun_path;
        self.plugin_bun_error = snapshot.bun_error;
        self.plugin_operation_in_flight = false;
    }

    fn refresh_plugin_settings(&mut self, cx: &mut Context<Self>) {
        if self.plugin_operation_in_flight {
            return;
        }
        self.plugin_operation_in_flight = true;
        cx.notify();
        let runtime = self.plugin_runtime.clone();
        cx.spawn(async move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let snapshot =
                smol::unblock(move || Self::load_plugin_settings_snapshot(&runtime)).await;
            let _ = cx.update(|cx| {
                this.update(cx, |view, cx| {
                    view.apply_plugin_settings_snapshot(snapshot);
                    cx.notify();
                })
            });
        })
        .detach();
    }

    fn install_plugin_from_folder(&mut self, cx: &mut Context<Self>) {
        if self.plugin_operation_in_flight {
            return;
        }
        self.plugin_operation_in_flight = true;
        cx.notify();
        let runtime = self.plugin_runtime.clone();
        cx.spawn(async move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let folder = rfd::AsyncFileDialog::new()
                .set_title("Install Termy Plugin")
                .pick_folder()
                .await;
            let Some(folder) = folder else {
                let _ = cx.update(|cx| {
                    this.update(cx, |view, cx| {
                        view.plugin_operation_in_flight = false;
                        cx.notify();
                    })
                });
                return;
            };

            let path = folder.path().to_path_buf();
            let folder_name = path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("this plugin");
            let message = format!(
                "Install \"{folder_name}\"? Plugins are trusted local code and run with your user permissions."
            );
            if !termy_native_sdk::confirm("Install Plugin", &message) {
                let _ = cx.update(|cx| {
                    this.update(cx, |view, cx| {
                        view.plugin_operation_in_flight = false;
                        cx.notify();
                    })
                });
                return;
            }

            let (result, snapshot) = smol::unblock(move || {
                let result = runtime.install_from_directory(&path);
                let snapshot = Self::load_plugin_settings_snapshot(&runtime);
                (result, snapshot)
            })
            .await;
            let _ = cx.update(|cx| {
                this.update(cx, |view, cx| {
                    view.apply_plugin_settings_snapshot(snapshot);
                    match result {
                        Ok(plugin) => termy_toast::success(format!(
                            "Installed {}. Open the command menu to use it.",
                            plugin.name
                        )),
                        Err(error) => termy_toast::error(error),
                    }
                    cx.notify();
                })
            });
        })
        .detach();
    }

    fn set_plugin_enabled_from_settings(
        &mut self,
        id: String,
        enabled: bool,
        cx: &mut Context<Self>,
    ) {
        if self.plugin_operation_in_flight {
            return;
        }
        self.plugin_operation_in_flight = true;
        cx.notify();
        let runtime = self.plugin_runtime.clone();
        cx.spawn(async move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let (result, snapshot) = smol::unblock(move || {
                let result = runtime.set_plugin_enabled(&id, enabled);
                let snapshot = Self::load_plugin_settings_snapshot(&runtime);
                (result, snapshot)
            })
            .await;
            let _ = cx.update(|cx| {
                this.update(cx, |view, cx| {
                    view.apply_plugin_settings_snapshot(snapshot);
                    match result {
                        Ok(()) if enabled => termy_toast::success("Plugin enabled"),
                        Ok(()) => termy_toast::success("Plugin disabled"),
                        Err(error) => termy_toast::error(error),
                    }
                    cx.notify();
                })
            });
        })
        .detach();
    }

    fn confirm_uninstall_plugin(&mut self, id: String, name: String, cx: &mut Context<Self>) {
        if self.plugin_operation_in_flight {
            return;
        }
        self.plugin_operation_in_flight = true;
        cx.notify();
        let runtime = self.plugin_runtime.clone();
        cx.spawn(async move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let message = format!(
                "Uninstall \"{name}\"? Its copied plugin directory will be permanently removed."
            );
            if !termy_native_sdk::confirm("Uninstall Plugin", &message) {
                let _ = cx.update(|cx| {
                    this.update(cx, |view, cx| {
                        view.plugin_operation_in_flight = false;
                        cx.notify();
                    })
                });
                return;
            }

            let (result, snapshot) = smol::unblock(move || {
                let result = runtime.uninstall_plugin(&id);
                let snapshot = Self::load_plugin_settings_snapshot(&runtime);
                (result, snapshot)
            })
            .await;
            let _ = cx.update(|cx| {
                this.update(cx, |view, cx| {
                    view.apply_plugin_settings_snapshot(snapshot);
                    match result {
                        Ok(()) => termy_toast::success("Plugin uninstalled"),
                        Err(error) => termy_toast::error(error),
                    }
                    cx.notify();
                })
            });
        })
        .detach();
    }

    fn open_plugins_directory(&mut self) {
        if let Err(error) = self.plugin_runtime.installed_plugins() {
            termy_toast::error(error);
            return;
        }
        let Some(path) = self.plugin_runtime.plugins_directory() else {
            termy_toast::error("Termy config path is unavailable");
            return;
        };

        #[cfg(target_os = "macos")]
        let result = Command::new("open")
            .arg(&path)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn();
        #[cfg(target_os = "linux")]
        let result = Command::new("xdg-open")
            .arg(&path)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn();
        #[cfg(target_os = "windows")]
        let result = Command::new("explorer")
            .arg(&path)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn();

        if let Err(error) = result {
            termy_toast::error(format!(
                "Failed to open plugin folder {}: {error}",
                path.display()
            ));
        }
    }

    pub(super) fn render_plugins_section(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let mut installed_rows = self
            .installed_plugins
            .clone()
            .into_iter()
            .map(|plugin| self.render_installed_plugin_row(plugin, cx))
            .collect::<Vec<_>>();
        if installed_rows.is_empty() {
            installed_rows.push(self.render_empty_plugins_row());
        }

        let mut section = div()
            .flex()
            .flex_col()
            .gap(px(CARD_GAP))
            .child(self.render_section_header(
                "Plugins",
                "Install and manage TypeScript extensions",
                SettingsSection::Plugins,
                cx,
            ))
            .child(self.render_settings_group(
                "Runtime",
                vec![
                    self.render_plugin_runtime_row(),
                    self.render_plugin_actions_row(cx),
                ],
            ))
            .child(self.render_settings_group("Installed plugins", installed_rows));

        if !self.plugin_inventory_errors.is_empty() || self.plugin_bun_error.is_some() {
            section = section.child(self.render_plugin_errors());
        }

        section.child(self.render_plugin_trust_notice())
    }

    fn render_plugin_runtime_row(&self) -> AnyElement {
        let ready = self.plugin_bun_path.is_some();
        let status = if ready { "Ready" } else { "Bun not found" };
        let detail = self.plugin_bun_path.as_ref().map_or_else(
            || "Install Bun to load plugins".to_string(),
            |path| path.display().to_string(),
        );
        let status_color = if ready {
            self.accent()
        } else {
            self.colors.ansi[3]
        };

        div()
            .w_full()
            .px(px(CARD_ROW_PADDING_X))
            .py(px(CARD_ROW_PADDING_Y))
            .flex()
            .items_center()
            .justify_between()
            .gap_4()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(3.0))
                    .child(
                        div()
                            .text_sm()
                            .font_weight(gpui::FontWeight::MEDIUM)
                            .text_color(self.text_primary())
                            .child("Bun runtime"),
                    )
                    .child(div().text_xs().text_color(self.text_muted()).child(detail)),
            )
            .child(
                div()
                    .text_xs()
                    .font_weight(gpui::FontWeight::MEDIUM)
                    .text_color(status_color)
                    .child(status),
            )
            .into_any_element()
    }

    fn render_plugin_actions_row(&self, cx: &mut Context<Self>) -> AnyElement {
        let busy = self.plugin_operation_in_flight;
        let border = self.border_color();
        let hover = self.bg_hover();
        let text_primary = self.text_primary();
        let text_secondary = self.text_secondary();
        let muted = self.text_muted();
        let button_bg = self.bg_input();
        let accent = self.accent();
        let accent_hover = self.accent_with_alpha(0.85);
        let button_text = self.contrasting_text_for_fill(accent, self.bg_card());

        let install_button = div()
            .id("plugin-install-folder")
            .h(px(28.0))
            .px_3()
            .rounded(px(SETTINGS_BUTTON_RADIUS))
            .bg(accent)
            .text_xs()
            .font_weight(gpui::FontWeight::MEDIUM)
            .text_color(button_text)
            .cursor_pointer()
            .flex()
            .items_center()
            .justify_center()
            .when(busy, |button| button.opacity(0.55))
            .when(!busy, |button| {
                button
                    .hover(move |style| style.bg(accent_hover))
                    .on_click(cx.listener(|view, _, _, cx| {
                        view.install_plugin_from_folder(cx);
                    }))
            })
            .child(if busy {
                "Working..."
            } else {
                "Install from folder"
            });

        let open_button = div()
            .id("plugin-open-folder")
            .h(px(28.0))
            .px_3()
            .rounded(px(SETTINGS_BUTTON_RADIUS))
            .border_1()
            .border_color(border)
            .bg(button_bg)
            .text_xs()
            .font_weight(gpui::FontWeight::MEDIUM)
            .text_color(text_secondary)
            .cursor_pointer()
            .flex()
            .items_center()
            .justify_center()
            .hover(move |style| style.bg(hover).text_color(text_primary))
            .child("Open folder")
            .on_click(cx.listener(|view, _, _, _| view.open_plugins_directory()));

        let refresh_button = div()
            .id("plugin-refresh")
            .h(px(28.0))
            .px_3()
            .rounded(px(SETTINGS_BUTTON_RADIUS))
            .text_xs()
            .font_weight(gpui::FontWeight::MEDIUM)
            .text_color(muted)
            .cursor_pointer()
            .flex()
            .items_center()
            .justify_center()
            .when(busy, |button| button.opacity(0.55))
            .when(!busy, |button| {
                button
                    .hover(move |style| style.bg(hover).text_color(text_primary))
                    .on_click(cx.listener(|view, _, _, cx| view.refresh_plugin_settings(cx)))
            })
            .child("Refresh");

        div()
            .w_full()
            .px(px(CARD_ROW_PADDING_X))
            .py(px(CARD_ROW_PADDING_Y))
            .flex()
            .items_center()
            .justify_between()
            .gap_4()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(3.0))
                    .child(
                        div()
                            .text_sm()
                            .font_weight(gpui::FontWeight::MEDIUM)
                            .text_color(self.text_primary())
                            .child("Local plugins"),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(muted)
                            .child("Choose a folder containing plugin.json and plugin.ts"),
                    ),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(6.0))
                    .child(refresh_button)
                    .child(open_button)
                    .child(install_button),
            )
            .into_any_element()
    }

    fn render_empty_plugins_row(&self) -> AnyElement {
        div()
            .w_full()
            .px(px(CARD_ROW_PADDING_X))
            .py(px(20.0))
            .flex()
            .flex_col()
            .items_center()
            .gap(px(4.0))
            .child(
                div()
                    .text_sm()
                    .font_weight(gpui::FontWeight::MEDIUM)
                    .text_color(self.text_primary())
                    .child("No plugins installed"),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(self.text_muted())
                    .child("Install a local TypeScript plugin folder to get started."),
            )
            .into_any_element()
    }

    fn render_installed_plugin_row(
        &self,
        plugin: InstalledPlugin,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let busy = self.plugin_operation_in_flight;
        let border = self.border_color();
        let button_bg = self.bg_input();
        let hover = self.bg_hover();
        let text_primary = self.text_primary();
        let text_secondary = self.text_secondary();
        let muted = self.text_muted();
        let has_error = plugin.error.is_some();
        let enabled = plugin.enabled && !has_error;
        let status = if has_error {
            "Invalid"
        } else if plugin.enabled {
            "Enabled"
        } else {
            "Disabled"
        };
        let status_color = if has_error {
            self.colors.ansi[1]
        } else if enabled {
            self.accent()
        } else {
            muted
        };
        let detail = plugin.error.clone().unwrap_or_else(|| {
            plugin.version.as_ref().map_or_else(
                || plugin.id.clone(),
                |version| format!("{} · v{version}", plugin.id),
            )
        });
        let toggle_id = plugin.id.clone();
        let uninstall_id = plugin.id.clone();
        let uninstall_name = plugin.name.clone();

        let toggle_button = div()
            .id(SharedString::from(format!("plugin-toggle-{}", plugin.id)))
            .h(px(27.0))
            .px_3()
            .rounded(px(SETTINGS_BUTTON_RADIUS))
            .border_1()
            .border_color(border)
            .bg(button_bg)
            .text_xs()
            .font_weight(gpui::FontWeight::MEDIUM)
            .text_color(text_secondary)
            .cursor_pointer()
            .flex()
            .items_center()
            .justify_center()
            .when(busy || has_error, |button| button.opacity(0.55))
            .when(!busy && !has_error, |button| {
                button
                    .hover(move |style| style.bg(hover).text_color(text_primary))
                    .on_click(cx.listener(move |view, _, _, cx| {
                        view.set_plugin_enabled_from_settings(
                            toggle_id.clone(),
                            !plugin.enabled,
                            cx,
                        );
                    }))
            })
            .child(if plugin.enabled { "Disable" } else { "Enable" });

        let uninstall_button = div()
            .id(SharedString::from(format!(
                "plugin-uninstall-{}",
                plugin.id
            )))
            .h(px(27.0))
            .px_3()
            .rounded(px(SETTINGS_BUTTON_RADIUS))
            .text_xs()
            .font_weight(gpui::FontWeight::MEDIUM)
            .text_color(muted)
            .cursor_pointer()
            .flex()
            .items_center()
            .justify_center()
            .when(busy, |button| button.opacity(0.55))
            .when(!busy, |button| {
                button
                    .hover(move |style| style.bg(hover).text_color(text_primary))
                    .on_click(cx.listener(move |view, _, _, cx| {
                        view.confirm_uninstall_plugin(
                            uninstall_id.clone(),
                            uninstall_name.clone(),
                            cx,
                        );
                    }))
            })
            .child("Uninstall");

        div()
            .w_full()
            .px(px(CARD_ROW_PADDING_X))
            .py(px(CARD_ROW_PADDING_Y))
            .flex()
            .items_center()
            .justify_between()
            .gap_4()
            .child(
                div()
                    .min_w(px(0.0))
                    .flex_1()
                    .flex()
                    .flex_col()
                    .gap(px(3.0))
                    .child(
                        div()
                            .text_sm()
                            .font_weight(gpui::FontWeight::MEDIUM)
                            .text_color(text_primary)
                            .child(plugin.name),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(if has_error { status_color } else { muted })
                            .overflow_x_hidden()
                            .child(detail),
                    ),
            )
            .child(
                div()
                    .flex_none()
                    .flex()
                    .items_center()
                    .gap(px(7.0))
                    .child(
                        div()
                            .text_xs()
                            .font_weight(gpui::FontWeight::MEDIUM)
                            .text_color(status_color)
                            .child(status),
                    )
                    .child(toggle_button)
                    .child(uninstall_button),
            )
            .into_any_element()
    }

    fn render_plugin_errors(&self) -> AnyElement {
        let mut messages = self.plugin_inventory_errors.clone();
        if let Some(error) = self.plugin_bun_error.clone() {
            messages.push(error);
        }
        let error_color = self.colors.ansi[1];
        let mut content = div()
            .w_full()
            .px(px(CARD_ROW_PADDING_X))
            .py(px(CARD_ROW_PADDING_Y))
            .rounded(px(SETTINGS_CARD_RADIUS))
            .bg(self.bg_elevated())
            .border_1()
            .border_color(self.card_border_color())
            .flex()
            .flex_col()
            .gap(px(5.0))
            .child(
                div()
                    .text_sm()
                    .font_weight(gpui::FontWeight::MEDIUM)
                    .text_color(error_color)
                    .child("Plugin runtime needs attention"),
            );
        for message in messages {
            content = content.child(div().text_xs().text_color(self.text_muted()).child(message));
        }
        content.into_any_element()
    }

    fn render_plugin_trust_notice(&self) -> AnyElement {
        div()
            .w_full()
            .px(px(CARD_ROW_PADDING_X))
            .py(px(CARD_ROW_PADDING_Y))
            .rounded(px(SETTINGS_CARD_RADIUS))
            .bg(self.bg_elevated())
            .border_1()
            .border_color(self.card_border_color())
            .flex()
            .flex_col()
            .gap(px(4.0))
            .child(
                div()
                    .text_sm()
                    .font_weight(gpui::FontWeight::MEDIUM)
                    .text_color(self.text_primary())
                    .child("Trusted local code"),
            )
            .child(div().text_xs().text_color(self.text_muted()).child(
                "Plugins run through Bun with your user permissions. Install only code you trust.",
            ))
            .into_any_element()
    }
}
