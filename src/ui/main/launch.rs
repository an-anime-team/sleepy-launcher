use relm4::prelude::*;
use gtk::prelude::*;

use anime_launcher_sdk::zzz::config::schema::prelude::LauncherBehavior;

use crate::*;

use super::{App, AppMsg};

pub fn launch(sender: ComponentSender<App>) {
    let config = Config::get().unwrap();

    match config.launcher.behavior {
        // Disable launch button and show kill game button if behavior set to "Nothing" to prevent sussy actions
        LauncherBehavior::Nothing => {
            sender.input(AppMsg::DisableButtons(true));
            sender.input(AppMsg::SetKillGameButton(true));
        }

        // Hide launcher window if behavior set to "Hide" or "Close"
        LauncherBehavior::Hide | LauncherBehavior::Close => sender.input(AppMsg::HideWindow)
    }

    std::thread::spawn(move || {
        let suggest_timeout_fix = match crate::dlss::run_game() {
            Ok(suggest) => suggest,
            Err(err) => {
                tracing::error!("Failed to launch game: {err}");

                sender.input(AppMsg::Toast {
                    title: tr!("game-launching-failed"),
                    description: Some(err.to_string())
                });

                false
            }
        };

        match config.launcher.behavior {
            // Enable launch button and hide kill game button if behavior set to "Nothing" after the game has closed
            LauncherBehavior::Nothing => {
                sender.input(AppMsg::DisableButtons(false));
                sender.input(AppMsg::SetKillGameButton(false));

                if suggest_timeout_fix {
                    sender.input(AppMsg::SuggestTimeoutFix);
                }
            }

            // Show back launcher window if behavior set to "Hide" and the game has closed
            LauncherBehavior::Hide => {
                sender.input(AppMsg::ShowWindow);

                if suggest_timeout_fix {
                    sender.input(AppMsg::SuggestTimeoutFix);
                }
            }

            // Otherwise close the launcher if behavior set to "Close" and the game has closed
            // We're calling quit method from the main context here because otherwise app won't be closed properly
            LauncherBehavior::Close => gtk::glib::MainContext::default().invoke(|| {
                relm4::main_application().quit();
            })
        }
    });
}
