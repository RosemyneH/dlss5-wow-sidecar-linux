#[derive(Clone, Copy)]
pub enum Msg {
    AppTitle,
    NavHeading,
    SectionStatus,
    SectionSetup,
    SectionChecks,
    SectionTuning,
    SectionLog,
    LanguageLabel,
    ThemeLabel,
    SaveSettings,
    ReloadFromDisk,
    UnsavedChanges,
    SettingsSaved,
    ShowHud,
    ShowOverlay,
    AdvancedMode,
    TuningPresetsHint,
    ThemeStoredLabel,
}

pub fn tr(lang: &str, msg: Msg) -> &'static str {
    let ru = lang == "ru";
    match msg {
        Msg::AppTitle => {
            if ru {
                "Менеджер WoW Sidecar"
            } else {
                "WoW Sidecar Manager"
            }
        }
        Msg::NavHeading => {
            if ru {
                "WoW Sidecar"
            } else {
                "WoW Sidecar"
            }
        }
        Msg::SectionStatus => {
            if ru {
                "Статус"
            } else {
                "Status"
            }
        }
        Msg::SectionSetup => {
            if ru {
                "Настройка"
            } else {
                "Setup"
            }
        }
        Msg::SectionChecks => {
            if ru {
                "Проверки"
            } else {
                "Checks"
            }
        }
        Msg::SectionTuning => {
            if ru {
                "Тюнинг"
            } else {
                "Tuning"
            }
        }
        Msg::SectionLog => {
            if ru {
                "Журнал"
            } else {
                "Log"
            }
        }
        Msg::LanguageLabel => {
            if ru {
                "Язык"
            } else {
                "Language"
            }
        }
        Msg::ThemeLabel => {
            if ru {
                "Тема"
            } else {
                "Theme"
            }
        }
        Msg::SaveSettings => {
            if ru {
                "Сохранить"
            } else {
                "Save settings"
            }
        }
        Msg::ReloadFromDisk => {
            if ru {
                "Перезагрузить с диска"
            } else {
                "Reload from disk"
            }
        }
        Msg::UnsavedChanges => {
            if ru {
                "Есть несохранённые изменения."
            } else {
                "Unsaved changes."
            }
        }
        Msg::SettingsSaved => {
            if ru {
                "Настройки сохранены."
            } else {
                "Settings saved."
            }
        }
        Msg::ShowHud => {
            if ru {
                "Показывать HUD"
            } else {
                "Show HUD"
            }
        }
        Msg::ShowOverlay => {
            if ru {
                "Показывать оверлей"
            } else {
                "Show overlay"
            }
        }
        Msg::AdvancedMode => {
            if ru {
                "Расширенный режим"
            } else {
                "Advanced mode"
            }
        }
        Msg::TuningPresetsHint => {
            if ru {
                "Пресеты меняют только рендер. Язык и тема — ниже."
            } else {
                "Presets adjust rendering settings only. Language and theme are below."
            }
        }
        Msg::ThemeStoredLabel => {
            if ru {
                "Тема интерфейса"
            } else {
                "Interface theme"
            }
        }
    }
}
