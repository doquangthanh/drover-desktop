//! Plugins in the app (plugins.rs is the file side): their actions in the command palette and as keys.toml shortcuts,
//! their event hooks when an agent changes state, and Plugins → install from GitHub (asked first: it is code).

use drover_proto::wire::Pane;
use gpui::{Context, Window};

use crate::actions::{Confirm, Then};
use crate::picker::Kind;
use crate::plugins::{self, Plugin};
use crate::toasts::{Target, Tone};
use crate::workspace_app::DroverApp;

type Env = Vec<(String, String)>;

/// What a plugin command is told about where it was started (see the top of plugins.rs).
fn env_for(plugin: &Plugin, event: &str, pane: Option<&Pane>) -> Env {
    let mut env = vec![("DROVER_PLUGIN_ID".to_string(), plugin.id.clone()), ("DROVER_PLUGIN_DIR".into(), plugin.dir.to_string_lossy().into_owned()),
                       ("DROVER_EVENT".into(), event.to_string())];
    if let Some(p) = pane {
        env.extend([("DROVER_PANE", &p.id), ("DROVER_PANE_NAME", &p.name), ("DROVER_STATE", &p.state), ("DROVER_TAB", &p.tab_id),
                    ("DROVER_SPACE", &p.space_id), ("DROVER_CWD", &p.cwd)].map(|(k, v)| (k.to_string(), v.clone())));
    }
    env
}

/// "owner/repo" or an https git URL -> (clone URL, folder name).
fn source(input: &str) -> Result<(String, String), String> {
    let input = input.trim().trim_end_matches('/');
    let url = match input.split('/').collect::<Vec<_>>().as_slice() {
        [owner, repo] if !owner.contains(':') => format!("https://github.com/{owner}/{repo}"),
        _ if input.starts_with("https://") => input.to_string(),
        _ => return Err("Write owner/repo (a GitHub repository) or an https:// git URL.".into()),
    };
    let name = url.rsplit('/').next().unwrap_or("").trim_end_matches(".git").to_string();
    if name.is_empty() || !name.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-')) || name.starts_with('.') {
        return Err(format!("\"{name}\" is not a usable folder name."));
    }
    Ok((url, name))
}

impl DroverApp {
    /// (id for keys.toml, palette label) of every plugin action.
    pub fn plugin_actions(&self) -> Vec<(String, String)> {
        self.plugins.iter().flat_map(|p| p.actions.iter().map(move |a| (format!("{}.{}", p.id, a.id), format!("{}: {}", p.name, a.title)))).collect()
    }

    /// Re-read the plugins folder.
    pub fn reload_plugins(&mut self, announce: bool) {
        let (plugins, problems) = plugins::load_from(&plugins::dir());
        let (actions, hooks) = (plugins.iter().map(|p| p.actions.len()).sum::<usize>(), plugins.iter().map(|p| p.hooks.len()).sum::<usize>());
        self.plugins = plugins;
        if !problems.is_empty() {
            self.toasts.error(problems.join("\n"));
        } else if announce {
            let sub = format!("{} plugin(s): {actions} action(s), {hooks} event hook(s)", self.plugins.len());
            self.toasts.push("Plugins reloaded".into(), sub, String::new(), Tone::Done, "", Target::Nothing);
        }
    }

    /// Run a command in the background; a failure is a toast, and so is what it printed when `show_output`.
    fn start_plugin_command(&mut self, plugin: &Plugin, command: Vec<String>, env: Env, show_output: bool) {
        let (dir, name) = (plugin.dir.clone(), plugin.name.clone());
        let rx = plugins::in_background(move || plugins::run(&command, &dir, &env));
        self.pending.push((rx, Box::new(move |app, r| match r {
            Ok(out) => {
                let out = out.as_str().unwrap_or("").lines().last().unwrap_or("").to_string();
                if show_output && !out.is_empty() {
                    app.toasts.push(name, out, String::new(), Tone::Done, "", Target::Nothing);
                }
            }
            Err(e) => app.toasts.error(format!("{name}: {e}")),
        })));
    }

    /// Run the action called `qualified` ("plugin.id.action") for the focused pane.
    pub fn run_plugin_action(&mut self, qualified: &str) {
        let found = self.plugins.iter().find_map(|p| {
            p.actions.iter().find(|a| format!("{}.{}", p.id, a.id) == qualified).map(|a| (p.clone(), a.command.clone()))
        });
        let Some((plugin, command)) = found else {
            self.toasts.error(format!("There is no plugin action called {qualified}."));
            return;
        };
        let pane = self.focused.as_deref().and_then(|id| self.store.pane(id)).cloned();
        let env = env_for(&plugin, "action", pane.as_ref());
        self.start_plugin_command(&plugin, command, env, true);
    }

    /// An agent changed state: run the plugins' hooks for `agent.<state>`.
    pub fn fire_plugin_event(&mut self, state: &str, pane: &Pane) {
        let event = format!("agent.{state}");
        let jobs: Vec<(Plugin, Vec<String>)> = self.plugins.iter()
            .flat_map(|p| p.hooks.iter().filter(|h| h.on == event).map(move |h| (p.clone(), h.command.clone()))).collect();
        for (plugin, command) in jobs {
            let env = env_for(&plugin, &event, Some(pane));
            self.start_plugin_command(&plugin, command, env, false);
        }
    }

    pub fn open_plugins_folder(&mut self) {
        let dir = plugins::dir();
        let _ = std::fs::create_dir_all(&dir);
        crate::actions::open_in_browser(&dir.to_string_lossy());
    }

    pub fn open_plugin_install(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.open_picker_with(Kind::InstallPlugin, ("owner/repo or https://… git URL", String::new()), None, window, cx);
    }

    /// Enter in the install box: ask before fetching anything.
    pub fn ask_install_plugin(&mut self, input: &str, cx: &mut Context<Self>) {
        match source(input) {
            Ok((url, name)) => {
                self.confirm = Some(Confirm { title: format!("Install {name}?"), ok: "Install", then: Then::InstallPlugin(url.clone()),
                    detail: format!("{url}\nA plugin is code that runs as you, and Drover does not sandbox it. Install only from authors you trust.") });
            }
            Err(e) => self.toasts.error(e),
        }
        cx.notify();
    }

    /// `git clone` into the plugins folder, then load it.
    pub fn install_plugin(&mut self, url: String) {
        let Ok((_, name)) = source(&url) else { return };
        let dest = plugins::dir().join(&name);
        if dest.exists() {
            self.toasts.error(format!("There is already a plugin folder called {name}. Remove it first to install again."));
            return;
        }
        self.toasts.push(format!("Installing {name}…"), url.clone(), String::new(), Tone::Update, "", Target::Nothing);
        let rx = plugins::in_background(move || {
            let _ = std::fs::create_dir_all(dest.parent().unwrap_or(&dest));
            plugins::run(&["git".into(), "clone".into(), "--depth".into(), "1".into(), "--".into(), url, dest.to_string_lossy().into_owned()], &std::env::temp_dir(), &[])?;
            if !dest.join(plugins::MANIFEST).is_file() {
                let _ = std::fs::remove_dir_all(&dest);
                return Err(format!("it has no {}, so it is not a Drover plugin", plugins::MANIFEST));
            }
            Ok(name)
        });
        self.pending.push((rx, Box::new(|app, r| match r {
            Ok(name) => {
                app.reload_plugins(false);
                app.toasts.push(format!("Installed {}", name.as_str().unwrap_or("")), "Its actions are in the command palette.".into(), String::new(), Tone::Done, "", Target::Nothing);
            }
            Err(e) => app.toasts.error(format!("Could not install the plugin: {e}")),
        })));
    }
}

#[cfg(test)]
mod tests {
    use super::source;

    #[test]
    fn accepts_github_shorthand_and_https_urls_only() {
        assert_eq!(source("acme/notes"), Ok(("https://github.com/acme/notes".into(), "notes".into())));
        assert_eq!(source(" https://example.com/a/b.git/ "), Ok(("https://example.com/a/b.git".into(), "b".into())));
        assert!(source("git@github.com:a/b.git").is_err());
        assert!(source("file:///etc").is_err());
        assert!(source("a/b/c").is_err());
        assert!(source("https://example.com/..").is_err());
    }
}
